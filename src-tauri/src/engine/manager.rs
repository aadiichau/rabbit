use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager as _};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use super::http::{self, Counters, ResumeState};
use super::{fsname, ingest, platform, post, ytdlp_dl};
use crate::error::{AppError, AppResult, ErrorInfo};
use crate::extract::model::*;
use crate::extract::bilibili;
use crate::extract::web::{SessionCookies, Web};
use crate::sys::cookies::{self, Site};
use crate::extract::{Native, fixups, normalize, ytdlp};
use crate::store::{NewLibraryItem, Settings, Store};
use crate::sys::tools::Tools;
use crate::sys::updater;

const ANALYSIS_TTL: Duration = Duration::from_secs(30 * 60);
const TICK: Duration = Duration::from_millis(125);
const THUMB_MAX: usize = 3 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Queued,
    Downloading,
    Processing,
    Verifying,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl TaskState {
    pub fn active(self) -> bool {
        matches!(self, TaskState::Queued | TaskState::Downloading | TaskState::Processing | TaskState::Verifying)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub id: String,
    pub title: String,
    pub uploader: Option<String>,
    pub source_url: String,
    pub platform: platform::Platform,
    pub platform_name: String,
    pub option_id: String,
    pub option_label: String,
    pub container: String,
    pub kind: MediaKind,
    pub thumbnail: Option<String>,
    pub state: TaskState,
    pub error: Option<ErrorInfo>,
    pub file_path: Option<String>,
    pub done: u64,
    pub total: Option<u64>,
    pub created_at: i64,
    /// True when the task was running when Rabbit last exited.
    #[serde(default)]
    pub interrupted: bool,
}

/// What survives an app restart. Stream URLs and headers are deliberately not stored:
/// they expire and can carry tokens, so a resumed task re-analyzes its source instead.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskRecord {
    info: TaskInfo,
    duration: Option<f64>,
    final_path: Option<PathBuf>,
    work_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TaskEvent {
    #[serde(rename_all = "camelCase")]
    Upsert { task: TaskInfo },
    #[serde(rename_all = "camelCase")]
    Progress { id: String, done: u64, total: Option<u64>, speed: f64, eta: Option<f64>, segments: Vec<f32> },
    #[serde(rename_all = "camelCase")]
    Removed { id: String },
}

struct Speed {
    last_done: u64,
    last_at: Instant,
    ewma: f64,
    samples: u32,
}

impl Speed {
    fn new() -> Self {
        Speed { last_done: 0, last_at: Instant::now(), ewma: 0.0, samples: 0 }
    }
}

struct Task {
    info: Mutex<TaskInfo>,
    /// `None` until (re-)analysis supplies fresh stream URLs.
    plan: Mutex<Option<Plan>>,
    cancel: Mutex<CancellationToken>,
    pause_requested: AtomicBool,
    /// Set by `remove`; a run that is still winding down must not re-save or re-announce the task.
    removed: AtomicBool,
    counters: Mutex<Vec<Arc<Counters>>>,
    speed: Mutex<Speed>,
    duration: Option<f64>,
    final_path: Mutex<Option<PathBuf>>,
    work_dir: Mutex<Option<PathBuf>>,
}

impl Task {
    fn id(&self) -> String {
        self.info.lock().unwrap().id.clone()
    }
}

struct Analysis {
    info: MediaInfo,
    plans: HashMap<String, Plan>,
    at: Instant,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UpdateEvent {
    #[serde(rename_all = "camelCase")]
    Progress { done: u64, total: Option<u64> },
}

/// Result of Settings → Signed-in sites → Check.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub site: Site,
    /// "signedIn" | "signedOut" | "noCookies"
    pub state: String,
    pub account: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtDlpStatus {
    /// Version reported by the yt-dlp in use (`None` if it couldn't be run).
    pub version: Option<String>,
    /// "update" when a verified downloaded copy is active, else "bundled".
    pub source: &'static str,
    pub update: Option<updater::Installed>,
    pub channel: updater::Channel,
    pub path: Option<String>,
}

pub struct Manager {
    client: reqwest::Client,
    /// For Rabbit's own extractors: resolves redirects hop by hop.
    web: Web,
    tools: std::sync::RwLock<Tools>,
    update_dir: Mutex<Option<PathBuf>>,
    update_source: Mutex<updater::Source>,
    version_cache: Mutex<Option<(PathBuf, String)>>,
    update_busy: tokio::sync::Mutex<()>,
    store: Arc<Store>,
    settings: Mutex<Settings>,
    sem: Arc<Semaphore>,
    sem_size: Mutex<usize>,
    tasks: Mutex<Vec<(String, Arc<Task>)>>,
    reserved: Mutex<HashSet<PathBuf>>,
    analyses: Mutex<HashMap<String, Analysis>>,
    analyze_cancel: Mutex<Option<CancellationToken>>,
    sink: Mutex<Option<Channel<TaskEvent>>>,
    app: OnceLock<AppHandle>,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn new_id() -> String {
    use std::sync::atomic::AtomicU64;
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{:x}{:04x}", nanos & 0xffff_ffff_ffff, SEQ.fetch_add(1, Ordering::Relaxed) & 0xffff)
}

fn temp_prefix(id: &str) -> String {
    format!(".rabbit-{id}-")
}

/// Bytes already saved for a task, from its resume-state files (shown before resuming).
fn saved_bytes(dir: &Path, id: &str) -> u64 {
    let prefix = temp_prefix(id);
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    rd.flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.starts_with(&prefix) && n.ends_with(".state.json")
        })
        .filter_map(|e| ResumeState::load(&e.path()))
        .map(|s| s.segments.iter().map(|x| x.2).sum::<u64>())
        .sum()
}

fn remove_temps(dir: &Path, id: &str) {
    let prefix = temp_prefix(id);
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Temp data worth keeping after a failure: transient network problems can resume.
fn keep_partial_after(e: &AppError) -> bool {
    matches!(e, AppError::Network(_) | AppError::Expired(_) | AppError::Http(429) | AppError::Http(500..=599))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedUrl {
    pub url: String,
    pub platform: platform::Platform,
    pub platform_name: &'static str,
    pub error: Option<String>,
}

pub fn detect_input(text: &str) -> Vec<DetectedUrl> {
    ingest::extract_urls(text)
        .into_iter()
        .map(|u| match ingest::validate(&u) {
            Ok(url) => {
                let p = platform::detect(&url);
                DetectedUrl { url: url.to_string(), platform: p, platform_name: p.display_name(), error: None }
            }
            Err(e) => DetectedUrl { url: u, platform: platform::Platform::Other, platform_name: "Other", error: Some(e.to_string()) },
        })
        .collect()
}

impl Manager {
    pub fn new(store: Arc<Store>, settings: Settings, tools: Tools) -> AppResult<Arc<Self>> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(16)
            .tcp_nodelay(true)
            .build()
            .map_err(|e| AppError::Network(e.to_string()))?;
        let n = settings.max_concurrent.clamp(1, 8);

        // Restore the queue. Anything that was running is shown as paused/interrupted.
        let mut restored: Vec<(String, Arc<Task>)> = Vec::new();
        for raw in store.load_tasks()? {
            let Ok(rec) = serde_json::from_str::<TaskRecord>(&raw) else { continue };
            let mut info = rec.info;
            if info.state.active() {
                info.state = TaskState::Paused;
                info.interrupted = true;
            }
            if info.state == TaskState::Paused {
                if let Some(dir) = &rec.work_dir {
                    let saved = saved_bytes(dir, &info.id);
                    if saved > 0 {
                        info.done = saved;
                    }
                }
            }
            let task = Arc::new(Task {
                info: Mutex::new(info.clone()),
                plan: Mutex::new(None),
                cancel: Mutex::new(CancellationToken::new()),
                pause_requested: AtomicBool::new(false),
                removed: AtomicBool::new(false),
                counters: Mutex::new(Vec::new()),
                speed: Mutex::new(Speed::new()),
                duration: rec.duration,
                final_path: Mutex::new(rec.final_path),
                work_dir: Mutex::new(rec.work_dir),
            });
            restored.push((info.id.clone(), task));
        }
        restored.sort_by_key(|(_, t)| t.info.lock().unwrap().created_at);

        let m = Arc::new(Manager {
            client,
            web: Web::new()?,
            tools: std::sync::RwLock::new(tools),
            update_dir: Mutex::new(None),
            update_source: Mutex::new(updater::Source::default()),
            version_cache: Mutex::new(None),
            update_busy: tokio::sync::Mutex::new(()),
            store,
            settings: Mutex::new(settings),
            sem: Arc::new(Semaphore::new(n)),
            sem_size: Mutex::new(n),
            tasks: Mutex::new(restored),
            reserved: Mutex::new(HashSet::new()),
            analyses: Mutex::new(HashMap::new()),
            analyze_cancel: Mutex::new(None),
            sink: Mutex::new(None),
            app: OnceLock::new(),
        });
        for (_, t) in m.tasks.lock().unwrap().iter() {
            m.persist(t);
        }
        let ticker = m.clone();
        tauri::async_runtime::spawn(async move { ticker.progress_loop().await });
        Ok(m)
    }

    /// Enables OS notifications (not available in headless tests).
    pub fn attach_app(&self, app: AppHandle) {
        let _ = self.app.set(app);
    }

    /// A snapshot of the tools; a yt-dlp update applies to operations started afterwards.
    pub fn tools(&self) -> Tools {
        self.tools.read().unwrap().clone()
    }

    /// Where verified yt-dlp updates live; activates one if present.
    pub fn set_update_dir(&self, dir: PathBuf) {
        self.tools.write().unwrap().apply_update_dir(&dir);
        *self.update_dir.lock().unwrap() = Some(dir);
    }

    /// Tests point the updater at a local release server.
    pub fn set_update_source(&self, src: updater::Source) {
        *self.update_source.lock().unwrap() = src;
    }

    fn update_dir(&self) -> AppResult<PathBuf> {
        self.update_dir.lock().unwrap().clone().ok_or_else(|| AppError::Tool("updates aren't available in this build".into()))
    }

    async fn active_version(&self) -> Option<String> {
        let tools = self.tools();
        if let Some(u) = &tools.ytdlp_update {
            return Some(u.version.clone());
        }
        let path = tools.ytdlp?;
        if let Some((p, v)) = self.version_cache.lock().unwrap().clone() {
            if p == path {
                return Some(v);
            }
        }
        let v = updater::run_version(&path).await.ok()?;
        *self.version_cache.lock().unwrap() = Some((path, v.clone()));
        Some(v)
    }

    pub async fn ytdlp_status(&self) -> YtDlpStatus {
        let version = self.active_version().await;
        let tools = self.tools();
        let channel = tools.ytdlp_update.as_ref().map(|u| u.channel).unwrap_or(self.settings().ytdlp_channel);
        YtDlpStatus {
            version,
            source: if tools.ytdlp_update.is_some() { "update" } else { "bundled" },
            update: tools.ytdlp_update.clone(),
            channel,
            path: tools.ytdlp.map(|p| p.display().to_string()),
        }
    }

    /// Asks GitHub for the latest release on `channel`. Network access happens only when the user asks.
    pub async fn ytdlp_check(&self, channel: updater::Channel) -> AppResult<updater::UpdateCheck> {
        let src = self.update_source.lock().unwrap().clone();
        let (latest, published_at) = updater::latest(&self.client, &src, channel).await?;
        let current = self.active_version().await;
        let current_channel = self.tools().ytdlp_update.map(|u| u.channel).unwrap_or(updater::Channel::Stable);
        // Switching channels may mean installing an older-numbered build; that's still an update.
        let available = current.as_deref() != Some(latest.as_str()) && (updater::is_newer(&latest, current.as_deref()) || channel != current_channel);
        Ok(updater::UpdateCheck { channel, current, latest, published_at, available })
    }

    pub async fn ytdlp_install(&self, channel: updater::Channel, tag: &str, events: Option<Channel<UpdateEvent>>) -> AppResult<YtDlpStatus> {
        let _busy = self.update_busy.try_lock().map_err(|_| AppError::Tool("an update is already running".into()))?;
        let dir = self.update_dir()?;
        let src = self.update_source.lock().unwrap().clone();
        let counters = Arc::new(Counters::default());
        let stop = CancellationToken::new();
        let ticker = events.map(|ch| {
            let c = counters.clone();
            let stop = stop.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let total = c.total.load(Ordering::Relaxed);
                    let _ = ch.send(UpdateEvent::Progress { done: c.done.load(Ordering::Relaxed), total: (total > 0).then_some(total) });
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(150)) => {}
                        _ = stop.cancelled() => break,
                    }
                }
            })
        });
        let result = updater::install(&self.client, &src, channel, tag, &dir, &counters, &CancellationToken::new()).await;
        stop.cancel();
        if let Some(t) = ticker {
            let _ = t.await;
        }
        let installed = result?;
        tracing::info!(version = %installed.version, channel = ?installed.channel, "yt-dlp updated");
        self.tools.write().unwrap().apply_update_dir(&dir);
        *self.version_cache.lock().unwrap() = None;
        let s = Settings { ytdlp_channel: channel, ..self.settings() };
        let _ = self.update_settings(s);
        Ok(self.ytdlp_status().await)
    }

    pub async fn ytdlp_use_bundled(&self) -> AppResult<YtDlpStatus> {
        let dir = self.update_dir()?;
        updater::revert(&dir)?;
        self.tools.write().unwrap().apply_update_dir(&dir);
        *self.version_cache.lock().unwrap() = None;
        Ok(self.ytdlp_status().await)
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn update_settings(&self, s: Settings) -> AppResult<Settings> {
        let s = Settings { max_concurrent: s.max_concurrent.clamp(1, 8), max_segments: s.max_segments.clamp(1, 16), ..s };
        self.store.save_settings(&s)?;
        // Grow or shrink the concurrency limit without disturbing running tasks.
        let mut size = self.sem_size.lock().unwrap();
        if s.max_concurrent > *size {
            self.sem.add_permits(s.max_concurrent - *size);
        } else if s.max_concurrent < *size {
            let sem = self.sem.clone();
            let diff = (*size - s.max_concurrent) as u32;
            tauri::async_runtime::spawn(async move {
                if let Ok(p) = sem.acquire_many_owned(diff).await {
                    p.forget();
                }
            });
        }
        *size = s.max_concurrent;
        *self.settings.lock().unwrap() = s.clone();
        Ok(s)
    }

    pub fn subscribe(&self, ch: Channel<TaskEvent>) -> Vec<TaskInfo> {
        *self.sink.lock().unwrap() = Some(ch);
        self.list()
    }

    fn emit(&self, ev: TaskEvent) {
        if let Some(ch) = self.sink.lock().unwrap().as_ref() {
            let _ = ch.send(ev);
        }
    }

    fn persist(&self, t: &Task) {
        if t.removed.load(Ordering::Relaxed) {
            return;
        }
        let rec = TaskRecord {
            info: t.info.lock().unwrap().clone(),
            duration: t.duration,
            final_path: t.final_path.lock().unwrap().clone(),
            work_dir: t.work_dir.lock().unwrap().clone(),
        };
        let created = rec.info.created_at;
        match serde_json::to_string(&rec) {
            Ok(json) => {
                if let Err(e) = self.store.save_task(&rec.info.id, &json, created) {
                    tracing::warn!("couldn't persist task: {e}");
                }
            }
            Err(e) => tracing::warn!("couldn't serialise task: {e}"),
        }
    }

    pub fn list(&self) -> Vec<TaskInfo> {
        self.tasks.lock().unwrap().iter().map(|(_, t)| t.info.lock().unwrap().clone()).collect()
    }

    fn task(&self, id: &str) -> AppResult<Arc<Task>> {
        self.tasks.lock().unwrap().iter().find(|(i, _)| i == id).map(|(_, t)| t.clone()).ok_or_else(|| AppError::NotFound("That download no longer exists.".into()))
    }

    // ---------- analysis ----------

    /// UI-initiated analysis: a newer request cancels the previous one.
    pub async fn analyze(&self, input: &str) -> AppResult<MediaInfo> {
        let cancel = CancellationToken::new();
        if let Some(prev) = self.analyze_cancel.lock().unwrap().replace(cancel.clone()) {
            prev.cancel();
        }
        self.analyze_with(input, &cancel).await
    }

    async fn analyze_with(&self, input: &str, cancel: &CancellationToken) -> AppResult<MediaInfo> {
        let urls = ingest::extract_urls(input);
        let raw = urls.first().ok_or_else(|| AppError::Input(ingest::InputError::NoUrl.to_string()))?;
        let url = ingest::validate(raw).map_err(|e| AppError::Input(e.to_string()))?;
        let platform = platform::detect(&url);

        if let Some(reason) = platform::unsupported_reason(platform) {
            return Err(AppError::Unsupported(reason.into()));
        }
        let started = Instant::now();
        let (json, norm) = self.extract(platform, &url, cancel).await?;

        let s = |k: &str| json.get(k).and_then(|v| v.as_str()).map(str::to_string).filter(|x| !x.is_empty());
        let thumb_url = s("thumbnail").or_else(|| {
            json.get("entries").and_then(|e| e.get(0)).and_then(|e| e.get("thumbnail")).and_then(|t| t.as_str()).map(str::to_string)
        });
        let thumbnail = match thumb_url {
            Some(t) => self.fetch_thumbnail(&t).await,
            None => None,
        };
        let upload_date = s("upload_date").filter(|d| d.len() == 8).map(|d| format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8]));

        let analysis_id = new_id();
        let info = MediaInfo {
            analysis_id: analysis_id.clone(),
            source_url: s("webpage_url").unwrap_or_else(|| url.to_string()),
            platform,
            platform_name: platform.display_name(),
            extractor: s("extractor_key").unwrap_or_else(|| "yt-dlp".into()),
            id: s("id").unwrap_or_default(),
            title: s("title").or_else(|| s("fulltitle")).unwrap_or_else(|| "Untitled".into()),
            uploader: s("uploader").or_else(|| s("channel")).or_else(|| s("creator")).or_else(|| s("uploader_id")),
            duration: json.get("duration").and_then(|d| d.as_f64()),
            upload_date,
            view_count: json.get("view_count").and_then(|v| v.as_u64()),
            thumbnail,
            kind: norm.kind,
            item_count: norm.item_count,
            options: norm.options.iter().map(|(o, _)| o.clone()).collect(),
            elapsed_ms: started.elapsed().as_millis() as u64,
            session: s("_rabbit_session"),
        };
        let plans = norm.options.into_iter().map(|(o, p)| (o.id, p)).collect();
        let mut analyses = self.analyses.lock().unwrap();
        analyses.retain(|_, a| a.at.elapsed() < ANALYSIS_TTL);
        analyses.insert(analysis_id, Analysis { info: info.clone(), plans, at: Instant::now() });
        Ok(info)
    }

    /// Extraction chain: Rabbit's own extractor when one claims the address, then yt-dlp.
    /// A result only counts once the normalizer finds downloadable media in it.
    async fn extract(&self, platform: platform::Platform, url: &url::Url, cancel: &CancellationToken) -> AppResult<(serde_json::Value, normalize::Normalized)> {
        let has_ffmpeg = self.tools().ffmpeg.is_some();
        let mut native_err = None;
        // The user's own Bilibili session, only when they switched it on (docs/DESIGN_SIGNED_IN_SESSIONS.md).
        let mut session_note: Option<String> = None;
        let bilibili_session = if platform == platform::Platform::Bilibili {
            match self.session_cookies(Site::Bilibili).await {
                Ok(Some(c)) if c.is_empty() => {
                    session_note = Some("guest qualities — no Bilibili cookies in your session source; sign in to bilibili.com in Firefox".into());
                    None
                }
                Ok(Some(c)) => SessionCookies::new(Site::Bilibili, &c, bilibili::SESSION_HOSTS),
                Ok(None) => None,
                Err(e) => {
                    session_note = Some(format!("guest qualities — couldn't read your Bilibili session ({e})"));
                    None
                }
            }
        } else {
            None
        };
        if let Some(native) = Native::for_url(platform, url) {
            let attempt = tokio::select! {
                r = native.extract(&self.web, url, bilibili_session.as_ref()) => r,
                _ = cancel.cancelled() => return Err(AppError::Cancelled),
            };
            match attempt {
                Ok(Some(mut v)) => match normalize::normalize(&v, has_ffmpeg) {
                    Ok(n) => {
                        if let Some(note) = session_note.filter(|_| v.get("_rabbit_session").is_none()) {
                            v["_rabbit_session"] = serde_json::json!(note);
                        }
                        return Ok((v, n));
                    }
                    Err(e) => native_err = Some(e),
                },
                Ok(None) => {}
                // The platform answered definitively (sign-in only, bad link): yt-dlp can't change that.
                Err(e @ (AppError::LoginRequired(_) | AppError::Input(_))) => return Err(e),
                Err(e) => native_err = Some(e),
            }
            if let Some(e) = &native_err {
                tracing::warn!(?native, kind = e.kind(), "native extractor failed: {e}");
                if !native.ytdlp_fallback() {
                    return Err(native_err.unwrap());
                }
            }
        }
        let mut extra: Vec<String> = fixups::ytdlp_args(platform).iter().map(|s| s.to_string()).collect();
        // Douyin with the user's session: a temporary cookies.txt holding only douyin.com cookies,
        // deleted when this function returns (success, failure or cancel).
        let mut _cookie_file = None;
        let douyin_session = platform == platform::Platform::Douyin && self.settings().sessions.source_for(Site::Douyin).is_some();
        if douyin_session {
            let cookies = self.session_cookies(Site::Douyin).await.map_err(|e| AppError::Tool(format!("Couldn't read your Douyin session: {e}")))?.unwrap_or_default();
            if cookies.is_empty() {
                return Err(AppError::LoginRequired("No Douyin cookies were found in your session source. Sign in to douyin.com in Firefox, then try again".into()));
            }
            let file = cookies::TempCookieFile::create(&self.session_dir(), &cookies)?;
            extra.extend(["--cookies".to_string(), file.path.display().to_string()]);
            _cookie_file = Some(file);
        }
        let extra_refs: Vec<&str> = extra.iter().map(String::as_str).collect();
        let attempt = ytdlp::dump_json_with(&self.tools(), url.as_str(), &extra_refs, cancel)
            .await
            .map_err(|e| platform::explain(platform, e))
            .map_err(|e| match (platform, e) {
                (platform::Platform::Douyin, AppError::LoginRequired(d)) if douyin_session => {
                    AppError::LoginRequired(format!("Your Douyin session has expired or isn't signed in. Open douyin.com in Firefox and sign in, then try again. {d}"))
                }
                (platform::Platform::Douyin, AppError::LoginRequired(d)) => {
                    AppError::LoginRequired(format!("Douyin only shows videos to signed-in visitors. To use your own browser session, turn on Douyin in Settings → Signed-in sites. {d}"))
                }
                (_, e) => e,
            })
            .map(|mut v| {
                fixups::apply(platform, &mut v);
                if douyin_session {
                    v["_rabbit_session"] = serde_json::json!("your Douyin session");
                }
                v
            })
            .and_then(|v| normalize::normalize(&v, has_ffmpeg).map(|n| (v, n)));
        match (attempt, native_err) {
            (Ok(done), _) => Ok(done),
            (Err(AppError::Cancelled), _) => Err(AppError::Cancelled),
            // Rabbit's own extractor knows more about the failure than a broken yt-dlp extractor.
            (Err(e), Some(native)) => {
                tracing::warn!(kind = e.kind(), "yt-dlp fallback failed too: {e}");
                Err(native)
            }
            (Err(e), None) => Err(e),
        }
    }

    /// Where temporary session files live: Rabbit's local app-data folder (user-only by default).
    fn session_dir(&self) -> PathBuf {
        self.update_dir.lock().unwrap().as_ref().and_then(|d| d.parent().map(|p| p.join("sessions"))).unwrap_or_else(|| std::env::temp_dir().join("rabbit-sessions"))
    }

    /// The user's cookies for `site` when they switched it on; `Ok(None)` when it's off.
    async fn session_cookies(&self, site: Site) -> AppResult<Option<Vec<cookies::Cookie>>> {
        let Some(src) = self.settings().sessions.source_for(site).cloned() else { return Ok(None) };
        tokio::task::spawn_blocking(move || cookies::read(&src, site)).await.map_err(|e| AppError::Io(e.to_string()))?.map(Some)
    }

    /// Settings → Signed-in sites → Check: reads the chosen source (even before the site's switch
    /// is on) and asks the site whether the session is signed in.
    pub async fn session_check(&self, site: Site) -> AppResult<SessionStatus> {
        let src = self.settings().sessions.source.clone().ok_or_else(|| AppError::Input("Choose a Firefox profile or a cookies.txt file first.".into()))?;
        let cookies = tokio::task::spawn_blocking(move || cookies::read(&src, site)).await.map_err(|e| AppError::Io(e.to_string()))??;
        let status = |state: &str, account: Option<String>, detail: &str| SessionStatus { site, state: state.into(), account, detail: detail.into() };
        if cookies.is_empty() {
            return Ok(status("noCookies", None, &format!("No {} cookies in this source. Sign in to the site in Firefox first.", site.name())));
        }
        Ok(match site {
            Site::Bilibili => {
                let session = SessionCookies::new(site, &cookies, bilibili::SESSION_HOSTS).expect("non-empty");
                let (acc, _) = bilibili::account(&self.web, &session).await?;
                match (acc.logged_in, acc.premium) {
                    (true, true) => status("signedIn", acc.name, "大会员 — 1080P+, 4K and HDR where a video offers them"),
                    (true, false) => status("signedIn", acc.name, "up to 1080p"),
                    (false, _) => status("signedOut", None, "Bilibili says this session isn't signed in (or has expired)."),
                }
            }
            // Douyin has no unsigned account endpoint; a login cookie is the available signal.
            Site::Douyin => {
                if cookies.iter().any(|c| matches!(c.name.as_str(), "sessionid" | "sessionid_ss" | "sid_tt" | "sid_guard")) {
                    status("signedIn", None, "A Douyin login cookie is present; the first download confirms it.")
                } else {
                    // yt-dlp asks for "fresh cookies (not necessarily logged in)": a recent visit may do.
                    status("signedOut", None, "Douyin cookies but no login cookie. A fresh visit to douyin.com in Firefox may be enough; signing in is the reliable way.")
                }
            }
        })
    }

    pub fn cancel_analyze(&self) {
        if let Some(c) = self.analyze_cancel.lock().unwrap().take() {
            c.cancel();
        }
    }

    async fn fetch_thumbnail(&self, url: &str) -> Option<String> {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return None;
        }
        let resp = tokio::time::timeout(Duration::from_secs(8), self.client.get(url).send()).await.ok()?.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let mime = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("image/jpeg").to_string();
        if !mime.starts_with("image/") || resp.content_length().is_some_and(|l| l as usize > THUMB_MAX) {
            return None;
        }
        let bytes = tokio::time::timeout(Duration::from_secs(8), resp.bytes()).await.ok()?.ok()?;
        if bytes.len() > THUMB_MAX {
            return None;
        }
        Some(format!("data:{};base64,{}", mime.split(';').next().unwrap_or("image/jpeg"), base64::engine::general_purpose::STANDARD.encode(&bytes)))
    }

    /// Re-analyzes a task's source to get fresh stream URLs for the same option.
    async fn refresh_plan(&self, task: &Task, cancel: &CancellationToken) -> AppResult<()> {
        let (source, option_id) = {
            let i = task.info.lock().unwrap();
            (i.source_url.clone(), i.option_id.clone())
        };
        let info = self.analyze_with(&source, cancel).await?;
        let plan = self.analyses.lock().unwrap().get(&info.analysis_id).and_then(|a| a.plans.get(&option_id).cloned());
        let plan = plan.ok_or_else(|| AppError::NotFound("The previously chosen format is no longer offered by the source.".into()))?;
        *task.plan.lock().unwrap() = Some(plan);
        Ok(())
    }

    // ---------- queue ----------

    pub fn enqueue(self: &Arc<Self>, analysis_id: &str, option_id: &str) -> AppResult<TaskInfo> {
        let (info, plan, option) = {
            let analyses = self.analyses.lock().unwrap();
            let a = analyses.get(analysis_id).ok_or_else(|| AppError::NotFound("This analysis expired. Analyze the link again.".into()))?;
            let plan = a.plans.get(option_id).cloned().ok_or_else(|| AppError::NotFound("That format is no longer available.".into()))?;
            let option = a.info.options.iter().find(|o| o.id == option_id).cloned().expect("option has a plan");
            (a.info.clone(), plan, option)
        };
        let id = new_id();
        let task_info = TaskInfo {
            id: id.clone(),
            title: info.title.clone(),
            uploader: info.uploader.clone(),
            source_url: info.source_url.clone(),
            platform: info.platform,
            platform_name: info.platform_name.to_string(),
            option_id: option.id.clone(),
            option_label: option.label.clone(),
            container: if plan.gallery { "folder".into() } else { plan.out_ext.clone() },
            kind: info.kind,
            thumbnail: info.thumbnail.clone(),
            state: TaskState::Queued,
            error: None,
            file_path: None,
            done: 0,
            total: option.size,
            created_at: now_ms(),
            interrupted: false,
        };
        let task = Arc::new(Task {
            info: Mutex::new(task_info.clone()),
            plan: Mutex::new(Some(plan)),
            cancel: Mutex::new(CancellationToken::new()),
            pause_requested: AtomicBool::new(false),
            removed: AtomicBool::new(false),
            counters: Mutex::new(Vec::new()),
            speed: Mutex::new(Speed::new()),
            duration: info.duration,
            final_path: Mutex::new(None),
            work_dir: Mutex::new(None),
        });
        self.tasks.lock().unwrap().push((id.clone(), task.clone()));
        self.persist(&task);
        self.emit(TaskEvent::Upsert { task: task_info.clone() });
        self.spawn_run(task);
        Ok(task_info)
    }

    fn spawn_run(self: &Arc<Self>, task: Arc<Task>) {
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            let cancel = task.cancel.lock().unwrap().clone();
            let result = me.drive(&task, &cancel).await;
            let paused = task.pause_requested.swap(false, Ordering::Relaxed);
            let snapshot = {
                let mut info = task.info.lock().unwrap();
                info.interrupted = false;
                match &result {
                    Ok(path) => {
                        info.state = TaskState::Completed;
                        info.file_path = Some(path.display().to_string());
                        if let Some(t) = info.total {
                            info.done = t;
                        }
                    }
                    Err(AppError::Cancelled) if paused => info.state = TaskState::Paused,
                    Err(AppError::Cancelled) => {
                        // Partial data is discarded on cancel, so don't show it as progress.
                        info.state = TaskState::Cancelled;
                        info.done = 0;
                    }
                    Err(e) => {
                        tracing::warn!(task = %info.id, kind = e.kind(), "download failed: {e}");
                        info.state = TaskState::Failed;
                        info.error = Some(ErrorInfo::from(e));
                    }
                }
                info.clone()
            };
            if task.removed.load(Ordering::Relaxed) {
                return;
            }
            me.persist(&task);
            me.emit(TaskEvent::Upsert { task: snapshot.clone() });
            me.notify(&snapshot);
        });
    }

    fn notify(&self, t: &TaskInfo) {
        if !self.settings.lock().unwrap().notifications {
            return;
        }
        let Some(app) = self.app.get() else { return };
        let focused = app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
        if focused {
            return;
        }
        let (title, body) = match t.state {
            TaskState::Completed => ("Download finished", t.title.clone()),
            TaskState::Failed => ("Download failed", format!("{} â€” {}", t.title, t.error.as_ref().map(|e| e.message.as_str()).unwrap_or(""))),
            _ => return,
        };
        use tauri_plugin_notification::NotificationExt;
        if let Err(e) = app.notification().builder().title(title).body(body).show() {
            tracing::warn!("notification failed: {e}");
        }
    }

    fn set_state(&self, task: &Task, state: TaskState) {
        if task.removed.load(Ordering::Relaxed) {
            return;
        }
        let snapshot = {
            let mut i = task.info.lock().unwrap();
            i.state = state;
            i.clone()
        };
        self.persist(task);
        self.emit(TaskEvent::Upsert { task: snapshot });
    }

    /// Runs a task, fetching a fresh plan first when needed and once more if links expired mid-way.
    async fn drive(&self, task: &Task, cancel: &CancellationToken) -> AppResult<PathBuf> {
        let mut refreshed = false;
        loop {
            if task.plan.lock().unwrap().is_none() {
                self.set_state(task, TaskState::Queued);
                self.refresh_plan(task, cancel).await?;
                refreshed = true;
            }
            match self.run(task, cancel).await {
                Err(AppError::Expired(code)) if !refreshed => {
                    tracing::info!(code, "media links expired; re-analyzing source and resuming");
                    *task.plan.lock().unwrap() = None;
                }
                other => return other,
            }
        }
    }

    async fn run(&self, task: &Task, cancel: &CancellationToken) -> AppResult<PathBuf> {
        let _permit = tokio::select! {
            p = self.sem.clone().acquire_owned() => p.map_err(|_| AppError::Cancelled)?,
            _ = cancel.cancelled() => return Err(AppError::Cancelled),
        };
        self.set_state(task, TaskState::Downloading);
        *task.speed.lock().unwrap() = Speed::new();

        let plan = task.plan.lock().unwrap().clone().ok_or_else(|| AppError::NotFound("missing download plan".into()))?;
        let (id, title, source_url, option_label) = {
            let i = task.info.lock().unwrap();
            (i.id.clone(), i.title.clone(), i.source_url.clone(), i.option_label.clone())
        };
        let settings = self.settings();
        // A resumed task keeps the folder it started in, even if the setting changed since.
        let dir = task.work_dir.lock().unwrap().clone().unwrap_or_else(|| settings.download_dir.clone());
        tokio::fs::create_dir_all(&dir).await.map_err(|e| match AppError::from(e) {
            AppError::Io(m) => AppError::Permission(format!("can't create the download folder ({m})")),
            other => other,
        })?;
        *task.work_dir.lock().unwrap() = Some(dir.clone());
        let stem = fsname::sanitize_stem(&title);

        let final_path = {
            let mut reserved = self.reserved.lock().unwrap();
            let previous = task.final_path.lock().unwrap().clone();
            // Reuse the earlier name unless something else has taken it (a gallery folder is ours to continue).
            let reuse = previous.filter(|p| (plan.gallery && p.is_dir()) || !p.exists());
            let p = match reuse {
                Some(p) => p,
                None if plan.gallery => fsname::unique_dir(&dir, &stem, &|p: &Path| reserved.contains(p)),
                None => fsname::unique_path(&dir, &stem, &plan.out_ext, &|p: &Path| reserved.contains(p)),
            };
            if !fsname::is_within(&dir, &p) {
                return Err(AppError::Permission("refusing to write outside the download folder".into()));
            }
            reserved.insert(p.clone());
            p
        };
        *task.final_path.lock().unwrap() = Some(final_path.clone());
        self.persist(task);

        let prefix = temp_prefix(&id);
        // The real extension stays last so FFmpeg can infer the container.
        let temp = |i: usize, ext: &str| dir.join(format!("{prefix}{i}.part.{ext}"));
        let state = |i: usize| dir.join(format!("{prefix}{i}.state.json"));

        let result = self.execute(task, &plan, &source_url, &final_path, &temp, &state, cancel, &settings).await;
        self.reserved.lock().unwrap().remove(&final_path);

        let keep = match &result {
            Ok(_) => false,
            Err(AppError::Cancelled) => task.pause_requested.load(Ordering::Relaxed),
            Err(e) => keep_partial_after(e),
        };
        if !keep {
            remove_temps(&dir, &id);
            if plan.gallery && result.is_err() && !task.pause_requested.load(Ordering::Relaxed) {
                if let Ok(rd) = std::fs::read_dir(&final_path) {
                    for e in rd.flatten().filter(|e| e.file_name().to_string_lossy().starts_with(".part.")) {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
        }
        let path = result?;

        let size = file_or_dir_size(&path).await;
        let info = task.info.lock().unwrap().clone();
        let _ = self.store.add(&NewLibraryItem {
            source_url: &source_url,
            platform: info.platform.as_str(),
            title: &info.title,
            uploader: info.uploader.as_deref(),
            kind: match info.kind {
                MediaKind::Video => "video",
                MediaKind::Audio => "audio",
                MediaKind::Image => "image",
                MediaKind::Gallery => "gallery",
            },
            option_label: &option_label,
            container: &info.container,
            file_path: &path,
            file_size: size,
            duration: task.duration,
        });
        Ok(path)
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute(
        &self,
        task: &Task,
        plan: &Plan,
        source_url: &str,
        final_path: &Path,
        temp: &(dyn Fn(usize, &str) -> PathBuf + Sync),
        state: &(dyn Fn(usize) -> PathBuf + Sync),
        cancel: &CancellationToken,
        settings: &Settings,
    ) -> AppResult<PathBuf> {
        let counters: Vec<Arc<Counters>> = plan.streams.iter().map(|_| Arc::new(Counters::default())).collect();
        *task.counters.lock().unwrap() = counters.clone();

        if plan.gallery {
            tokio::fs::create_dir_all(final_path).await?;
            for (i, s) in plan.streams.iter().enumerate() {
                let name = s.item_name.clone().unwrap_or_else(|| format!("{:02}.{}", i + 1, s.ext));
                let out = final_path.join(&name);
                if let Ok(m) = tokio::fs::metadata(&out).await {
                    // Finished in an earlier run.
                    counters[i].total.store(m.len(), Ordering::Relaxed);
                    counters[i].done.store(m.len(), Ordering::Relaxed);
                    continue;
                }
                let part = final_path.join(format!(".part.{name}"));
                let st = final_path.join(format!(".part.{name}.state.json"));
                self.fetch_stream(s, source_url, &part, Some(&st), &counters[i], cancel, settings.max_segments).await?;
                tokio::fs::rename(&part, &out).await?;
                let _ = tokio::fs::remove_file(&st).await;
            }
            return Ok(final_path.to_path_buf());
        }

        // yt-dlp-managed (fragmented) plans produce the final container themselves; yt-dlp
        // resumes its own .part files when restarted with the same output name.
        if let [StreamPlan { transfer: Transfer::YtDlp { format_spec }, .. }] = plan.streams.as_slice() {
            // temp() ends in ".part.ytdlp"; strip both so yt-dlp appends the real extension.
            let stem_tmp = temp(0, "ytdlp").with_extension("").with_extension("");
            let merge = format_spec.contains('+').then_some(plan.out_ext.as_str());
            let produced = ytdlp_dl::download(&self.tools(), source_url, format_spec, &stem_tmp, merge, &counters[0], cancel).await?;
            self.set_state(task, TaskState::Verifying);
            self.verify(task, plan, &produced, cancel).await?;
            let ext = produced.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| plan.out_ext.clone());
            let out = final_path.with_extension(ext);
            tokio::fs::rename(&produced, &out).await?;
            return Ok(out);
        }

        // Native streams in parallel (e.g. video + audio), each with its own resume state.
        let mut parts = Vec::new();
        let mut jobs = Vec::new();
        for (i, s) in plan.streams.iter().enumerate() {
            let part = temp(i, &s.ext);
            let st = state(i);
            parts.push(part.clone());
            let c = counters[i].clone();
            jobs.push(async move { self.fetch_stream(s, source_url, &part, Some(&st), &c, cancel, settings.max_segments).await });
        }
        let results = futures_util::future::join_all(jobs).await;
        // Surface a pause/cancel ahead of secondary errors from sibling streams.
        if results.iter().any(|r| matches!(r, Err(AppError::Cancelled))) {
            return Err(AppError::Cancelled);
        }
        for r in results {
            r?;
        }

        let produced = match plan.post {
            Post::None => parts[0].clone(),
            Post::Merge => {
                self.set_state(task, TaskState::Processing);
                let out = temp(9, &plan.out_ext);
                post::merge(&self.tools(), &parts[0], &parts[1], &out, cancel).await?;
                out
            }
            Post::ToMp3 => {
                self.set_state(task, TaskState::Processing);
                let out = temp(9, "mp3");
                post::to_mp3(&self.tools(), &parts[0], &out, cancel).await?;
                out
            }
        };
        self.set_state(task, TaskState::Verifying);
        self.verify(task, plan, &produced, cancel).await?;
        tokio::fs::rename(&produced, final_path).await?;
        Ok(final_path.to_path_buf())
    }

    async fn verify(&self, task: &Task, plan: &Plan, file: &Path, cancel: &CancellationToken) -> AppResult<()> {
        let kind = task.info.lock().unwrap().kind;
        let ext = file.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let is_sub = matches!(ext.as_str(), "vtt" | "srt" | "ass" | "json3");
        let is_image = matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "gif" | "avif" | "heic");
        if is_sub || is_image {
            let len = tokio::fs::metadata(file).await?.len();
            return if len > 0 { Ok(()) } else { Err(AppError::Verification("the file is empty".into())) };
        }
        let audio_only = plan.post == Post::ToMp3 || kind == MediaKind::Audio || matches!(ext.as_str(), "mp3" | "m4a" | "opus" | "weba" | "ogg");
        // Require audio only where the source declared it; silent sources stay valid.
        let need_audio = audio_only || plan.expect_audio == Some(true);
        post::verify(&self.tools(), file, plan.expected_duration, !audio_only, need_audio, plan.min_short_side, cancel).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn fetch_stream(
        &self,
        s: &StreamPlan,
        source_url: &str,
        dest: &Path,
        state: Option<&Path>,
        counters: &Arc<Counters>,
        cancel: &CancellationToken,
        max_segments: usize,
    ) -> AppResult<()> {
        match &s.transfer {
            Transfer::Native { url, headers, mirrors } => {
                let urls: Vec<&str> = std::iter::once(url.as_str()).chain(mirrors.iter().map(String::as_str)).collect();
                http::fetch_mirrored(&self.client, &urls, &http::build_headers(headers), max_segments, dest, counters.clone(), cancel, state).await.map(|_| ())
            }
            Transfer::YtDlp { format_spec } => {
                let stem = dest.with_extension("");
                let produced = ytdlp_dl::download(&self.tools(), source_url, format_spec, &stem, None, counters, cancel).await?;
                tokio::fs::rename(&produced, dest).await?;
                Ok(())
            }
        }
    }

    // ---------- user actions ----------

    pub fn pause(&self, id: &str) -> AppResult<()> {
        let t = self.task(id)?;
        if t.info.lock().unwrap().state.active() {
            t.pause_requested.store(true, Ordering::Relaxed);
            t.cancel.lock().unwrap().cancel();
        }
        Ok(())
    }

    pub fn cancel(&self, id: &str) -> AppResult<()> {
        let t = self.task(id)?;
        let state = t.info.lock().unwrap().state;
        if state.active() {
            t.pause_requested.store(false, Ordering::Relaxed);
            t.cancel.lock().unwrap().cancel();
        } else if state == TaskState::Paused {
            // Not running: discard its partial data now.
            if let Some(dir) = t.work_dir.lock().unwrap().clone() {
                remove_temps(&dir, id);
            }
            t.info.lock().unwrap().done = 0;
            self.set_state(&t, TaskState::Cancelled);
        }
        Ok(())
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let t = self.task(id)?;
        t.removed.store(true, Ordering::Relaxed);
        if t.info.lock().unwrap().state.active() {
            t.pause_requested.store(false, Ordering::Relaxed);
            t.cancel.lock().unwrap().cancel(); // the run cleans up its temp files as it stops
        } else if let Some(dir) = t.work_dir.lock().unwrap().clone() {
            remove_temps(&dir, id);
        }
        self.tasks.lock().unwrap().retain(|(i, _)| i != id);
        let _ = self.store.delete_task(id);
        self.emit(TaskEvent::Removed { id: id.to_string() });
        Ok(())
    }

    /// Resumes a paused task, or retries a failed/cancelled one.
    pub fn resume(self: &Arc<Self>, id: &str) -> AppResult<()> {
        let t = self.task(id)?;
        let state = t.info.lock().unwrap().state;
        if state.active() || state == TaskState::Completed {
            return Ok(());
        }
        if state == TaskState::Failed {
            // Stream links are probably stale after a failure; analyze again.
            *t.plan.lock().unwrap() = None;
        }
        t.pause_requested.store(false, Ordering::Relaxed);
        *t.cancel.lock().unwrap() = CancellationToken::new();
        {
            let mut i = t.info.lock().unwrap();
            i.state = TaskState::Queued;
            i.error = None;
            i.interrupted = false;
            if state == TaskState::Cancelled {
                i.done = 0;
            }
        }
        self.persist(&t);
        self.emit(TaskEvent::Upsert { task: t.info.lock().unwrap().clone() });
        self.spawn_run(t);
        Ok(())
    }

    pub fn pause_all(&self) {
        let ids: Vec<String> = self.tasks.lock().unwrap().iter().filter(|(_, t)| t.info.lock().unwrap().state.active()).map(|(i, _)| i.clone()).collect();
        for id in ids {
            let _ = self.pause(&id);
        }
    }

    pub fn resume_all(self: &Arc<Self>) {
        let ids: Vec<String> = self.tasks.lock().unwrap().iter().filter(|(_, t)| t.info.lock().unwrap().state == TaskState::Paused).map(|(i, _)| i.clone()).collect();
        for id in ids {
            let _ = self.resume(&id);
        }
    }

    pub fn task_path(&self, id: &str) -> AppResult<PathBuf> {
        let t = self.task(id)?;
        let p = t.info.lock().unwrap().file_path.clone();
        p.map(PathBuf::from).ok_or_else(|| AppError::NotFound("The file isn't ready yet.".into()))
    }

    // ---------- progress ----------

    async fn progress_loop(self: Arc<Self>) {
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let active: Vec<Arc<Task>> = self
                .tasks
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, t)| matches!(t.info.lock().unwrap().state, TaskState::Downloading | TaskState::Processing))
                .map(|(_, t)| t.clone())
                .collect();
            for t in active {
                let counters = t.counters.lock().unwrap().clone();
                if counters.is_empty() {
                    continue;
                }
                let done: u64 = counters.iter().map(|c| c.done.load(Ordering::Relaxed)).sum();
                let totals: Vec<u64> = counters.iter().map(|c| c.total.load(Ordering::Relaxed)).collect();
                let total = if totals.iter().all(|x| *x > 0) { Some(totals.iter().sum::<u64>()) } else { t.info.lock().unwrap().total };
                let segments = if counters.len() == 1 { counters[0].segment_fractions() } else { Vec::new() };

                let (speed, eta) = {
                    let mut sp = t.speed.lock().unwrap();
                    if sp.samples == 0 && sp.last_done == 0 {
                        // Don't count bytes restored from a previous run as speed.
                        sp.last_done = done;
                    }
                    let dt = sp.last_at.elapsed().as_secs_f64();
                    if dt >= 0.5 {
                        let inst = done.saturating_sub(sp.last_done) as f64 / dt;
                        sp.ewma = if sp.samples == 0 { inst } else { 0.3 * inst + 0.7 * sp.ewma };
                        sp.samples += 1;
                        sp.last_done = done;
                        sp.last_at = Instant::now();
                    }
                    let eta = match total {
                        Some(tot) if sp.samples >= 4 && sp.ewma > 1.0 => Some(tot.saturating_sub(done) as f64 / sp.ewma),
                        _ => None,
                    };
                    (sp.ewma, eta)
                };
                {
                    let mut i = t.info.lock().unwrap();
                    i.done = done;
                    i.total = total.or(i.total);
                }
                self.emit(TaskEvent::Progress { id: t.id(), done, total, speed, eta, segments });
            }
        }
    }
}

async fn file_or_dir_size(p: &Path) -> u64 {
    match tokio::fs::metadata(p).await {
        Ok(m) if m.is_file() => m.len(),
        Ok(_) => {
            let mut total = 0;
            if let Ok(mut rd) = tokio::fs::read_dir(p).await {
                while let Ok(Some(e)) = rd.next_entry().await {
                    total += e.metadata().await.map(|m| m.len()).unwrap_or(0);
                }
            }
            total
        }
        Err(_) => 0,
    }
}

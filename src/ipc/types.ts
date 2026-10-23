// Mirrors the serde types in src-tauri/src (camelCase). Keep in sync with the Rust structs.

export type Platform =
  | "youtube" | "tiktok" | "douyin" | "bilibili" | "instagram" | "facebook"
  | "twitter" | "pinterest" | "kuaishou" | "rednote" | "suno" | "other";

export type MediaKind = "video" | "audio" | "image" | "gallery";
export type OptionGroup = "video" | "audio" | "image" | "subtitle";

export interface ErrorInfo {
  kind: string;
  message: string;
  detail: string | null;
  retryable: boolean;
}

export interface DetectedUrl {
  url: string;
  platform: Platform;
  platformName: string;
  error: string | null;
}

export interface DownloadOption {
  id: string;
  group: OptionGroup;
  label: string;
  container: string;
  codec: string;
  fps: number | null;
  width: number | null;
  height: number | null;
  size: number | null;
  sizeEstimated: boolean;
  note: string;
  conversion: boolean;
  recommended: boolean;
}

export interface MediaInfo {
  analysisId: string;
  sourceUrl: string;
  platform: Platform;
  platformName: string;
  extractor: string;
  id: string;
  title: string;
  uploader: string | null;
  duration: number | null;
  uploadDate: string | null;
  viewCount: number | null;
  thumbnail: string | null;
  kind: MediaKind;
  itemCount: number | null;
  options: DownloadOption[];
  elapsedMs: number;
  /** Set when a signed-in session was used, or why a guest fallback happened. */
  session: string | null;
}

export type TaskState = "queued" | "downloading" | "processing" | "verifying" | "paused" | "completed" | "failed" | "cancelled";

export interface TaskInfo {
  id: string;
  title: string;
  uploader: string | null;
  sourceUrl: string;
  platform: Platform;
  platformName: string;
  optionId: string;
  optionLabel: string;
  container: string;
  kind: MediaKind;
  thumbnail: string | null;
  state: TaskState;
  error: ErrorInfo | null;
  filePath: string | null;
  done: number;
  total: number | null;
  createdAt: number;
  /** Was running when Rabbit last exited. */
  interrupted: boolean;
}

export interface Progress {
  done: number;
  total: number | null;
  speed: number;
  eta: number | null;
  segments: number[];
}

export type TaskEvent =
  | { type: "upsert"; task: TaskInfo }
  | ({ type: "progress"; id: string } & Progress)
  | { type: "removed"; id: string };

export interface LibraryItem {
  id: number;
  sourceUrl: string;
  platform: Platform;
  title: string;
  uploader: string | null;
  kind: MediaKind;
  optionLabel: string;
  container: string;
  filePath: string;
  fileSize: number;
  duration: number | null;
  completedAt: number;
  fileExists: boolean;
}

export interface Settings {
  downloadDir: string;
  maxConcurrent: number;
  maxSegments: number;
  notifications: boolean;
  ytdlpChannel: UpdateChannel;
  sessions: Sessions;
}

export type SessionSite = "bilibili" | "douyin";

export type CookieSource =
  | { kind: "firefoxProfile"; path: string }
  | { kind: "netscapeFile"; path: string };

/** Opt-in signed-in sessions: which source and which sites. Never cookie values. */
export interface Sessions {
  enabled: boolean;
  source: CookieSource | null;
  bilibili: boolean;
  douyin: boolean;
}

export interface FirefoxProfile {
  name: string;
  path: string;
  isDefault: boolean;
  hasCookies: boolean;
}

export interface SessionStatus {
  site: SessionSite;
  state: "signedIn" | "signedOut" | "noCookies";
  account: string | null;
  detail: string;
}

export type LibrarySort = "newest" | "oldest" | "largest" | "title";

export interface LibraryQuery {
  text: string;
  platform: string | null;
  kind: string | null;
  sort: LibrarySort;
  offset: number;
}

export interface LibraryFacets {
  total: number;
  platforms: { value: string; count: number }[];
  kinds: { value: string; count: number }[];
}

export type UpdateChannel = "stable" | "nightly";

export interface InstalledYtDlp {
  version: string;
  file: string;
  sha256: string;
  channel: UpdateChannel;
  installedAt: number;
}

export interface YtDlpStatus {
  version: string | null;
  source: "bundled" | "update";
  update: InstalledYtDlp | null;
  channel: UpdateChannel;
  path: string | null;
}

export interface UpdateCheck {
  channel: UpdateChannel;
  current: string | null;
  latest: string;
  publishedAt: string | null;
  available: boolean;
}

export type UpdateEvent = { type: "progress"; done: number; total: number | null };

export interface ToolStatus {
  tools: { ytdlp: string | null; ffmpeg: string | null; ffprobe: string | null; jsRuntime: string | null };
  version: string;
}

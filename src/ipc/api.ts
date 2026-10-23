import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  DetectedUrl, ErrorInfo, FirefoxProfile, LibraryFacets, LibraryItem, LibraryQuery, MediaInfo, SessionSite, SessionStatus, Settings,
  TaskEvent, TaskInfo, ToolStatus, UpdateChannel, UpdateCheck, UpdateEvent, YtDlpStatus,
} from "./types";

export const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Normalises anything thrown by `invoke` into Rabbit's ErrorInfo shape. */
export function toError(e: unknown): ErrorInfo {
  if (e && typeof e === "object" && "message" in e && "kind" in e) return e as ErrorInfo;
  return { kind: "unknown", message: typeof e === "string" ? e : String((e as Error)?.message ?? e), detail: null, retryable: false };
}

export const api = {
  subscribe(onEvent: (e: TaskEvent) => void) {
    const ch = new Channel<TaskEvent>();
    ch.onmessage = onEvent;
    return invoke<TaskInfo[]>("subscribe", { channel: ch });
  },
  detectInput: (text: string) => invoke<DetectedUrl[]>("detect_input", { text }),
  analyze: (input: string) => invoke<MediaInfo>("analyze", { input }),
  cancelAnalyze: () => invoke<void>("cancel_analyze"),
  enqueue: (analysisId: string, optionId: string) => invoke<TaskInfo>("enqueue", { analysisId, optionId }),
  cancelTask: (id: string) => invoke<void>("cancel_task", { id }),
  removeTask: (id: string) => invoke<void>("remove_task", { id }),
  pauseTask: (id: string) => invoke<void>("pause_task", { id }),
  /** Resumes a paused task, or retries a failed/cancelled one. */
  resumeTask: (id: string) => invoke<void>("resume_task", { id }),
  pauseAll: () => invoke<void>("pause_all"),
  resumeAll: () => invoke<void>("resume_all"),
  openTask: (id: string, reveal: boolean) => invoke<void>("open_task", { id, reveal }),
  libraryList: (query: LibraryQuery) => invoke<LibraryItem[]>("library_list", { query }),
  libraryFacets: () => invoke<LibraryFacets>("library_facets"),
  libraryOpen: (id: number, reveal: boolean) => invoke<void>("library_open", { id, reveal }),
  libraryRemove: (id: number, deleteFile: boolean) => invoke<void>("library_remove", { id, deleteFile }),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<Settings>("set_settings", { settings }),
  chooseDownloadDir: () => invoke<Settings | null>("choose_download_dir"),
  toolStatus: () => invoke<ToolStatus>("tool_status"),
  ytdlpStatus: () => invoke<YtDlpStatus>("ytdlp_status"),
  ytdlpCheck: (channel: UpdateChannel) => invoke<UpdateCheck>("ytdlp_check", { channel }),
  ytdlpInstall(channel: UpdateChannel, tag: string, onProgress: (e: UpdateEvent) => void) {
    const ch = new Channel<UpdateEvent>();
    ch.onmessage = onProgress;
    return invoke<YtDlpStatus>("ytdlp_install", { channel, tag, onProgress: ch });
  },
  ytdlpUseBundled: () => invoke<YtDlpStatus>("ytdlp_use_bundled"),
  listFirefoxProfiles: () => invoke<FirefoxProfile[]>("list_firefox_profiles"),
  chooseCookieFile: () => invoke<string | null>("choose_cookie_file"),
  sessionCheck: (site: SessionSite) => invoke<SessionStatus>("session_check", { site }),
};

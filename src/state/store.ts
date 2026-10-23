import { create } from "zustand";
import { api, toError } from "../ipc/api";
import type { DetectedUrl, ErrorInfo, MediaInfo, Progress, Settings, TaskEvent, TaskInfo } from "../ipc/types";

export type View = "home" | "queue" | "library" | "settings";

export interface Toast {
  id: number;
  tone: "ok" | "err" | "info";
  title: string;
  body?: string;
}

type Analysis =
  | { status: "idle" }
  | { status: "running"; startedAt: number }
  | { status: "done"; info: MediaInfo }
  | { status: "error"; error: ErrorInfo };

interface AppState {
  view: View;
  input: string;
  detected: DetectedUrl[];
  analysis: Analysis;
  tasks: Record<string, TaskInfo>;
  order: string[];
  progress: Record<string, Progress>;
  settings: Settings | null;
  toasts: Toast[];
  recent: { url: string; title: string; platform: string }[];

  setView(v: View): void;
  setInput(text: string): void;
  analyze(text?: string): Promise<void>;
  clearAnalysis(): void;
  download(optionId: string): Promise<boolean>;
  applyEvent(e: TaskEvent): void;
  loadTasks(list: TaskInfo[]): void;
  setSettings(s: Settings): void;
  toast(t: Omit<Toast, "id">): void;
  dismiss(id: number): void;
}

const RECENT_KEY = "rabbit.recent";
function loadRecent(): AppState["recent"] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
  } catch {
    return [];
  }
}
function saveRecent(r: AppState["recent"]) {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(r));
  } catch {
    /* storage unavailable: recent list is a convenience only */
  }
}

let toastSeq = 1;
let detectSeq = 0;

export const useApp = create<AppState>((set, get) => ({
  view: "home",
  input: "",
  detected: [],
  analysis: { status: "idle" },
  tasks: {},
  order: [],
  progress: {},
  settings: null,
  toasts: [],
  recent: loadRecent(),

  setView: (view) => set({ view }),

  setInput(text) {
    set({ input: text });
    const seq = ++detectSeq;
    if (!text.trim()) {
      set({ detected: [] });
      return;
    }
    api.detectInput(text).then((d) => {
      if (seq === detectSeq) set({ detected: d });
    }, () => {});
  },

  async analyze(text) {
    const input = text ?? get().input;
    if (text !== undefined) get().setInput(text);
    if (!input.trim()) return;
    set({ analysis: { status: "running", startedAt: performance.now() } });
    try {
      const info = await api.analyze(input);
      const recent = [{ url: info.sourceUrl, title: info.title, platform: info.platform }, ...get().recent.filter((r) => r.url !== info.sourceUrl)].slice(0, 6);
      saveRecent(recent);
      set({ analysis: { status: "done", info }, recent });
    } catch (e) {
      const error = toError(e);
      if (error.kind === "cancelled") return;
      set({ analysis: { status: "error", error } });
    }
  },

  clearAnalysis() {
    if (get().analysis.status === "running") api.cancelAnalyze().catch(() => {});
    set({ analysis: { status: "idle" } });
  },

  async download(optionId) {
    const a = get().analysis;
    if (a.status !== "done") return false;
    try {
      await api.enqueue(a.info.analysisId, optionId);
      return true;
    } catch (e) {
      const err = toError(e);
      get().toast({ tone: "err", title: "Couldn't start the download", body: err.message });
      return false;
    }
  },

  applyEvent(e) {
    if (e.type === "upsert") {
      const prev = get().tasks[e.task.id];
      set((s) => {
        // Live progress belongs to the running transfer; drop it once the task stops.
        const stopped = !isActive(e.task);
        const progress = stopped && s.progress[e.task.id] ? { ...s.progress } : s.progress;
        if (progress !== s.progress) delete progress[e.task.id];
        return {
          tasks: { ...s.tasks, [e.task.id]: e.task },
          order: s.order.includes(e.task.id) ? s.order : [e.task.id, ...s.order],
          progress,
        };
      });
      if (prev && prev.state !== e.task.state) {
        if (e.task.state === "completed") get().toast({ tone: "ok", title: "Download finished", body: e.task.title });
        if (e.task.state === "failed") get().toast({ tone: "err", title: "Download failed", body: e.task.error?.message });
      }
    } else if (e.type === "progress") {
      const { id, type: _t, ...p } = e;
      set((s) => ({ progress: { ...s.progress, [id]: p } }));
    } else if (e.type === "removed") {
      set((s) => {
        const tasks = { ...s.tasks };
        const progress = { ...s.progress };
        delete tasks[e.id];
        delete progress[e.id];
        return { tasks, progress, order: s.order.filter((x) => x !== e.id) };
      });
    }
  },

  loadTasks(list) {
    const tasks: Record<string, TaskInfo> = {};
    for (const t of list) tasks[t.id] = t;
    set({ tasks, order: list.map((t) => t.id).reverse() });
  },

  setSettings: (settings) => set({ settings }),

  toast(t) {
    const id = toastSeq++;
    set((s) => ({ toasts: [...s.toasts.slice(-2), { ...t, id }] }));
    if (t.tone !== "err") setTimeout(() => get().dismiss(id), 5000);
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

export const isActive = (t: TaskInfo) => t.state === "queued" || t.state === "downloading" || t.state === "processing" || t.state === "verifying";

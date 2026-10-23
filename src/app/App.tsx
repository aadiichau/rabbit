import { lazy, Suspense, useEffect, useState } from "react";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { api, isTauri } from "../ipc/api";
import { useApp } from "../state/store";
import { TitleBar } from "./TitleBar";
import { Sidebar } from "./Sidebar";
import { Toasts } from "./Toasts";
import { HomeView } from "../features/home/HomeView";
import { QueueView } from "../features/queue/QueueView";

const LibraryView = lazy(() => import("../features/library/LibraryView"));
const SettingsView = lazy(() => import("../features/settings/SettingsView"));

const isEditable = (el: Element | null) => !!el && (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || (el as HTMLElement).isContentEditable);

export function App() {
  const view = useApp((s) => s.view);
  const [dragging, setDragging] = useState(false);
  const [backendMissing] = useState(() => !isTauri());

  useEffect(() => {
    if (backendMissing) return;
    const { applyEvent, loadTasks, setSettings } = useApp.getState();
    api.subscribe(applyEvent).then(loadTasks);
    api.getSettings().then(setSettings);
  }, [backendMissing]);

  useEffect(() => {
    const onKey = async (e: KeyboardEvent) => {
      const s = useApp.getState();
      if (e.ctrlKey && !e.shiftKey && !e.altKey) {
        if (e.key === "1") s.setView("home");
        else if (e.key === "2") s.setView("queue");
        else if (e.key === "3") s.setView("library");
        else if (e.key === ",") s.setView("settings");
        else if (e.key.toLowerCase() === "f") {
          s.setView("library");
          setTimeout(() => document.getElementById("library-search")?.focus(), 50);
        } else if (e.key.toLowerCase() === "l") {
          s.setView("home");
          requestAnimationFrame(() => document.getElementById("url-input")?.focus());
        } else if (e.key.toLowerCase() === "v" && !isEditable(document.activeElement)) {
          e.preventDefault();
          const text = await readText().catch(() => "");
          if (text.trim()) {
            s.setView("home");
            s.analyze(text);
          }
        } else return;
        e.preventDefault();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Links dragged from a browser arrive as text/uri-list or text/plain.
  const onDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragging(false);
    const text = e.dataTransfer.getData("text/uri-list") || e.dataTransfer.getData("text/plain");
    if (text.trim()) {
      const s = useApp.getState();
      s.setView("home");
      s.analyze(text);
    }
  };

  return (
    <div
      className="grid h-full grid-rows-[var(--titlebar-h)_minmax(0,1fr)]"
      onDragOver={(e) => {
        if (e.dataTransfer.types.some((t) => t === "text/uri-list" || t === "text/plain")) {
          e.preventDefault();
          setDragging(true);
        }
      }}
      onDragLeave={(e) => {
        if (e.relatedTarget === null) setDragging(false);
      }}
      onDrop={onDrop}
    >
      <TitleBar />
      <div className="grid min-h-0 grid-cols-[var(--sidebar-w)_minmax(0,1fr)] grid-rows-[minmax(0,1fr)]">
        <Sidebar />
        <main className="relative min-h-0 min-w-0" key={view}>
          {backendMissing ? (
            <div className="p-6 text-fg-2">Rabbit's engine isn't connected. Start the app with <span className="mono">npm run tauri dev</span>.</div>
          ) : (
            <Suspense fallback={null}>
              {view === "home" && <HomeView />}
              {view === "queue" && <QueueView />}
              {view === "library" && <LibraryView />}
              {view === "settings" && <SettingsView />}
            </Suspense>
          )}
          {dragging && <div className="pointer-events-none absolute inset-2 rounded-md border border-dashed border-[var(--amber-line)] bg-[rgb(229_162_60/0.04)]" />}
        </main>
      </div>
      <Toasts />
    </div>
  );
}

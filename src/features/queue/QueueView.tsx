import { useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { Icon } from "../../icons/Icon";
import { api } from "../../ipc/api";
import type { TaskInfo } from "../../ipc/types";
import { isActive, useApp } from "../../state/store";
import { QueueRow } from "./QueueRow";

type Filter = "all" | "active" | "done" | "failed";
const FILTERS: { id: Filter; label: string; test: (t: TaskInfo) => boolean }[] = [
  { id: "all", label: "All", test: () => true },
  { id: "active", label: "Active", test: (t) => isActive(t) || t.state === "paused" },
  { id: "done", label: "Done", test: (t) => t.state === "completed" },
  { id: "failed", label: "Failed", test: (t) => t.state === "failed" },
];

export function QueueView() {
  const [filter, setFilter] = useState<Filter>("all");
  const setView = useApp((s) => s.setView);
  const tasks = useApp(useShallow((s) => s.order.map((id) => s.tasks[id]).filter(Boolean)));
  const visible = useMemo(() => tasks.filter(FILTERS.find((f) => f.id === filter)!.test).map((t) => t.id), [tasks, filter]);
  const finished = tasks.filter((t) => !isActive(t) && t.state !== "paused");
  const running = tasks.some(isActive);
  const paused = tasks.filter((t) => t.state === "paused");

  return (
    <div className="scroll rise h-full px-6 pb-6">
      <div className="flex items-center gap-3 pt-5 pb-4">
        <h1 className="text-lg font-semibold tracking-[-0.01em]">Queue</h1>
        <div className="ml-4 flex rounded border border-line-2 bg-bg-2 p-0.5" role="tablist">
          {FILTERS.map((f) => {
            const n = tasks.filter(f.test).length;
            return (
              <button
                key={f.id}
                role="tab"
                aria-selected={filter === f.id}
                onClick={() => setFilter(f.id)}
                className={`h-6 rounded-[3px] px-2.5 text-xs ${filter === f.id ? "bg-bg-4 text-fg-1" : "text-fg-3 hover:text-fg-1"}`}
              >
                {f.label} <span className="mono text-fg-4">{n}</span>
              </button>
            );
          })}
        </div>
        <div className="flex-1" />
        {running && (
          <button className="btn btn-ghost" onClick={() => api.pauseAll().catch(() => {})}>
            <Icon name="pause" />Pause all
          </button>
        )}
        {paused.length > 0 && (
          <button className="btn btn-secondary" onClick={() => api.resumeAll().catch(() => {})}>
            <Icon name="play" />Resume {paused.length > 1 ? `all ${paused.length}` : ""}
          </button>
        )}
        {finished.length > 0 && (
          <button className="btn btn-ghost" onClick={() => finished.forEach((t) => api.removeTask(t.id).catch(() => {}))}>
            Clear finished
          </button>
        )}
      </div>
      {visible.length === 0 ? (
        <div className="rounded-md border border-dashed border-line-2 p-5">
          <p className="text-fg-2">{tasks.length === 0 ? "Nothing downloading." : "Nothing matches this filter."}</p>
          <p className="mt-0.5 mb-3 text-xs text-fg-3">Paste a link on Home or drop one anywhere in the window.</p>
          <button className="btn btn-secondary" onClick={() => setView("home")}>Go to Home</button>
        </div>
      ) : (
        <div>{visible.map((id) => <QueueRow key={id} id={id} />)}</div>
      )}
    </div>
  );
}

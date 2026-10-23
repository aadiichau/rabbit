import type { TaskState } from "../ipc/types";

/** Rabbit's signature progress display: hairline track, 10 % ticks, optional live segment bars. */
export function Ruler({ done, total, state, segments }: { done: number; total: number | null; state: TaskState; segments?: number[] }) {
  const tone = state === "completed" || state === "verifying" ? "ok" : state === "failed" ? "err" : state === "cancelled" || state === "paused" ? "paused" : "";
  const known = total != null && total > 0;
  const pct = state === "completed" ? 100 : state === "cancelled" ? 0 : known ? Math.min(100, (done / total!) * 100) : 0;
  const indeterminate = !known && (state === "downloading" || state === "processing");
  return (
    <div>
      <div
        className={`ruler ${indeterminate ? "indet" : ""}`}
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={known ? Math.round(pct) : undefined}
      >
        <div className={`fill ${tone}`} style={indeterminate ? undefined : { width: `${pct}%` }} />
      </div>
      {segments && segments.length > 1 && state === "downloading" && (
        <div className="segs" aria-hidden="true">
          {segments.map((f, i) => (
            <i key={i}>
              <b style={{ width: `${Math.round(f * 100)}%` }} />
            </i>
          ))}
        </div>
      )}
    </div>
  );
}

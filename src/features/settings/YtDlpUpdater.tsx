import { useEffect, useRef, useState } from "react";
import { Icon } from "../../icons/Icon";
import { api, toError } from "../../ipc/api";
import type { ErrorInfo, UpdateChannel, UpdateCheck, YtDlpStatus } from "../../ipc/types";
import { Ruler } from "../../design/Ruler";
import { progressPair, relativeDate } from "../../lib/format";
import { useApp } from "../../state/store";

type Phase =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "checked"; result: UpdateCheck }
  | { kind: "installing"; tag: string; done: number; total: number | null }
  | { kind: "reverting" }
  | { kind: "error"; error: ErrorInfo };

const CHANNELS: { id: UpdateChannel; label: string }[] = [
  { id: "stable", label: "Stable" },
  { id: "nightly", label: "Nightly" },
];

/** User-triggered yt-dlp updates. Nothing here touches the network until a button is pressed. */
export function YtDlpUpdater({ onChanged }: { onChanged?: () => void }) {
  const savedChannel = useApp((s) => s.settings?.ytdlpChannel ?? "stable");
  const [status, setStatus] = useState<YtDlpStatus | null>(null);
  // Starts from the saved setting at once; the status call (which runs yt-dlp, ~1–2 s) must not
  // override a choice the user has already made.
  const [channel, setChannel] = useState<UpdateChannel>(savedChannel);
  const touched = useRef(false);
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const toast = useApp((s) => s.toast);

  useEffect(() => {
    api.ytdlpStatus().then((s) => {
      setStatus(s);
      if (!touched.current) setChannel(s.channel);
    }, () => {});
  }, []);

  const check = async () => {
    setPhase({ kind: "checking" });
    try {
      setPhase({ kind: "checked", result: await api.ytdlpCheck(channel) });
    } catch (e) {
      setPhase({ kind: "error", error: toError(e) });
    }
  };

  // Installs exactly what was checked: the tag *and* the channel it came from.
  const install = async (check: UpdateCheck) => {
    const tag = check.latest;
    setPhase({ kind: "installing", tag, done: 0, total: null });
    try {
      const s = await api.ytdlpInstall(check.channel, tag, (e) => setPhase({ kind: "installing", tag, done: e.done, total: e.total }));
      setStatus(s);
      setPhase({ kind: "idle" });
      onChanged?.();
      toast({ tone: "ok", title: `yt-dlp ${s.version} is now in use`, body: "Checksum verified. New analyses and downloads use it." });
    } catch (e) {
      setPhase({ kind: "error", error: toError(e) });
    }
  };

  const useBundled = async () => {
    // Reading the bundled version starts yt-dlp, which takes a few seconds on a cold start.
    setPhase({ kind: "reverting" });
    try {
      const s = await api.ytdlpUseBundled();
      setStatus(s);
      setPhase({ kind: "idle" });
      onChanged?.();
      toast({ tone: "info", title: `Back to the bundled yt-dlp ${s.version ?? ""}`.trim() });
    } catch (e) {
      setPhase({ kind: "error", error: toError(e) });
    }
  };

  const busy = phase.kind === "checking" || phase.kind === "installing" || phase.kind === "reverting";
  const verifying = phase.kind === "installing" && phase.total != null && phase.done >= phase.total;

  return (
    <div className="grid gap-3 border-t border-line-1 py-3.5">
      <div className="flex items-center gap-3">
        <span className="t-data w-20 shrink-0 text-fg-3">yt-dlp</span>
        <span className="mono text-fg-1">{status?.version ?? "—"}</span>
        {status && (
          <span className={`chip chip-sm ${status.source === "update" ? "chip-amber" : ""}`}>
            {status.source === "update" ? `updated · ${status.update?.channel}` : "bundled"}
          </span>
        )}
        {status?.update && <span className="t-meta">installed {relativeDate(status.update.installedAt)}</span>}
        <div className="flex-1" />
        {status?.source === "update" && (
          <button className="btn btn-ghost" disabled={busy} onClick={useBundled}>
            {phase.kind === "reverting" ? "Switching to bundled…" : "Use bundled version"}
          </button>
        )}
      </div>

      <div className="flex items-center gap-3 pl-[92px]">
        <div className="flex rounded border border-line-2 bg-bg-2 p-0.5" role="radiogroup" aria-label="Update channel">
          {CHANNELS.map((c) => (
            <button
              key={c.id}
              role="radio"
              aria-checked={channel === c.id}
              disabled={busy}
              onClick={() => {
                touched.current = true;
                setChannel(c.id);
                setPhase({ kind: "idle" });
              }}
              className={`h-6 rounded-[3px] px-2.5 text-xs ${channel === c.id ? "bg-bg-4 text-fg-1" : "text-fg-3 hover:text-fg-1"}`}
            >
              {c.label}
            </button>
          ))}
        </div>
        <button className="btn btn-secondary" disabled={busy} onClick={check}>
          <Icon name="retry" />{phase.kind === "checking" ? "Checking…" : "Check for updates"}
        </button>
        {phase.kind === "checked" && !phase.result.available && (
          <span className="flex items-center gap-1.5 text-xs text-ok"><Icon name="check" />Up to date — {phase.result.latest} is the latest {phase.result.channel} release</span>
        )}
        {phase.kind === "checked" && phase.result.available && (
          <>
            <span className="text-xs text-fg-2">
              <span className="mono text-fg-1">{phase.result.latest}</span> available
              {phase.result.publishedAt && <span className="text-fg-3"> · published {phase.result.publishedAt.slice(0, 10)}</span>}
            </span>
            <button className="btn btn-primary" onClick={() => install(phase.result)}>
              <Icon name="download" />Update
            </button>
          </>
        )}
      </div>

      {phase.kind === "installing" && (
        <div className="ml-[92px] max-w-md" aria-live="polite">
          <div className="t-data mb-1.5 flex justify-between text-fg-2">
            <span>{verifying ? "Verifying checksum and version…" : `Downloading yt-dlp ${phase.tag}`}</span>
            <span>{progressPair(phase.done, phase.total)}</span>
          </div>
          <Ruler done={phase.done} total={phase.total} state={verifying ? "verifying" : "downloading"} />
        </div>
      )}

      {phase.kind === "error" && (
        <div className="ml-[92px] flex items-start gap-2 text-xs leading-4 text-err">
          <Icon name="alert" />
          <span className="selectable">{phase.error.message}</span>
        </div>
      )}

      <p className="t-meta pl-[92px]">
        Updates come from github.com/yt-dlp and are checked against the release's SHA-256 checksums before use. Rabbit
        never checks or updates on its own. Nightly builds get site fixes sooner but are less tested.
      </p>
    </div>
  );
}

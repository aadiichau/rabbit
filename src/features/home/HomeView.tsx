import { useEffect, useRef, useState } from "react";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { useShallow } from "zustand/react/shallow";
import { Icon } from "../../icons/Icon";
import { MONOGRAM } from "../../lib/format";
import { isActive, useApp } from "../../state/store";
import { QueueRow } from "../queue/QueueRow";
import { Preview } from "./Preview";

function Elapsed({ since }: { since: number }) {
  const [now, setNow] = useState(performance.now());
  useEffect(() => {
    const t = setInterval(() => setNow(performance.now()), 100);
    return () => clearInterval(t);
  }, []);
  return <span>{((now - since) / 1000).toFixed(1)} s</span>;
}

function HeroInput() {
  const input = useApp((s) => s.input);
  const detected = useApp((s) => s.detected);
  const analysis = useApp((s) => s.analysis);
  const { setInput, analyze, clearAnalysis } = useApp.getState();
  const ref = useRef<HTMLInputElement>(null);
  const running = analysis.status === "running";
  const first = detected[0];
  const invalid = first?.error;

  useEffect(() => {
    if (analysis.status === "idle") ref.current?.focus();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const paste = async () => {
    const text = await readText().catch(() => "");
    if (text.trim()) analyze(text);
  };

  return (
    <div>
      <div className={`field relative flex h-12 items-center gap-2.5 pr-1.5 pl-2 ${invalid ? "field-invalid" : ""}`}>
        {first && !invalid ? (
          <span className={`chip rise ${first.platform === "other" ? "chip-info" : "chip-amber"}`} title={first.platformName}>
            {MONOGRAM[first.platform]}
          </span>
        ) : (
          <span className="grid w-7 place-items-center text-fg-3"><Icon name="link" /></span>
        )}
        <input
          id="url-input"
          ref={ref}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") analyze();
            if (e.key === "Escape") {
              setInput("");
              clearAnalysis();
            }
          }}
          spellCheck={false}
          autoComplete="off"
          placeholder="Paste a link or share text"
          aria-label="Media link"
          aria-invalid={!!invalid}
          className="mono h-full min-w-0 flex-1 bg-transparent text-[15px] leading-6 outline-none placeholder:text-fg-3"
        />
        {detected.length > 1 && <span className="t-data shrink-0 text-fg-3">{detected.length} links · first is analyzed</span>}
        {input && !running && (
          <button className="btn btn-icon" aria-label="Clear" title="Clear (Esc)" onClick={() => { setInput(""); clearAnalysis(); ref.current?.focus(); }}>
            <Icon name="x" />
          </button>
        )}
        <button className="btn btn-ghost" onClick={paste}>
          <Icon name="paste" />Paste <kbd>Ctrl V</kbd>
        </button>
        {running ? (
          <button className="btn btn-secondary" onClick={clearAnalysis}>Stop</button>
        ) : (
          <button className="btn btn-primary" onClick={() => analyze()} disabled={!input.trim() || !!invalid}>
            Analyze <kbd>↵</kbd>
          </button>
        )}
        {running && <div className="sweep" />}
      </div>
      <div className="t-data mt-2 flex min-h-4 gap-4 text-fg-3" aria-live="polite">
        {invalid && <span className="flex items-center gap-1.5 font-sans text-err"><Icon name="alert" />{invalid}</span>}
        {!invalid && first && <span><span className="text-ok">✓</span> Detected {first.platformName}{first.platform === "other" ? " — will try the generic extractor" : ""}</span>}
        {running && <span>Reading metadata ·<Elapsed since={analysis.startedAt} /></span>}
        {analysis.status === "done" && (
          <span><span className="text-ok">✓</span> {analysis.info.options.length} options · {(analysis.info.elapsedMs / 1000).toFixed(2)} s · {analysis.info.extractor}</span>
        )}
        {analysis.status === "done" && analysis.info.session && (
          <span className={analysis.info.session.startsWith("guest") ? "text-warn" : "text-amber"} title="Settings → Signed-in sites">· {analysis.info.session}</span>
        )}
      </div>
    </div>
  );
}

function AnalysisError() {
  const analysis = useApp((s) => s.analysis);
  if (analysis.status !== "error") return null;
  const e = analysis.error;
  return (
    <div className="panel rise mt-4 p-4">
      <div className="flex items-center gap-2 font-medium text-err"><Icon name="alert" />{e.kind === "unsupported" ? "Not supported" : e.kind === "loginRequired" ? "Sign-in required" : "Couldn't analyze this link"}</div>
      <p className="selectable mt-1 text-fg-2">{e.message}</p>
      {e.detail && <pre className="selectable mono mt-2 whitespace-pre-wrap text-xs text-fg-3">{e.detail}</pre>}
    </div>
  );
}

function PreviewSkeleton() {
  return (
    <div className="panel mt-4 grid grid-cols-[300px_1fr] overflow-hidden">
      <div className="skel aspect-video rounded-none" />
      <div className="flex flex-col gap-3 p-5">
        <div className="skel h-3 w-24" />
        <div className="skel h-5 w-3/4" />
        <div className="skel h-3 w-1/3" />
        <div className="mt-4 grid gap-2">{[0, 1, 2].map((i) => <div key={i} className="skel h-6" />)}</div>
      </div>
    </div>
  );
}

function Recent() {
  const recent = useApp((s) => s.recent);
  const analyze = useApp((s) => s.analyze);
  if (!recent.length) {
    return (
      <div className="mt-10 text-fg-3">
        <div className="t-micro mb-2">Works with</div>
        <p className="max-w-xl text-xs leading-5">
          YouTube, Bilibili, Kuaishou, RedNote, Instagram, Facebook, X, Pinterest, Suno and other sites supported by yt-dlp. Public content —
          plus, only if you switch it on, your own Bilibili or Douyin session. Rabbit never asks for passwords or bypasses access controls.
          Drop a link anywhere in the window, or press <kbd>Ctrl V</kbd>.
        </p>
      </div>
    );
  }
  return (
    <div className="mt-8">
      <div className="t-micro mb-1.5">Recent</div>
      {recent.map((r) => (
        <button key={r.url} onClick={() => analyze(r.url)} className="flex h-8 w-full items-center gap-3 border-t border-line-1 px-1 text-left hover:bg-bg-2">
          <span className="chip chip-sm">{MONOGRAM[r.platform] ?? "··"}</span>
          <span className="truncate text-fg-2">{r.title}</span>
          <span className="t-data ml-auto max-w-[40%] shrink-0 truncate text-fg-4">{r.url.replace(/^https?:\/\/(www\.)?/, "")}</span>
        </button>
      ))}
    </div>
  );
}

export function HomeView() {
  const status = useApp((s) => s.analysis.status);
  const setView = useApp((s) => s.setView);
  const active = useApp(useShallow((s) => s.order.filter((id) => s.tasks[id] && isActive(s.tasks[id]))));

  return (
    <div className="scroll rise h-full px-6 pb-6">
      <div className="flex items-baseline gap-3 pt-5 pb-4">
        <h1 className="text-lg font-semibold tracking-[-0.01em]">Analyze a link</h1>
        <span className="t-meta">Paste a URL or share text — detection happens locally.</span>
      </div>
      <HeroInput />
      {status === "running" && <PreviewSkeleton />}
      {status === "done" && <Preview />}
      <AnalysisError />
      {active.length > 0 && (
        <div className="mt-6">
          <div className="mb-1.5 flex items-baseline justify-between">
            <span className="t-micro">In progress</span>
            <button className="text-xs text-fg-3 hover:text-fg-1" onClick={() => setView("queue")}>Open queue →</button>
          </div>
          {active.slice(0, 3).map((id) => <QueueRow key={id} id={id} />)}
        </div>
      )}
      {(status === "idle" || status === "error") && <Recent />}
    </div>
  );
}

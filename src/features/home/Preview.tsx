import { useEffect, useMemo, useState } from "react";
import { Icon, type IconName } from "../../icons/Icon";
import { api, toError } from "../../ipc/api";
import type { DownloadOption, OptionGroup } from "../../ipc/types";
import { bytes, clock, compactCount } from "../../lib/format";
import { useApp } from "../../state/store";

const TABS: { group: OptionGroup; label: string; icon: IconName }[] = [
  { group: "video", label: "Video", icon: "video" },
  { group: "audio", label: "Audio", icon: "audio" },
  { group: "image", label: "Images", icon: "image" },
  { group: "subtitle", label: "Subtitles", icon: "subtitle" },
];

function sizeText(o: DownloadOption) {
  if (o.size == null) return o.conversion ? "after conversion" : "—";
  return `${o.sizeEstimated ? "≈ " : ""}${bytes(o.size)}`;
}

export function Preview() {
  const analysis = useApp((s) => s.analysis);
  const settings = useApp((s) => s.settings);
  const { download, setSettings, toast } = useApp.getState();
  const info = analysis.status === "done" ? analysis.info : null;

  const groups = useMemo(() => TABS.filter((t) => info?.options.some((o) => o.group === t.group)), [info]);
  const initial = info?.options.find((o) => o.recommended) ?? info?.options[0];
  const [tab, setTab] = useState<OptionGroup>(initial?.group ?? "video");
  const [selected, setSelected] = useState<string | undefined>(initial?.id);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setTab(initial?.group ?? "video");
    setSelected(initial?.id);
  }, [info?.analysisId]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!info) return null;
  const rows = info.options.filter((o) => o.group === tab);
  const chosen = info.options.find((o) => o.id === selected);

  const start = async () => {
    if (!chosen) return;
    setBusy(true);
    const ok = await download(chosen.id);
    setBusy(false);
    if (ok) toast({ tone: "info", title: "Added to queue", body: `${chosen.label} · ${info.title}` });
  };

  const changeDir = async () => {
    try {
      const s = await api.chooseDownloadDir();
      if (s) setSettings(s);
    } catch (e) {
      toast({ tone: "err", title: "Couldn't change the folder", body: toError(e).message });
    }
  };

  const showFps = rows.some((o) => o.fps);
  return (
    <section className="panel rise mt-4 grid grid-cols-[300px_minmax(0,1fr)] overflow-hidden">
      <div className="border-r border-line-1">
        <div className="relative aspect-video bg-bg-0">
          {info.thumbnail ? (
            <img src={info.thumbnail} alt="" className="h-full w-full object-contain" draggable={false} />
          ) : (
            <div className="grid h-full place-items-center text-fg-4"><Icon name={info.kind === "audio" ? "audio" : "image"} size={20} /></div>
          )}
          {info.duration != null && <span className="mono absolute right-2 bottom-2 rounded-[2px] bg-[rgb(13_14_16/0.85)] px-[5px] text-[11px] leading-4 font-medium">{clock(info.duration)}</span>}
          {info.itemCount != null && <span className="mono absolute right-2 bottom-2 rounded-[2px] bg-[rgb(13_14_16/0.85)] px-[5px] text-[11px] leading-4 font-medium">{info.itemCount} items</span>}
        </div>
        <dl className="t-data grid grid-cols-[auto_1fr] gap-x-4 gap-y-0.5 border-t border-line-1 px-3.5 py-3 text-fg-3">
          <dt>source</dt><dd className="truncate text-right text-fg-2">{info.platformName}</dd>
          {info.uploadDate && (<><dt>published</dt><dd className="text-right text-fg-2">{info.uploadDate}</dd></>)}
          {info.id && (<><dt>id</dt><dd className="selectable truncate text-right text-fg-2">{info.id}</dd></>)}
          <dt>extractor</dt><dd className="truncate text-right text-fg-2">{info.extractor}</dd>
        </dl>
      </div>

      <div className="flex min-w-0 flex-col">
        <div className="border-b border-line-1 px-5 pt-4 pb-3">
          <div className="t-micro flex items-center gap-2">
            <span className="chip">{info.kind.toUpperCase()}</span>
            {info.platformName}
          </div>
          <h2 className="selectable mt-2 line-clamp-2 text-xl leading-[26px] font-semibold tracking-[-0.015em]" title={info.title}>{info.title}</h2>
          <div className="mt-1 text-fg-2">
            {info.uploader ?? "Unknown creator"}
            <span className="t-data text-fg-3">
              {info.duration != null && ` · ${clock(info.duration)}`}
              {info.viewCount != null && ` · ${compactCount(info.viewCount)} views`}
            </span>
          </div>
        </div>

        <div className="flex gap-5 border-b border-line-1 px-5" role="tablist">
          {groups.map((t) => {
            const n = info.options.filter((o) => o.group === t.group).length;
            const on = tab === t.group;
            return (
              <button
                key={t.group}
                role="tab"
                aria-selected={on}
                onClick={() => {
                  setTab(t.group);
                  const pick = info.options.find((o) => o.group === t.group && o.recommended) ?? info.options.find((o) => o.group === t.group);
                  setSelected(pick?.id);
                }}
                className={`relative flex h-9 items-center gap-2 ${on ? "text-fg-1" : "text-fg-3 hover:text-fg-1"}`}
              >
                <Icon name={t.icon} />
                {t.label} <span className="mono text-[11px] text-fg-4">{n}</span>
                {on && <span className="absolute right-0 bottom-[-1px] left-0 h-0.5 bg-amber" />}
              </button>
            );
          })}
        </div>

        <table className="w-full table-fixed border-collapse" role="radiogroup" aria-label="Format">
          <colgroup>
            <col className="w-[112px]" /><col className="w-[84px]" /><col className="w-[150px]" />
            {showFps && <col className="w-[48px]" />}<col className="w-[104px]" /><col />
          </colgroup>
          <thead>
            <tr className="t-micro text-left">
              <th className="py-2.5 pr-3 pl-5 font-medium">{tab === "subtitle" ? "Language" : "Quality"}</th>
              <th className="px-3 font-medium">Container</th>
              <th className="px-3 font-medium">{tab === "subtitle" ? "Code" : "Codec"}</th>
              {showFps && <th className="px-3 font-medium">FPS</th>}
              <th className="px-3 font-medium">Size</th>
              <th className="px-3 font-medium">Notes</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((o) => {
              const on = o.id === selected;
              return (
                <tr
                  key={o.id}
                  role="radio"
                  aria-checked={on}
                  tabIndex={on ? 0 : -1}
                  onClick={() => setSelected(o.id)}
                  onDoubleClick={start}
                  onKeyDown={(e) => {
                    const i = rows.indexOf(o);
                    if (e.key === "ArrowDown" && rows[i + 1]) setSelected(rows[i + 1].id);
                    if (e.key === "ArrowUp" && rows[i - 1]) setSelected(rows[i - 1].id);
                    if (e.key === "Enter") start();
                  }}
                  className={`t-data h-8 border-t border-line-1 ${on ? "bg-[var(--amber-tint)] text-fg-1" : "text-fg-2 hover:bg-bg-3"}`}
                >
                  <td className={`pr-3 pl-5 font-sans text-[13px] font-medium text-fg-1 ${on ? "shadow-[inset_2px_0_0_var(--amber)]" : ""}`}>
                    <span className={`mr-2.5 inline-block h-3 w-3 rounded-full border-[1.5px] align-[-2px] ${on ? "border-amber bg-[radial-gradient(circle,var(--amber)_0_3px,transparent_3.5px)]" : "border-line-3"}`} />
                    {o.label}
                  </td>
                  <td className="truncate px-3">{o.container}</td>
                  <td className="truncate px-3" title={o.codec}>{o.codec || "—"}</td>
                  {showFps && <td className="px-3">{o.fps ? Math.round(o.fps) : "—"}</td>}
                  <td className="px-3 whitespace-nowrap">{sizeText(o)}</td>
                  <td className={`truncate px-3 font-sans text-xs ${on ? "text-fg-2" : "text-fg-3"}`} title={o.note}>
                    {o.recommended && <span className="mr-1.5 text-amber">recommended ·</span>}
                    {o.note}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>

        <div className="mt-auto flex items-center gap-3 border-t border-line-1 bg-bg-1 px-5 py-3">
          <div className="flex min-w-0 flex-1 flex-col">
            <span className="t-micro">Save to</span>
            <span className="t-data truncate text-fg-2" title={settings?.downloadDir}>{settings?.downloadDir ?? "—"}</span>
          </div>
          <button className="btn btn-secondary" onClick={changeDir}><Icon name="folder" />Change</button>
          <button className="btn btn-primary btn-lg" onClick={start} disabled={!chosen || busy}>
            <Icon name="download" />
            {chosen ? `Download ${chosen.label}` : "Choose a format"}
          </button>
        </div>
      </div>
    </section>
  );
}

import { useEffect, useState } from "react";
import { Icon } from "../../icons/Icon";
import { api, toError } from "../../ipc/api";
import type { FirefoxProfile, SessionSite, SessionStatus, Sessions, Settings } from "../../ipc/types";
import { Row, Switch } from "./controls";

const SITES: { id: SessionSite; name: string; gain: string }[] = [
  { id: "bilibili", name: "Bilibili", gain: "720p/1080p (1080P+ and 4K with 大会员) instead of the guest 480p" },
  { id: "douyin", name: "Douyin", gain: "Douyin videos, which aren't shown to signed-out visitors" },
];

type Check = { state: "idle" } | { state: "checking" } | { state: "done"; status: SessionStatus } | { state: "error"; message: string };

/**
 * Opt-in signed-in sessions (docs/DESIGN_SIGNED_IN_SESSIONS.md). Off by default; collapsed while off.
 * Rabbit stores only which source and which sites — cookies are read fresh and never saved.
 */
export function SignedInSites({ settings, save }: { settings: Settings; save: (patch: Partial<Settings>) => void }) {
  const s = settings.sessions;
  const [profiles, setProfiles] = useState<FirefoxProfile[] | null>(null);
  const [checks, setChecks] = useState<Record<SessionSite, Check>>({ bilibili: { state: "idle" }, douyin: { state: "idle" } });

  useEffect(() => {
    if (s.enabled && profiles === null) api.listFirefoxProfiles().then(setProfiles, () => setProfiles([]));
  }, [s.enabled, profiles]);

  const set = (patch: Partial<Sessions>) => {
    save({ sessions: { ...s, ...patch } });
    setChecks({ bilibili: { state: "idle" }, douyin: { state: "idle" } });
  };

  const toggleAll = (on: boolean) => {
    if (!on) {
      // Off forgets the source and the site switches.
      set({ enabled: false, source: null, bilibili: false, douyin: false });
      return;
    }
    set({ enabled: true });
  };

  const pickFirefox = (list: FirefoxProfile[]) => {
    const pick = list.find((p) => p.isDefault && p.hasCookies) ?? list.find((p) => p.hasCookies) ?? list[0];
    if (pick) set({ source: { kind: "firefoxProfile", path: pick.path } });
  };

  const chooseFile = () =>
    api.chooseCookieFile().then((path) => path && set({ source: { kind: "netscapeFile", path } }), () => {});

  const check = async (site: SessionSite) => {
    setChecks((c) => ({ ...c, [site]: { state: "checking" } }));
    try {
      const status = await api.sessionCheck(site);
      setChecks((c) => ({ ...c, [site]: { state: "done", status } }));
    } catch (e) {
      setChecks((c) => ({ ...c, [site]: { state: "error", message: toError(e).message } }));
    }
  };

  const kind = s.source?.kind ?? null;

  return (
    <>
      <Row label="Signed-in sites" hint="Optional. Use your own browser session for sites that hide videos or HD from signed-out visitors.">
        <Switch label="Signed-in sites" on={s.enabled} onChange={toggleAll} />
        {!s.enabled && <span className="t-meta">Off — Rabbit downloads as a signed-out visitor.</span>}
      </Row>

      {s.enabled && (
        <div className="grid gap-3 border-t border-line-1 py-3.5 pl-[264px]">
          <p className="t-meta max-w-2xl">
            Rabbit never sees or stores your password. It reads your cookies for the sites switched on below — only those
            sites' cookies — each time you analyze a link, and sends them only to that site. Nothing is saved except the
            source you choose here.
          </p>

          <div className="grid gap-2" role="radiogroup" aria-label="Session source">
            <label className="flex items-center gap-2">
              <input type="radio" name="session-source" checked={kind === "firefoxProfile"} onChange={() => profiles && pickFirefox(profiles)} disabled={!profiles?.length} />
              <span className="w-36">Firefox profile</span>
              {profiles && profiles.length > 0 ? (
                <select
                  className="field h-7 max-w-sm px-2"
                  aria-label="Firefox profile"
                  disabled={kind !== "firefoxProfile"}
                  value={kind === "firefoxProfile" ? s.source!.path : ""}
                  onChange={(e) => set({ source: { kind: "firefoxProfile", path: e.target.value } })}
                >
                  {kind !== "firefoxProfile" && <option value="">—</option>}
                  {profiles.map((p) => (
                    <option key={p.path} value={p.path}>
                      {p.name}{p.isDefault ? " (default)" : ""}{p.hasCookies ? "" : " — no cookies yet"}
                    </option>
                  ))}
                </select>
              ) : (
                <span className="t-meta">{profiles === null ? "Looking for Firefox…" : "Firefox isn't installed (or has no profile)."}</span>
              )}
            </label>
            <label className="flex items-center gap-2">
              <input type="radio" name="session-source" checked={kind === "netscapeFile"} onChange={chooseFile} />
              <span className="w-36">cookies.txt file</span>
              {kind === "netscapeFile" && <span className="t-data min-w-0 truncate text-fg-2" title={s.source!.path}>{s.source!.path}</span>}
              <button className="btn btn-secondary" onClick={chooseFile}><Icon name="folder" />Choose…</button>
            </label>
            <p className="t-meta">Chrome and Edge encrypt their cookies so other programs can't read them; use Firefox, or export a cookies.txt.</p>
          </div>

          <div className="grid gap-1">
            {SITES.map((site) => {
              const c = checks[site.id];
              const on = s[site.id];
              return (
                <div key={site.id} className="flex min-w-0 items-center gap-3 py-1">
                  <Switch label={`Use my ${site.name} session`} on={on} disabled={!s.source} onChange={(v) => set({ [site.id]: v } as Partial<Sessions>)} />
                  <span className="w-20 font-medium">{site.name}</span>
                  <span className="t-meta min-w-0 flex-1 truncate">
                    {c.state === "checking" && "Checking…"}
                    {c.state === "error" && <span className="text-err">{c.message}</span>}
                    {c.state === "done" && (
                      <span className={c.status.state === "signedIn" ? "text-ok" : "text-warn"}>
                        {c.status.state === "signedIn" ? `Signed in${c.status.account ? ` as ${c.status.account}` : ""} · ` : ""}
                        {c.status.detail}
                      </span>
                    )}
                    {c.state === "idle" && (on ? site.gain : "Off")}
                  </span>
                  <button className="btn btn-ghost" disabled={!s.source || c.state === "checking"} onClick={() => check(site.id)}>Check</button>
                </div>
              );
            })}
          </div>

          <p className="t-meta flex items-center gap-1.5 text-warn"><Icon name="alert" />Platforms may limit accounts used for heavy automated downloading.</p>
        </div>
      )}
    </>
  );
}

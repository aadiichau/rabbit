import { Icon, type IconName } from "../icons/Icon";
import { speed } from "../lib/format";
import { isActive, useApp, type View } from "../state/store";

function NavItem({ view, icon, label, hint, count }: { view: View; icon: IconName; label: string; hint: string; count?: number }) {
  const active = useApp((s) => s.view === view);
  const setView = useApp((s) => s.setView);
  return (
    <button
      onClick={() => setView(view)}
      aria-current={active ? "page" : undefined}
      className={`relative flex h-8 w-full items-center gap-2.5 rounded px-2.5 text-left transition-colors duration-[90ms] ${active ? "bg-bg-2 text-fg-1" : "text-fg-2 hover:bg-bg-2 hover:text-fg-1"}`}
    >
      {active && <span className="absolute -left-2 top-2 bottom-2 w-0.5 bg-amber" />}
      <Icon name={icon} />
      {label}
      {count ? (
        <span className="mono ml-auto rounded-[2px] bg-amber px-[5px] text-[11px] font-medium leading-4 text-[var(--amber-ink)]">{count}</span>
      ) : (
        <kbd className="ml-auto bg-transparent text-fg-4">{hint}</kbd>
      )}
    </button>
  );
}

function Heartbeat() {
  const stats = useApp((s) => {
    let bps = 0;
    let active = 0;
    let queued = 0;
    for (const id of s.order) {
      const t = s.tasks[id];
      if (!t) continue;
      if (t.state === "queued") queued++;
      else if (isActive(t)) {
        active++;
        bps += s.progress[id]?.speed ?? 0;
      }
    }
    return `${bps}|${active}|${queued}`;
  });
  const [bps, active, queued] = stats.split("|").map(Number);
  return (
    <div className="mx-[-8px] mb-1.5 mt-2 border-t border-line-1 px-[18px] pt-3.5 pb-2">
      <div className="t-micro">Throughput</div>
      <div className="mono mt-1.5 flex items-center gap-2 text-[15px] font-medium leading-5">
        {active > 0 && <span className="dot" />}
        {active > 0 ? speed(bps) : "Idle"}
      </div>
      <div className="t-data mt-0.5 text-fg-3">
        {active} active · {queued} queued
      </div>
    </div>
  );
}

export function Sidebar() {
  const running = useApp((s) => s.order.reduce((n, id) => n + (s.tasks[id] && isActive(s.tasks[id]) ? 1 : 0), 0));
  return (
    <nav className="flex flex-col gap-0.5 border-r border-line-1 bg-bg-0 px-2 py-3" aria-label="Main">
      <NavItem view="home" icon="home" label="Home" hint="^1" />
      <NavItem view="queue" icon="queue" label="Queue" hint="^2" count={running} />
      <NavItem view="library" icon="library" label="Library" hint="^3" />
      <div className="flex-1" />
      <Heartbeat />
      <NavItem view="settings" icon="settings" label="Settings" hint="^," />
    </nav>
  );
}

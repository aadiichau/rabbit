export function Row({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[240px_minmax(0,1fr)] items-center gap-6 border-t border-line-1 py-3.5">
      <div>
        <div className="font-medium">{label}</div>
        {hint && <div className="t-meta mt-0.5">{hint}</div>}
      </div>
      <div className="flex min-w-0 items-center gap-2">{children}</div>
    </div>
  );
}

export function Stepper({ value, min, max, onChange, label }: { value: number; min: number; max: number; onChange: (n: number) => void; label: string }) {
  return (
    <div className="field flex w-28 items-center justify-between px-1" role="group" aria-label={label}>
      <button className="btn btn-icon h-6 w-6" aria-label={`Decrease ${label}`} disabled={value <= min} onClick={() => onChange(value - 1)}>−</button>
      <span className="mono">{value}</span>
      <button className="btn btn-icon h-6 w-6" aria-label={`Increase ${label}`} disabled={value >= max} onClick={() => onChange(value + 1)}>+</button>
    </div>
  );
}

export function Switch({ on, onChange, label, disabled }: { on: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!on)}
      className={`relative h-5 w-9 shrink-0 rounded-full border transition-colors duration-[160ms] disabled:opacity-40 ${on ? "border-amber bg-amber" : "border-line-3 bg-bg-3"}`}
    >
      <span
        className={`absolute top-[2px] h-3.5 w-3.5 rounded-full transition-[left,background] duration-[160ms] ${on ? "left-[18px] bg-[var(--amber-ink)]" : "left-[2px] bg-fg-3"}`}
      />
    </button>
  );
}

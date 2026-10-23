const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** Bytes in Explorer-style binary units with one decimal (KB and above). */
export function bytes(n: number | null | undefined): string {
  if (n == null || !Number.isFinite(n) || n < 0) return "—";
  let i = 0;
  let v = n;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  return i === 0 ? `${v} B` : `${v.toFixed(1)} ${UNITS[i]}`;
}

/** Same unit for both numbers so "41.2 / 128.0 MB" reads cleanly. */
export function progressPair(done: number, total: number | null): string {
  if (total == null || total <= 0) return bytes(done);
  let i = 0;
  let t = total;
  while (t >= 1024 && i < UNITS.length - 1) {
    t /= 1024;
    i++;
  }
  const d = done / 1024 ** i;
  return i === 0 ? `${done} / ${total} B` : `${d.toFixed(1)} / ${t.toFixed(1)} ${UNITS[i]}`;
}

export function speed(bps: number): string {
  if (!Number.isFinite(bps) || bps <= 0) return "—";
  return `${bytes(bps)}/s`;
}

/** 7 → "0:07", 3725 → "1:02:05". */
export function clock(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return "—";
  const s = Math.round(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export function compactCount(n: number | null | undefined): string {
  if (n == null) return "";
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return String(n);
}

export function relativeDate(ms: number, now = Date.now()): string {
  const d = Math.max(0, now - ms) / 1000;
  if (d < 60) return "just now";
  if (d < 3600) return `${Math.floor(d / 60)} min ago`;
  if (d < 86400) return `${Math.floor(d / 3600)} h ago`;
  return new Date(ms).toISOString().slice(0, 10);
}

export const MONOGRAM: Record<string, string> = {
  youtube: "YT", tiktok: "TT", douyin: "DY", bilibili: "BI", instagram: "IG", facebook: "FB",
  twitter: "X", pinterest: "PI", kuaishou: "KS", rednote: "RN", suno: "SU", other: "··",
};

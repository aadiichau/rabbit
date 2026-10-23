// Renders research/platform-report.json (written by tests/platform_matrix.rs) as a Markdown table for
// docs/TEST_RESULTS.md. Usage: node results-table.mjs > table.md
import { readFileSync } from "node:fs";
const rows = JSON.parse(readFileSync(new URL("../platform-report.json", import.meta.url), "utf8"));
const order = ["youtube", "bilibili", "kuaishou", "rednote", "instagram", "facebook", "twitter", "pinterest", "suno", "tiktok", "douyin"];
rows.sort((a, b) => order.indexOf(a.platform) - order.indexOf(b.platform));
const mb = (b) => (b ? `${(b / 1e6).toFixed(1)} MB` : "—");
const cell = (s) => String(s ?? "—").replace(/\|/g, "\\|").replace(/\n/g, " ");
console.log("| Platform | URL | Type | Highest offered | Chosen | Final (FFprobe) | Size | Audio | Outcome | Extractor / error |");
console.log("|---|---|---|---|---|---|---|---|---|---|");
for (const r of rows) {
  const p = r.probe ?? {};
  let final = p.width ? `${p.width}×${p.height} ${p.vcodec ?? ""}` : "—";
  let audio = p.acodec ?? (r.probe ? "none" : "—");
  if (Array.isArray(r.files)) {
    const imgs = r.files.filter((f) => f.vcodec === "mjpeg" || /\.(jpg|png|webp)$/.test(f.name));
    const vids = r.files.filter((f) => !imgs.includes(f));
    final = `${r.files.length} files: ${[...imgs, ...vids].map((f) => `${f.width}×${f.height}`).join(", ")}`;
    audio = vids.length ? vids.map((f) => f.acodec ?? "none").join(", ") : "—";
  }
  const offered = r.bestOffered ? `${r.bestOffered}p` : "—";
  const outcome = r.ok ? "PASS" : `FAIL (${r.errorKind ?? r.stage})`;
  const note = r.ok ? r.extractor : r.error;
  console.log(`| ${r.platform} | ${cell(r.url.replace(/^https?:\/\//, "").slice(0, 70))} | ${cell(r.kind ?? r.expect)} | ${offered} | ${cell(r.chosen)} | ${cell(final)} | ${mb(r.bytes)} | ${cell(audio)} | ${outcome} | ${cell(note?.slice(0, 110))} |`);
}

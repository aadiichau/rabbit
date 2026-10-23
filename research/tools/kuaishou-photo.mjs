// Finds the photo object inside the mobile share page's INIT_STATE (keys are obfuscated, so search by fields)
// and checks the media CDN. Usage: node kuaishou-photo.mjs <v.kuaishou.com short link>
const MOBILE = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
const res = await fetch(process.argv[2], { headers: { "user-agent": MOBILE }, redirect: "follow", signal: AbortSignal.timeout(25000) });
const html = await res.text();
const at = html.indexOf("INIT_STATE");
const start = html.indexOf("{", at);
let depth = 0, end = 0;
for (let i = start, inStr = false, esc = false; i < html.length; i++) {
  const ch = html[i];
  if (inStr) { if (esc) esc = false; else if (ch === "\\") esc = true; else if (ch === '"') inStr = false; continue; }
  if (ch === '"') inStr = true; else if (ch === "{") depth++; else if (ch === "}" && --depth === 0) { end = i + 1; break; }
}
const state = JSON.parse(html.slice(start, end));
let photo = null;
const walk = (o) => {
  if (!o || typeof o !== "object" || photo) return;
  if (Array.isArray(o.mainMvUrls) && (o.photoId || o.caption !== undefined)) { photo = o; return; }
  for (const v of Object.values(o)) walk(v);
};
walk(state);
if (!photo) { console.log("no photo object found"); process.exit(0); }
console.log("photo keys:", Object.keys(photo).join(","));
console.log({ photoId: photo.photoId, caption: photo.caption, userName: photo.userName, duration: photo.duration, type: photo.photoType ?? photo.type, width: photo.width, height: photo.height, atlas: !!photo.ext_params?.atlas || !!photo.atlas });
console.log("mainMvUrls:", photo.mainMvUrls.map((m) => m.cdn).join(", "));
const reps = photo.manifest?.adaptationSet?.flatMap((s) => s.representation ?? []) ?? [];
for (const r of reps) console.log(`  rep ${r.width}x${r.height} ${r.avgBitrate ?? r.maxBitrate}kbps ${r.qualityType ?? ""} url=${!!r.url} backup=${(r.backupUrl ?? []).length}`);
if (process.argv[3]) (await import("node:fs")).writeFileSync(process.argv[3], JSON.stringify(photo, null, 1));
if (reps[0]) console.log("rep keys:", Object.keys(reps[0]).join(","), "| set keys:", Object.keys(photo.manifest.adaptationSet[0]).join(","));
const url = photo.mainMvUrls[0].url;
const probe = await fetch(url, { headers: { "user-agent": MOBILE, range: "bytes=0-0" }, signal: AbortSignal.timeout(20000) });
console.log("CDN range probe:", probe.status, probe.headers.get("content-range"), probe.headers.get("content-type"));

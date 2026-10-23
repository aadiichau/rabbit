// Dumps the shape of the Kuaishou share page's __APOLLO_STATE__ photo entry.
const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const res = await fetch(process.argv[2], { headers: { "user-agent": UA }, redirect: "follow" });
const html = await res.text();
const m = html.match(/window\.__APOLLO_STATE__\s*=\s*(\{[\s\S]*?\});?\s*\(function|window\.__APOLLO_STATE__\s*=\s*(\{[\s\S]*?\})\s*<\/script>/);
const raw = (m?.[1] ?? m?.[2] ?? "").replace(/:\s*undefined/g, ":null");
const state = JSON.parse(raw);
const client = state.defaultClient ?? state;
const keys = Object.keys(client);
console.log("top keys:", keys.filter((k) => /Photo|Author|Video/.test(k)).slice(0, 10));
const photoKey = keys.find((k) => k.startsWith("VisionVideoDetailPhoto:"));
const p = client[photoKey];
const shape = (o, depth = 0) =>
  Object.fromEntries(Object.entries(o).map(([k, v]) => [k, v && typeof v === "object" && depth < 2 ? (Array.isArray(v) ? `array[${v.length}]` : shape(v, depth + 1)) : typeof v === "string" ? `str(${v.length})` : v]));
console.log(photoKey, JSON.stringify(shape(p), null, 1).slice(0, 2500));
if (p.videoResource) {
  const vr = typeof p.videoResource === "string" ? JSON.parse(p.videoResource) : p.videoResource.json ?? p.videoResource;
  console.log("videoResource type:", typeof p.videoResource, Object.keys(vr ?? {}));
  for (const codec of Object.keys(vr ?? {})) {
    const sets = vr[codec]?.adaptationSet ?? [];
    for (const s of sets) for (const r of s.representation ?? []) console.log(`  ${codec} ${r.width}x${r.height} ${r.avgBitrate}kbps ${r.qualityType ?? r.qualityLabel ?? ""} url=${!!r.url} backup=${(r.backupUrl ?? []).length} size=${r.fileSize ?? "?"}`);
  }
}
const authorKey = keys.find((k) => k.startsWith("VisionVideoDetailAuthor:"));
console.log(authorKey, client[authorKey]?.name);

// Fetches a public RedNote note page and summarises window.__INITIAL_STATE__ (what yt-dlp parses).
// Usage: node rednote-probe.mjs <url>
const url = process.argv[2];
const res = await fetch(url, {
  headers: { "user-agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36", "accept-language": "en-US,en;q=0.9" },
  redirect: "follow",
});
const html = await res.text();
console.log(`status ${res.status} final ${res.url.split("?")[0]} bytes ${html.length}`);
const m = html.match(/window\.__INITIAL_STATE__\s*=\s*(\{[\s\S]*?\})\s*<\/script>/);
if (!m) {
  console.log("no __INITIAL_STATE__; title:", html.match(/<title>([^<]*)/)?.[1]);
  process.exit(0);
}
// The state is JS, not JSON: `undefined` appears as a bare value.
const state = JSON.parse(m[1].replace(/:\s*undefined/g, ":null").replace(/new Set\((\[[^\]]*\])\)/g, "$1"));
const map = state?.note?.noteDetailMap ?? {};
const ids = Object.keys(map);
console.log("noteDetailMap keys:", ids);
for (const id of ids) {
  const n = map[id]?.note;
  if (!n || !Object.keys(n).length) {
    console.log(id, "→ empty note object");
    continue;
  }
  const streams = Object.values(n.video?.media?.stream ?? {}).flat();
  console.log({
    id,
    type: n.type,
    title: n.title,
    user: n.user?.nickname,
    images: (n.imageList ?? []).length,
    imageSample: n.imageList?.[0]?.urlDefault?.slice(0, 60),
    livePhotos: (n.imageList ?? []).filter((i) => i.livePhoto).length,
    streams: streams.map((s) => `${s.videoCodec}/${s.audioCodec} ${s.width}x${s.height} ${s.size ?? "?"}B url=${!!s.masterUrl}`),
    duration: n.video?.capa?.duration,
  });
}

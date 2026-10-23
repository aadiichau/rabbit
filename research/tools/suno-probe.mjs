// What a public Suno song page gives a signed-out visitor: resolved song id, og/meta tags,
// audio/video/image URLs referenced by the page, and whether the CDN serves them.
// Usage: node suno-probe.mjs <suno.com/song/<uuid> or suno.com/s/<code>>
const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
let url = process.argv[2];
for (let i = 0; i < 5; i++) {
  const r = await fetch(url, { headers: { "user-agent": UA }, redirect: "manual", signal: AbortSignal.timeout(20000) });
  const loc = r.headers.get("location");
  console.log("hop", r.status, new URL(url).host + new URL(url).pathname);
  if (!loc) break;
  url = new URL(loc, url).href;
}
const page = await fetch(url, { headers: { "user-agent": UA }, signal: AbortSignal.timeout(20000) });
const html = await page.text();
console.log("page", page.status, html.length, "bytes", new URL(page.url).pathname);
for (const m of html.matchAll(/<meta[^>]+(?:property|name)="((?:og|twitter):[^"]+)"[^>]+content="([^"]*)"/g)) console.log("  meta", m[1], m[2].slice(0, 110));
const cdn = [...new Set(html.match(/https:\/\/cdn\d?\.suno\.ai\/[A-Za-z0-9_.\-\/]+/g) ?? [])];
console.log("cdn urls:", cdn.length);
for (const u of cdn.slice(0, 12)) console.log("  ", u);
for (const key of ["audio_url", "video_url", "image_large_url", "display_name", "\\\"title\\\"", "duration", "is_public", "major_model_version"]) {
  const i = html.indexOf(key);
  console.log(key.padEnd(20), i < 0 ? "-" : html.slice(i, i + 140).replace(/\s+/g, " "));
}
const id = new URL(page.url).pathname.split("/").pop();
for (const u of [`https://cdn1.suno.ai/${id}.mp3`, `https://cdn1.suno.ai/${id}.mp4`, `https://cdn2.suno.ai/image_large_${id}.jpeg`]) {
  const r = await fetch(u, { headers: { "user-agent": UA, range: "bytes=0-15" }, signal: AbortSignal.timeout(20000) });
  console.log("cdn", r.status, r.headers.get("content-type"), r.headers.get("content-range"), u);
}

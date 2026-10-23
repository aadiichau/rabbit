// Prints the schema.org JSON-LD on a public kwai.com video page (phone UA) and probes the video URL.
// Usage: node kwai-ldjson.mjs <kwai.com video url>
const UA = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
const html = await (await fetch(process.argv[2], { headers: { "user-agent": UA }, signal: AbortSignal.timeout(20000) })).text();
const blocks = [...html.matchAll(/<script type="application\/ld\+json"[^>]*>([\s\S]*?)<\/script>/g)].map((m) => JSON.parse(m[1]));
const short = (k, v) => (typeof v === "string" && v.length > 90 ? v.slice(0, 90) + "…" : v);
for (const b of blocks) console.log(JSON.stringify(b, short, 1).slice(0, 3000));
const find = (o) => (o && typeof o === "object" ? (o["@type"] === "VideoObject" ? o : Object.values(o).map(find).find(Boolean)) : null);
const video = blocks.map(find).find(Boolean);
if (video?.contentUrl) {
  const r = await fetch(video.contentUrl, { headers: { "user-agent": UA, range: "bytes=0-0" } });
  console.log("contentUrl", r.status, r.headers.get("content-type"), r.headers.get("content-range"));
}

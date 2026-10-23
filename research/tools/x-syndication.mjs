// What X's public embed data (cdn.syndication.twimg.com/tweet-result, used by publish.twitter.com
// embeds and Vercel's react-tweet) returns for a post: photos, videos and their variants.
// Usage: node x-syndication.mjs <status id>
const id = process.argv[2];
// Token as computed by the embed widget (react-tweet `getToken`).
const token = ((Number(id) / 1e15) * Math.PI).toString(36).replace(/(0+|\.)/g, "");
const r = await fetch(`https://cdn.syndication.twimg.com/tweet-result?id=${id}&lang=en&token=${token}`, {
  headers: { "user-agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36" },
  signal: AbortSignal.timeout(20000),
});
const text = await r.text();
console.log("status", r.status, text.length, "bytes");
let t;
try { t = JSON.parse(text); } catch { console.log(text.slice(0, 200)); process.exit(0); }
console.log({ type: t.__typename, user: t.user?.screen_name, text: t.text?.slice(0, 60), created: t.created_at, tombstone: !!t.tombstone });
for (const m of t.mediaDetails ?? []) {
  console.log(" media", m.type, m.original_info?.width + "x" + m.original_info?.height, m.media_url_https);
  for (const v of m.video_info?.variants ?? []) console.log("    variant", v.content_type, v.bitrate ?? "", v.url.slice(0, 110));
}
for (const p of t.photos ?? []) console.log(" photo", p.width + "x" + p.height, p.url);
if (t.video) console.log(" video", JSON.stringify(t.video).slice(0, 200));

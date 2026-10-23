// What Instagram's public embed page (/p/<code>/embed/captioned/) gives a signed-out visitor:
// image URLs and sizes, carousel children, video URLs. Usage: node instagram-embed.mjs <shortcode>
const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const code = process.argv[2];
const r = await fetch(`https://www.instagram.com/p/${code}/embed/captioned/`, { headers: { "user-agent": UA }, signal: AbortSignal.timeout(20000) });
const html = await r.text();
console.log("embed", r.status, html.length, "bytes | login wall:", /loginForm|Log in to see/i.test(html));
const decoded = html.replace(/\\\//g, "/").replace(/\\u0026/g, "&");
const imgs = [...new Set([...decoded.matchAll(/https:\/\/[^"'\s]+?\.(?:jpg|webp|heic)[^"'\s\\]*/g)].map((m) => m[0]))];
console.log("image-like URLs:", imgs.length);
for (const u of imgs.slice(0, 12)) {
  const size = u.match(/(?:s|p)(\d{3,4})x(\d{3,4})/)?.slice(1).join("x") ?? "?";
  console.log("  ", size, u.slice(0, 120));
}
for (const key of ["display_resources", "edge_sidecar_to_children", "video_url", "is_video", "shortcode_media", "gql_data", "contextJSON", "EmbeddedMediaImage", "Carousel"]) {
  const i = decoded.indexOf(key);
  console.log(key.padEnd(24), i < 0 ? "-" : `@${i} ${decoded.slice(i, i + 100).replace(/\s+/g, " ")}`);
}

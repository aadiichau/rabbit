// Checks what Douyin's mobile share pages give a signed-out phone browser: m.douyin.com and
// iesdouyin.com /share/video/<id>, looking for a video URL, video_id or embedded post data.
// Usage: node douyin-mshare.mjs <aweme id>
const UA = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
const id = process.argv[2];
for (const url of [`https://m.douyin.com/share/video/${id}`, `https://www.iesdouyin.com/share/video/${id}/`, `https://www.douyin.com/video/${id}`]) {
  try {
    const r = await fetch(url, { headers: { "user-agent": UA }, redirect: "follow", signal: AbortSignal.timeout(20000) });
    const html = await r.text();
    const hits = ["playwm", "play_addr", "video_id=", "<video", "aweme_detail", "item_list", "RENDER_DATA", "_ROUTER_DATA", "isSpider\":true", "验证", "captcha"].map((n) => `${n}:${html.includes(n) ? "y" : "-"}`).join(" ");
    console.log(r.status, new URL(r.url).host + new URL(r.url).pathname, html.length, "|", hits);
    const m = html.match(/video_id=([A-Za-z0-9]+)/);
    if (m) console.log("   video_id", m[1]);
  } catch (e) {
    console.log("ERR", url, e.cause?.code ?? e.message);
  }
}

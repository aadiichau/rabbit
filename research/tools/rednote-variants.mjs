// Tries how a public RedNote share link opens for a guest, per user agent and URL shape.
// Usage: node rednote-variants.mjs <resolved xiaohongshu.com note url>
const src = new URL(process.argv[2]);
const id = src.pathname.split("/").pop();
const token = src.searchParams.get("xsec_token");
const UAS = {
  desktop: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36",
  iphone: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1",
  android: "Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Mobile Safari/537.36",
};
const shapes = {
  "discovery+all": `/discovery/item/${id}${src.search}`,
  "discovery+token": `/discovery/item/${id}?xsec_token=${encodeURIComponent(token)}&xsec_source=app_share`,
  "explore+token": `/explore/${id}?xsec_token=${encodeURIComponent(token)}&xsec_source=app_share`,
  "explore+pc_share": `/explore/${id}?xsec_token=${encodeURIComponent(token)}&xsec_source=pc_share`,
};
for (const [uaName, ua] of Object.entries(UAS)) {
  for (const [shapeName, path] of Object.entries(shapes)) {
    try {
      const res = await fetch(`https://www.rednote.com${path}`, { headers: { "user-agent": ua, "accept-language": "en-US,en;q=0.9" }, redirect: "follow", signal: AbortSignal.timeout(20000) });
      const html = await res.text();
      const m = html.match(/window\.__INITIAL_STATE__\s*=\s*(\{[\s\S]*?\})\s*<\/script>/);
      let note = null;
      if (m) {
        const state = JSON.parse(m[1].replace(/:\s*undefined/g, ":null").replace(/new Set\((\[[^\]]*\])\)/g, "$1"));
        note = state?.note?.noteDetailMap?.[id]?.note ?? null;
      }
      const verify = /captcha|verification|website-login|\/login/i.test(res.url);
      const streams = note ? Object.values(note.video?.media?.stream ?? {}).flat().length : 0;
      console.log(`${uaName.padEnd(8)} ${shapeName.padEnd(17)} → ${res.status} ${new URL(res.url).pathname.slice(0, 40).padEnd(40)} note=${note && Object.keys(note).length ? `${note.type} "${(note.title ?? "").slice(0, 20)}" streams=${streams} images=${note.imageList?.length ?? 0}` : "none"}${verify ? " [login/verify]" : ""}`);
    } catch (e) {
      console.log(`${uaName.padEnd(8)} ${shapeName.padEnd(17)} → ERROR ${e.cause?.code ?? e.message}`);
    }
  }
}

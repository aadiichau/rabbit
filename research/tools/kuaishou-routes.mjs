// Which Kuaishou URL forms lead to a mobile share page with public data, for a known photo id.
// Usage: node kuaishou-routes.mjs <photo id, e.g. 3xjegz725scedme>
const MOBILE = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
const id = process.argv[2];
const routes = [
  `https://v.m.chenzhongtech.com/fw/photo/${id}`,
  `https://www.kuaishou.com/short-video/${id}`,
  `https://m.gifshow.com/fw/photo/${id}`,
  `https://v.kuaishou.com/fw/photo/${id}`,
];
for (const start of routes) {
  let url = start;
  const hops = [];
  try {
    for (let i = 0; i < 6; i++) {
      const r = await fetch(url, { headers: { "user-agent": MOBILE }, redirect: "manual", signal: AbortSignal.timeout(15000) });
      const loc = r.headers.get("location");
      if (r.status >= 300 && r.status < 400 && loc) {
        url = new URL(loc, url).href;
        hops.push(`${r.status}→${new URL(url).host}${new URL(url).pathname}`);
        continue;
      }
      const html = await r.text();
      console.log(start, "|", hops.join(" "), "| final", r.status, html.length, "INIT_STATE:", html.includes("INIT_STATE"), "mainMvUrls:", html.includes("mainMvUrls"), "captcha:", /captcha/i.test(html));
      break;
    }
  } catch (e) {
    console.log(start, "|", hops.join(" "), "| ERR", e.cause?.code ?? e.message);
  }
}

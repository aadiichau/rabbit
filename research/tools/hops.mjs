// Prints each redirect hop (status, host+path, timing) for a URL with a desktop or mobile UA.
// Usage: node hops.mjs <url> [mobile|desktop]
const UA = process.argv[3] === "desktop"
  ? "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36"
  : "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
let url = process.argv[2];
for (let i = 0; i < 8; i++) {
  const t = Date.now();
  try {
    const r = await fetch(url, { headers: { "user-agent": UA }, redirect: "manual", signal: AbortSignal.timeout(20000) });
    const loc = r.headers.get("location");
    const body = loc ? "" : await r.text();
    const u = new URL(url);
    console.log(r.status, `${Date.now() - t}ms`, u.host + u.pathname, u.search.slice(0, 60), loc ? "" : `body ${body.length}B title=${body.match(/<title>([^<]*)/)?.[1] ?? ""}`);
    if (!loc) break;
    url = new URL(loc, url).href;
  } catch (e) {
    console.log("ERR", `${Date.now() - t}ms`, new URL(url).host + new URL(url).pathname, e.cause?.code ?? e.message);
    break;
  }
}

// Inspects Kuaishou's mobile share landing page (v.m.chenzhongtech.com/fw/photo/<id>) for public media data.
// Usage: node kuaishou-mobile.mjs <v.kuaishou.com short link>
const MOBILE = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
const hop = await fetch(process.argv[2], { headers: { "user-agent": MOBILE }, redirect: "manual", signal: AbortSignal.timeout(20000) });
const target = hop.headers.get("location");
console.log("short link →", hop.status, target ? new URL(target).host + new URL(target).pathname : "(none)");
if (!target) process.exit(0);
const res = await fetch(target, { headers: { "user-agent": MOBILE }, redirect: "follow", signal: AbortSignal.timeout(25000) });
const html = await res.text();
console.log("page", res.status, new URL(res.url).host + new URL(res.url).pathname, "bytes", html.length, "title:", html.match(/<title>([^<]*)/)?.[1]);
for (const m of html.matchAll(/window\.(__\w+__|INIT_STATE|__INIT_PROPS__)\s*=/g)) console.log("window var:", m[1]);
for (const needle of ["INIT_STATE", "photoUrl", "mainMvUrls", "manifest", "atlas", "coverUrls", ".mp4", "caption", "captcha", "verify"]) {
  const i = html.indexOf(needle);
  console.log(`${needle.padEnd(12)} ${i < 0 ? "-" : "@" + i + "  " + html.slice(i, i + 90).replace(/\s+/g, " ")}`);
}

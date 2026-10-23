// Inspects the mobile (H5) RedNote note page: where note data, video streams and images live.
// Usage: node rednote-mobile.mjs <resolved xiaohongshu.com note url>
const src = new URL(process.argv[2]);
const UA = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
const res = await fetch(`https://www.rednote.com${src.pathname}${src.search}`, { headers: { "user-agent": UA }, signal: AbortSignal.timeout(20000) });
const html = await res.text();
console.log("status", res.status, "bytes", html.length, "title:", html.match(/<title>([^<]*)/)?.[1]);
for (const m of html.matchAll(/window\.(__\w+__)\s*=/g)) console.log("window var:", m[1]);
for (const needle of ["masterUrl", "sns-video", ".mp4", "imageList", "urlDefault", "noteData", "noteDetailMap", "originVideoKey", "captcha"]) {
  const i = html.indexOf(needle);
  console.log(`${needle.padEnd(15)} ${i < 0 ? "-" : "@" + i}`);
}
const m = html.match(/window\.__INITIAL_STATE__\s*=\s*(\{[\s\S]*?\})\s*<\/script>/);
if (m) {
  const state = JSON.parse(m[1].replace(/:\s*undefined/g, ":null").replace(/new Set\((\[[^\]]*\])\)/g, "$1"));
  const walk = (o, path, depth) => {
    if (!o || typeof o !== "object" || depth > 5) return;
    for (const [k, v] of Object.entries(o)) {
      const p = `${path}.${k}`;
      if (/^(noteData|note|video|media|stream|imageList|h264|h265|title|user|nickname|type)$/.test(k)) {
        const desc = Array.isArray(v) ? `array[${v.length}]` : typeof v === "object" && v ? `{${Object.keys(v).slice(0, 8).join(",")}}` : JSON.stringify(v)?.slice(0, 50);
        console.log(p, "=", desc);
      }
      walk(v, p, depth + 1);
    }
  };
  walk(state, "state", 0);
}

// Looks for other notes (id + xsec token) referenced from a note's mobile page, e.g. related notes.
// Usage: node rednote-related.mjs <resolved rednote/xiaohongshu note url>
const UA = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
let url = new URL(process.argv[2]);
for (let hop = 0; hop < 5 && !/(^|\.)(xiaohongshu|rednote)\.com$/.test(url.host); hop++) {
  const r = await fetch(url, { headers: { "user-agent": UA }, redirect: "manual", signal: AbortSignal.timeout(20000) });
  const loc = r.headers.get("location");
  if (!loc) break;
  url = new URL(loc, url);
}
const html = await (await fetch(`https://www.rednote.com${url.pathname}${url.search}`, { headers: { "user-agent": UA } })).text();
const ids = new Set(html.match(/[0-9a-f]{24}/g));
console.log("24-hex ids:", ids.size);
for (const m of html.matchAll(/xsec_?[tT]oken["'=:\s]+([A-Za-z0-9_=+\/-]{20,})/g)) console.log("token at", m.index, html.slice(Math.max(0, m.index - 120), m.index).replace(/\s+/g, " ").slice(-120));

// Resolves an xhslink.com / xiaohongshu.com / rednote.com note link and writes the public note data
// from the mobile page to a JSON file, for building the parser. Usage: node rednote-dump.mjs <link> <out.json>
import { writeFileSync } from "node:fs";
const UA = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
let url = new URL(process.argv[2]);
for (let hop = 0; hop < 5 && !/(^|\.)(xiaohongshu|rednote)\.com$/.test(url.host); hop++) {
  const r = await fetch(url, { headers: { "user-agent": UA }, redirect: "manual", signal: AbortSignal.timeout(20000) });
  const loc = r.headers.get("location");
  if (!loc) break;
  url = new URL(loc, url);
}
console.log("resolved path:", url.pathname, "params:", [...url.searchParams.keys()].join(","));
const res = await fetch(`https://www.rednote.com${url.pathname}${url.search}`, { headers: { "user-agent": UA }, signal: AbortSignal.timeout(20000) });
const html = await res.text();
const m = html.match(/window\.__INITIAL_STATE__\s*=\s*(\{[\s\S]*?\})\s*<\/script>/);
const state = JSON.parse(m[1].replace(/:\s*undefined/g, ":null").replace(/new Set\((\[[^\]]*\])\)/g, "$1"));
const note = state?.noteData?.data?.noteData;
console.log("status", res.status, "type", note?.type, "images", note?.imageList?.length, "title", note?.title);
writeFileSync(process.argv[3], JSON.stringify({ state: { noteData: state.noteData } }, null, 1));

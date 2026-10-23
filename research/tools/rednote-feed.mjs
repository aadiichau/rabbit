// Lists notes from rednote.com's public guest homepage feed (id, type, xsecToken), for test links.
const res = await fetch("https://www.rednote.com/explore", {
  headers: { "user-agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36", "accept-language": "en-US,en;q=0.9" },
});
const html = await res.text();
const m = html.match(/window\.__INITIAL_STATE__\s*=\s*(\{[\s\S]*?\})\s*<\/script>/);
const state = JSON.parse(m[1].replace(/:\s*undefined/g, ":null").replace(/new Set\((\[[^\]]*\])\)/g, "$1"));
const feeds = state?.feed?.feeds ?? [];
console.log(`status ${res.status}; ${feeds.length} feed items`);
for (const f of feeds.slice(0, 12)) {
  const c = f.noteCard ?? {};
  console.log(`${f.id}\t${c.type}\t${(c.displayTitle ?? "").slice(0, 30)}\t${f.xsecToken ? "token" : "no-token"}\thttps://www.rednote.com/explore/${f.id}?xsec_token=${encodeURIComponent(f.xsecToken ?? "")}&xsec_source=pc_feed`);
}

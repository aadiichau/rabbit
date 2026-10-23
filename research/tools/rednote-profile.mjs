// Lists notes (id, type, xsec_token present) visible on a public RedNote profile's mobile page,
// to find image-note test cases. Usage: node rednote-profile.mjs <userId>
const UA = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
const res = await fetch(`https://www.rednote.com/user/profile/${process.argv[2]}`, { headers: { "user-agent": UA }, signal: AbortSignal.timeout(20000) });
const html = await res.text();
console.log("status", res.status, new URL(res.url).pathname, "bytes", html.length);
const m = html.match(/window\.__INITIAL_STATE__\s*=\s*(\{[\s\S]*?\})\s*<\/script>/);
if (!m) process.exit(0);
const state = JSON.parse(m[1].replace(/:\s*undefined/g, ":null").replace(/new Set\((\[[^\]]*\])\)/g, "$1"));
const seen = new Set();
const walk = (o) => {
  if (!o || typeof o !== "object") return;
  const id = o.noteId ?? o.id;
  if (typeof id === "string" && /^[0-9a-f]{24}$/.test(id) && (o.type || o.noteCard) && !seen.has(id)) {
    seen.add(id);
    const card = o.noteCard ?? o;
    console.log(id, card.type, "token:", !!(o.xsecToken ?? card.xsecToken), (card.displayTitle ?? card.title ?? "").slice(0, 30));
  }
  for (const v of Object.values(o)) walk(v);
};
walk(state);
console.log("top-level keys:", Object.keys(state).join(","));
const notes = state.profile?.noteData ?? [];
if (notes[0]) console.log("note keys:", Object.keys(notes[0]).join(","));
for (const n of notes) console.log(n.noteId ?? n.id, n.type, "token:", !!(n.xsecToken ?? n.xsec_token), (n.title ?? n.displayTitle ?? "").slice(0, 30));

// Prints the default photoUrl and the first H.265 representation URL from a Kuaishou share page.
const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const res = await fetch(process.argv[2], { headers: { "user-agent": UA }, redirect: "follow" });
const html = await res.text();
const start = html.indexOf("window.__APOLLO_STATE__");
const body = html.slice(html.indexOf("=", start) + 1).replace(/:\s*undefined/g, ":null");
let depth = 0, end = 0;
for (let i = 0, inStr = false, esc = false; i < body.length; i++) {
  const ch = body[i];
  if (inStr) { if (esc) esc = false; else if (ch === "\\") esc = true; else if (ch === '"') inStr = false; continue; }
  if (ch === '"') inStr = true;
  else if (ch === "{") depth++;
  else if (ch === "}" && --depth === 0) { end = i + 1; break; }
}
const state = JSON.parse(body.slice(0, end).trim());
const client = state.defaultClient ?? state;
const photo = client[Object.keys(client).find((k) => k.startsWith("VisionVideoDetailPhoto:"))];
console.log(photo.photoUrl);
console.log(photo.manifestH265?.json?.adaptationSet?.[0]?.representation?.[0]?.url ?? "");

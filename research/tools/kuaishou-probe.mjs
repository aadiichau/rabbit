// Summarises the public Kuaishou share page behind a short link: where the media data lives.
// Usage: node kuaishou-probe.mjs <url>
const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const res = await fetch(process.argv[2], { headers: { "user-agent": UA }, redirect: "follow" });
const html = await res.text();
console.log("final:", res.url.split("?")[0], "status", res.status, "bytes", html.length);

// Find JSON-ish script assignments and keys mentioning photo / mp4.
for (const m of html.matchAll(/window\.(\w+)\s*=\s*/g)) console.log("window var:", m[1]);
const i = html.indexOf("photoUrl");
console.log("context around photoUrl:\n", html.slice(Math.max(0, i - 600), i + 400).replace(/\s+/g, " "));
const mp4s = [...new Set([...html.matchAll(/https?:(?:\\u002F|\/)[^"\s]+?\.mp4[^"\s]*/g)].map((m) => m[0].replace(/\\u002F/g, "/")))];
console.log("distinct mp4 urls:", mp4s.length);
for (const u of mp4s.slice(0, 4)) console.log("  ", new URL(u).host, new URL(u).pathname.slice(0, 60));

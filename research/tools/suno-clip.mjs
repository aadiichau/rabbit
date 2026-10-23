// Prints the clip record embedded in a public Suno song page (Next.js flight data), to see which
// media the page offers signed-out visitors. Usage: node suno-clip.mjs <suno.com/song/<uuid>>
const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const page = await fetch(process.argv[2], { headers: { "user-agent": UA }, signal: AbortSignal.timeout(20000) });
const html = await page.text();
// Flight chunks: self.__next_f.push([1,"<escaped string>"])
const chunks = [...html.matchAll(/self\.__next_f\.push\(\[1,"((?:[^"\\]|\\.)*)"\]\)/g)].map((m) => JSON.parse(`"${m[1]}"`)).join("");
const id = new URL(page.url).pathname.split("/").pop();
const at = chunks.indexOf(`"id":"${id}"`);
let start = chunks.lastIndexOf("{", at);
// Walk outwards until the object containing video_url parses.
for (let tries = 0; tries < 20 && start >= 0; tries++) {
  let depth = 0, end = -1;
  for (let i = start, s = false, e = false; i < chunks.length; i++) {
    const c = chunks[i];
    if (s) { if (e) e = false; else if (c === "\\") e = true; else if (c === '"') s = false; continue; }
    if (c === '"') s = true; else if (c === "{") depth++; else if (c === "}" && --depth === 0) { end = i + 1; break; }
  }
  try {
    const o = JSON.parse(chunks.slice(start, end));
    if (o.video_url || o.audio_url) {
      const { metadata, ...rest } = o;
      console.log(JSON.stringify(rest, null, 1).slice(0, 2500));
      console.log("metadata keys:", Object.keys(metadata ?? {}).join(","), "duration:", metadata?.duration);
      break;
    }
  } catch {}
  start = chunks.lastIndexOf("{", start - 1);
}

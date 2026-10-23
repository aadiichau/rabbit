// Lists the URLs in yt-dlp extractor _TESTS blocks with their flags.
// Usage: node ytdlp-tests.mjs <extractor.py.txt> [ClassName ...]
import { readFileSync } from "node:fs";

const [, , file, ...classes] = process.argv;
const src = readFileSync(file, "utf8");
const classRe = /^class (\w+)\(/gm;
const bounds = [];
for (let m; (m = classRe.exec(src)); ) bounds.push({ name: m[1], at: m.index });

for (let i = 0; i < bounds.length; i++) {
  const { name, at } = bounds[i];
  if (classes.length && !classes.includes(name)) continue;
  const body = src.slice(at, bounds[i + 1]?.at ?? src.length);
  const testsAt = body.indexOf("_TESTS = [");
  if (testsAt < 0) continue;
  // Split the test list into entries at "'url':" occurrences.
  const tests = body.slice(testsAt).split(/\{\s*'url':\s*/).slice(1);
  for (const t of tests) {
    const url = t.match(/^'([^']+)'/)?.[1] ?? t.match(/^"([^"]+)"/)?.[1];
    if (!url) continue;
    const entry = t.split(/\n\s*\}, \{|\n\s*\}\]/)[0];
    const flags = [];
    if (/'only_matching':\s*True/.test(entry)) flags.push("only_matching");
    if (/'skip':/.test(entry)) flags.push("skip:" + (entry.match(/'skip':\s*'([^']*)'/)?.[1] ?? "?").slice(0, 40));
    if (/'playlist_count'|'playlist_mincount'|'playlist':/.test(entry)) flags.push("playlist");
    const ext = entry.match(/'ext':\s*'(\w+)'/)?.[1];
    console.log(`${name}\t${flags.join(",") || "real"}\t${ext ?? ""}\t${url}`);
  }
}

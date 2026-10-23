// Turns live `yt-dlp -J` output into a test fixture with signed URLs and tokens removed.
// Usage: node scripts/make-fixture.mjs <raw.json> <out.json>
import { readFileSync, writeFileSync } from "node:fs";

const [, , input, output] = process.argv;
const doc = JSON.parse(readFileSync(input, "utf8").replace(/^﻿/, ""));
const DROP = new Set(["fragments", "cookies", "_version", "requested_downloads", "automatic_captions", "heatmap"]);

const scrub = (node, key) => {
  if (Array.isArray(node)) return node.map((n) => scrub(n, key));
  if (node && typeof node === "object") {
    const out = {};
    for (const [k, v] of Object.entries(node)) if (!DROP.has(k)) out[k] = scrub(v, k);
    return out;
  }
  if (typeof node === "string" && /^https?:\/\//.test(node)) {
    const u = new URL(node);
    const isThumb = /^(thumbnail|url)$/.test(key ?? "") && /ytimg|\.jpg|\.webp/.test(u.pathname + u.host);
    return isThumb ? `${u.origin}${u.pathname}` : `https://redacted.invalid/${key ?? "url"}${u.pathname.slice(-24)}`;
  }
  return node;
};

writeFileSync(output, JSON.stringify(scrub(doc), null, 1));
console.log(`fixture written: ${output}`);

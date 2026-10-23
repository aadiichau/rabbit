// Prints the format fields Rabbit's normaliser relies on, from a `yt-dlp -J` dump.
// Usage: node formats.mjs <dump.json>
import { readFileSync } from "node:fs";

const raw = readFileSync(process.argv[2], "utf8").replace(/^﻿/, "").trim();
if (!raw.startsWith("{")) {
  console.log("not JSON:", raw.slice(0, 300));
  process.exit(0);
}
const d = JSON.parse(raw);
console.log(`_type=${d._type ?? "video"} extractor=${d.extractor_key} title=${JSON.stringify(d.title)} duration=${d.duration} entries=${d.entries?.length ?? "-"}`);
const show = (fmts, indent = "") => {
  for (const f of fmts ?? []) {
    console.log(
      `${indent}${String(f.format_id).padEnd(28)} ext=${String(f.ext).padEnd(5)} v=${String(f.vcodec).padEnd(14)} a=${String(f.acodec).padEnd(12)} ` +
        `vext=${String(f.video_ext).padEnd(5)} aext=${String(f.audio_ext).padEnd(5)} ${f.width ?? "?"}x${f.height ?? "?"} proto=${f.protocol} url=${f.url ? "yes" : "no"} size=${f.filesize ?? f.filesize_approx ?? "?"}`,
    );
  }
};
show(d.formats);
if (!d.formats && d.url) console.log("root-level url, ext=", d.ext);
for (const [i, e] of (d.entries ?? []).entries()) {
  console.log(`-- entry ${i + 1}: ${e.title ?? ""} ${e.url ? "(root url)" : ""}`);
  show(e.formats, "   ");
}

// Checks how RedNote's CDN serves a note's images and video: as published (with imageView2
// parameters), without parameters, and over https. Usage: node rednote-media.mjs <note dump json>
import { readFileSync } from "node:fs";
const note = JSON.parse(readFileSync(process.argv[2], "utf8")).state.noteData.data.noteData;
const head = async (label, url, headers = {}) => {
  try {
    const r = await fetch(url, { headers: { range: "bytes=0-15", ...headers }, signal: AbortSignal.timeout(20000) });
    const buf = Buffer.from(await r.arrayBuffer());
    console.log(label.padEnd(22), r.status, r.headers.get("content-type"), r.headers.get("content-range") ?? r.headers.get("content-length"), buf.subarray(0, 12).toString("hex"));
  } catch (e) {
    console.log(label.padEnd(22), "ERR", e.cause?.code ?? e.message);
  }
};
for (const img of note.imageList) {
  console.log("image url:", img.url);
  await head("as published", img.url);
  await head("no params", img.url.split("?")[0]);
  await head("https no params", img.url.split("?")[0].replace(/^http:/, "https:"));
}
for (const s of note.video?.media?.stream?.h264 ?? []) {
  await head("video master", s.masterUrl);
  await head("video https", s.masterUrl.replace(/^http:/, "https:"));
  for (const b of s.backupUrls ?? []) await head("video backup", b);
}

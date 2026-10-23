// How a CDN URL handles N parallel range requests (Rabbit's segmented engine) vs one request.
// Usage: node range-test.mjs <url> <parallel> [referer]
const [url, n = "8", referer = "https://www.bilibili.com/"] = process.argv.slice(2);
const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const h = { "user-agent": UA, referer };
// Rabbit's engine probes with an open-ended range and keeps reading that response as segment 1.
const openEnded = process.argv.includes("--open-ended");
const head = await fetch(url, { headers: { ...h, range: openEnded ? "bytes=0-" : "bytes=0-0" } });
const total = Number(head.headers.get("content-range")?.split("/")[1]);
console.log("probe", head.status, "total", total, "server", head.headers.get("server"));
const parts = Number(n);
const size = Math.ceil(total / parts);
const t = Date.now();
const res = await Promise.all(
  Array.from({ length: parts }, async (_, i) => {
    const start = i * size, end = Math.min(total, start + size) - 1;
    try {
      const r = await fetch(url, { headers: { ...h, range: `bytes=${start}-${end}` } });
      const b = await r.arrayBuffer();
      return `${r.status}:${b.byteLength}`;
    } catch (e) { return `ERR:${e.cause?.code ?? e.message}`; }
  }),
);
console.log(`${parts} parallel ranges in ${Date.now() - t} ms:`, res.join(" "));

// Probes Bilibili's public web API as an anonymous visitor (what the bilibili.com player calls).
// Usage: node bilibili-probe.mjs <BV id>
const bvid = process.argv[2];
const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const H = { "user-agent": UA, referer: "https://www.bilibili.com/" };
const get = async (url, extra = {}) => {
  const r = await fetch(url, { headers: { ...H, ...extra }, signal: AbortSignal.timeout(20000) });
  return { status: r.status, body: await r.json().catch(() => null), setCookie: r.headers.getSetCookie?.() ?? [] };
};

// Anonymous device cookies, as a fresh browser gets them: the homepage, then the public fingerprint endpoint.
const home = await fetch("https://www.bilibili.com/", { headers: H, signal: AbortSignal.timeout(20000) });
const jar = new Map();
for (const c of home.headers.getSetCookie()) {
  const [kv] = c.split(";");
  const [k, v] = kv.split("=");
  jar.set(k.trim(), v);
}
console.log("homepage", home.status, "cookies:", [...jar.keys()].join(","));
const spi = await get("https://api.bilibili.com/x/frontend/finger/spi");
if (spi.body?.data?.b_3) jar.set("buvid3", spi.body.data.b_3);
if (spi.body?.data?.b_4) jar.set("buvid4", spi.body.data.b_4);
console.log("finger/spi", spi.status, "code", spi.body?.code, "→ cookies:", [...jar.keys()].join(","));
H.cookie = [...jar].map(([k, v]) => `${k}=${v}`).join("; ");

const view = await get(`https://api.bilibili.com/x/web-interface/view?bvid=${bvid}`);
console.log("view:", view.status, "code", view.body?.code, view.body?.message);
const d = view.body?.data;
if (!d) process.exit(0);
console.log({ title: d.title, owner: d.owner?.name, duration: d.duration, pages: d.pages?.length, cid: d.cid, pic: !!d.pic, rights: d.rights });

// Plain (unsigned) playurl first; DASH (fnval 16) + HDR/4K/AV1 flags = 4048.
for (const [name, url] of [
  ["playurl", `https://api.bilibili.com/x/player/playurl?bvid=${bvid}&cid=${d.cid}&fnval=4048&fourk=1`],
  ["playurl-mp4", `https://api.bilibili.com/x/player/playurl?bvid=${bvid}&cid=${d.cid}&fnval=1&qn=64`],
]) {
  const p = await get(url);
  const pd = p.body?.data;
  console.log(`${name}:`, p.status, "code", p.body?.code, p.body?.message, "accept_quality", pd?.accept_quality, "quality", pd?.quality);
  if (pd?.dash) {
    for (const v of pd.dash.video ?? []) console.log(`  video id=${v.id} ${v.width}x${v.height} ${v.codecs} ${Math.round(v.bandwidth / 1000)}kbps host=${new URL(v.baseUrl ?? v.base_url).host}`);
    for (const a of pd.dash.audio ?? []) console.log(`  audio id=${a.id} ${a.codecs} ${Math.round(a.bandwidth / 1000)}kbps`);
  }
  if (pd?.durl) for (const s of pd.durl) console.log(`  durl ${s.size}B ${Math.round(s.length / 1000)}s host=${new URL(s.url).host}`);
}

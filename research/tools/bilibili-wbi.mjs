// Bilibili via the WBI-signed web API (the route of yt-dlp PR #17898), as an anonymous visitor.
// Usage: node bilibili-wbi.mjs <BV id>
import { createHash } from "node:crypto";

const bvid = process.argv[2];
const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const H = { "user-agent": UA, referer: `https://www.bilibili.com/video/${bvid}/`, origin: "https://www.bilibili.com" };

const home = await fetch("https://www.bilibili.com/", { headers: H });
const jar = new Map(home.headers.getSetCookie().map((c) => c.split(";")[0].split("=")).map(([k, v]) => [k.trim(), v]));
const spi = await (await fetch("https://api.bilibili.com/x/frontend/finger/spi", { headers: H })).json();
if (spi?.data?.b_3) jar.set("buvid3", spi.data.b_3);
if (spi?.data?.b_4) jar.set("buvid4", spi.data.b_4);
H.cookie = [...jar].map(([k, v]) => `${k}=${v}`).join("; ");

// Mixin-key permutation used by bilibili.com's web client (same table as yt-dlp's _sign_wbi).
const TAB = [46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29, 28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52];
const nav = await (await fetch("https://api.bilibili.com/x/web-interface/nav", { headers: H })).json();
const stem = (u) => u.split("/").pop().split(".")[0];
const raw = stem(nav.data.wbi_img.img_url) + stem(nav.data.wbi_img.sub_url);
const mixin = TAB.map((i) => raw[i]).join("").slice(0, 32);
const sign = (params) => {
  const q = { ...params, wts: Math.floor(Date.now() / 1000) };
  const s = Object.keys(q).sort().map((k) => `${encodeURIComponent(k)}=${encodeURIComponent(String(q[k]).replace(/[!'()*]/g, ""))}`).join("&");
  return `${s}&w_rid=${createHash("md5").update(s + mixin).digest("hex")}`;
};
console.log("nav code", nav.code, "(anonymous:", !nav.data.isLogin, ") mixin key derived:", mixin.length === 32);

const detailRes = await fetch(`https://api.bilibili.com/x/web-interface/wbi/view/detail?${sign({ bvid })}`, { headers: H });
const detail = await detailRes.json().catch(() => null);
console.log("view/detail", detailRes.status, "code", detail?.code, detail?.message);
const v = detail?.data?.View;
if (!v) process.exit(0);
console.log({ title: v.title, owner: v.owner?.name, duration: v.duration, pages: v.pages?.length, cid: v.cid, pic: v.pic?.slice(0, 40) });

// `try_look=1` is what bilibili.com's own player sends for signed-out visitors (yt-dlp sends it too).
const extra = process.argv[3] === "--try-look" ? { try_look: 1 } : {};
// `--mp4`: the progressive (single-file, muxed) form, fnval 1.
const query = process.argv[3] === "--mp4" ? { bvid, cid: v.cid, fnval: 1, qn: 64 } : { bvid, cid: v.cid, fnval: 4048, fourk: 1, qn: 0, ...extra };
const playRes = await fetch(`https://api.bilibili.com/x/player/wbi/playurl?${sign(query)}`, { headers: H });
const play = await playRes.json().catch(() => null);
const pd = play?.data;
console.log("playurl", playRes.status, "code", play?.code, play?.message, "accept_quality", pd?.accept_quality, "accept_description", pd?.accept_description);
for (const s of pd?.dash?.video ?? []) console.log(`  video qn=${s.id} ${s.width}x${s.height} ${s.codecs} ${Math.round(s.bandwidth / 1000)}kbps ${new URL(s.baseUrl).host}`);
for (const s of pd?.dash?.audio ?? []) console.log(`  audio id=${s.id} ${s.codecs} ${Math.round(s.bandwidth / 1000)}kbps`);
console.log("format", pd?.format, "quality", pd?.quality);
for (const s of pd?.durl ?? []) {
  console.log(`  durl ${s.size}B ${Math.round(s.length / 1000)}s ${new URL(s.url).host}`);
  const r = await fetch(s.url, { headers: { "user-agent": UA, referer: "https://www.bilibili.com/", range: "bytes=0-0" } });
  console.log("  durl range probe:", r.status, r.headers.get("content-range"));
}
if (pd?.dash?.video?.[0]) {
  const u = pd.dash.video.at(-1).baseUrl;
  const head = await fetch(u, { headers: { "user-agent": UA, referer: "https://www.bilibili.com/", range: "bytes=0-0" } });
  console.log("CDN range probe:", head.status, head.headers.get("content-range"));
}

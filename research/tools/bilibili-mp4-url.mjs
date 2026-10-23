// Prints the signed-out progressive MP4 URL (fnval 1) for a BV id, for inspection with ffprobe.
// Usage: node bilibili-mp4-url.mjs <BV id> [qn]
import { createHash } from "node:crypto";
const [bvid, qn = "80"] = process.argv.slice(2);
const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const H = { "user-agent": UA, referer: `https://www.bilibili.com/video/${bvid}/`, origin: "https://www.bilibili.com" };
const home = await fetch("https://www.bilibili.com/", { headers: H });
const jar = new Map(home.headers.getSetCookie().map((c) => c.split(";")[0].split("=")).map(([k, v]) => [k.trim(), v]));
const spi = await (await fetch("https://api.bilibili.com/x/frontend/finger/spi", { headers: H })).json();
if (spi?.data?.b_3) jar.set("buvid3", spi.data.b_3);
if (spi?.data?.b_4) jar.set("buvid4", spi.data.b_4);
H.cookie = [...jar].map(([k, v]) => `${k}=${v}`).join("; ");
const TAB = [46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29, 28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52];
const nav = await (await fetch("https://api.bilibili.com/x/web-interface/nav", { headers: H })).json();
const stem = (u) => u.split("/").pop().split(".")[0];
const raw = stem(nav.data.wbi_img.img_url) + stem(nav.data.wbi_img.sub_url);
const mixin = TAB.map((i) => raw[i]).join("").slice(0, 32);
const sign = (p) => {
  const q = { ...p, wts: Math.floor(Date.now() / 1000) };
  const s = Object.keys(q).sort().map((k) => `${k}=${encodeURIComponent(String(q[k]))}`).join("&");
  return `${s}&w_rid=${createHash("md5").update(s + mixin).digest("hex")}`;
};
const detail = await (await fetch(`https://api.bilibili.com/x/web-interface/wbi/view/detail?${sign({ bvid })}`, { headers: H })).json();
const cid = detail.data.View.cid;
const play = await (await fetch(`https://api.bilibili.com/x/player/wbi/playurl?${sign({ bvid, cid, fnval: 1, qn })}`, { headers: H })).json();
console.error("quality", play.data.quality, "format", play.data.format, "segments", play.data.durl.length);
console.log(play.data.durl[0].url);

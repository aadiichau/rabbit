// Probes Douyin's public mobile share page (www.iesdouyin.com/share/video/<id>/) for post data,
// as a phone browser opening a share link sees it. Usage: node douyin-share.mjs <douyin url or video id>
const UA = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
let arg = process.argv[2];
let id = arg.match(/(\d{15,20})/)?.[1];
if (!id) {
  // Short link: follow redirects manually and read the id from the address.
  let url = arg;
  for (let i = 0; i < 6 && !id; i++) {
    const r = await fetch(url, { headers: { "user-agent": UA }, redirect: "manual", signal: AbortSignal.timeout(20000) });
    const loc = r.headers.get("location");
    console.log("hop", r.status, new URL(url).host + new URL(url).pathname);
    if (!loc) break;
    url = new URL(loc, url).href;
    id = url.match(/\/(?:video|note)\/(\d{15,20})/)?.[1];
  }
}
console.log("id", id);
const page = await fetch(`https://www.iesdouyin.com/share/video/${id}/`, { headers: { "user-agent": UA }, signal: AbortSignal.timeout(20000) });
const html = await page.text();
console.log("share page", page.status, html.length, "bytes", "| _ROUTER_DATA:", html.includes("_ROUTER_DATA"), "| captcha:", /captcha|verify/i.test(html.slice(0, 5000)));
const at = html.indexOf("window._ROUTER_DATA");
if (at < 0) process.exit(0);
const start = html.indexOf("{", at);
let depth = 0, end = 0;
for (let i = start, s = false, e = false; i < html.length; i++) {
  const c = html[i];
  if (s) { if (e) e = false; else if (c === "\\") e = true; else if (c === '"') s = false; continue; }
  if (c === '"') s = true; else if (c === "{") depth++; else if (c === "}" && --depth === 0) { end = i + 1; break; }
}
const data = JSON.parse(html.slice(start, end));
for (const [k, v] of Object.entries(data.loaderData ?? {})) console.log("loaderData." + k, "keys:", v && Object.keys(v).join(","));
const pv = data.loaderData?.["video_(id)/page"];
if (pv) for (const k of Object.keys(pv)) if (!["ua", "query"].includes(k)) console.log("  ", k, "=", JSON.stringify(pv[k])?.slice(0, 200));
const pageKey = Object.keys(data.loaderData ?? {}).find((k) => data.loaderData[k]?.videoInfoRes);
const item = data.loaderData?.[pageKey]?.videoInfoRes?.item_list?.[0];
console.log("loader key", pageKey, "| status", JSON.stringify(data.loaderData?.[pageKey]?.videoInfoRes?.status_code), "filter:", JSON.stringify(data.loaderData?.[pageKey]?.videoInfoRes?.filter_list?.[0] ?? null));
if (!item) process.exit(0);
const v = item.video ?? {};
console.log({ desc: item.desc?.slice(0, 40), author: item.author?.nickname, aweme_type: item.aweme_type, images: item.images?.length ?? 0, duration: v.duration, w: v.width, h: v.height, ratio: v.ratio });
console.log("play_addr uri", v.play_addr?.uri, "urls:", (v.play_addr?.url_list ?? []).map((u) => u.slice(0, 90)));
console.log("bit_rate list:", (v.bit_rate ?? []).map((b) => `${b.gear_name} ${b.play_addr?.width}x${b.play_addr?.height}`).join(", ") || "-");
if (item.images?.length) console.log("image[0] urls:", item.images[0].url_list?.map((u) => u.slice(0, 90)), item.images[0].width, item.images[0].height);
const u = v.play_addr?.url_list?.[0];
if (u) {
  for (const variant of [u, u.replace("/playwm/", "/play/")]) {
    const r = await fetch(variant, { headers: { "user-agent": UA, range: "bytes=0-15" }, redirect: "follow", signal: AbortSignal.timeout(20000) });
    const b = Buffer.from(await r.arrayBuffer());
    console.log(variant.includes("/playwm/") ? "playwm" : "play  ", r.status, r.headers.get("content-type"), r.headers.get("content-range"), new URL(r.url).host, b.subarray(4, 8).toString());
  }
}

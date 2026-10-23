// Captures the README / website screenshots from a running Rabbit window (fresh profile, neutral
// download folder) at 2x pixel density. Usage: node ui-shots.mjs <out dir>
import { mkdirSync, writeFileSync } from "node:fs";
const out = process.argv[2];
mkdirSync(out, { recursive: true });
const targets = await (await fetch("http://127.0.0.1:9222/json/list")).json();
const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
let seq = 0;
const pending = new Map();
ws.onmessage = (m) => { const msg = JSON.parse(m.data); pending.get(msg.id)?.(msg); pending.delete(msg.id); };
const send = (method, params = {}) => new Promise((r) => { const id = ++seq; pending.set(id, r); ws.send(JSON.stringify({ id, method, params })); });
const run = async (js) => (await send("Runtime.evaluate", { expression: js, awaitPromise: true, returnByValue: true })).result?.result?.value;
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const click = (text) => run(`[...document.querySelectorAll('button')].find(b => b.textContent.trim().startsWith(${JSON.stringify(text)}))?.click()`);
const shot = async (name) => {
  const r = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  writeFileSync(`${out}/${name}.png`, Buffer.from(r.result.data, "base64"));
  console.log("saved", name);
};
const analyze = async (url) => {
  await click("Home");
  await wait(300);
  await run(`(() => { const i = document.querySelector('input[placeholder="Paste a link or share text"]'); const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set; set.call(i, ${JSON.stringify(url)}); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
  await wait(300);
  await click("Analyze");
  for (let i = 0; i < 120; i++) {
    await wait(500);
    if (await run(`/options · [0-9.]+ s/.test(document.querySelector('main').innerText)`)) break;
  }
  await wait(800); // thumbnail
};

await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 800, deviceScaleFactor: 2, mobile: false });
// Neutral download folder (no user name in the path) for this fresh profile.
await run(`(async () => { const i = window.__TAURI_INTERNALS__; const s = await i.invoke('get_settings'); await i.invoke('set_settings', { settings: { ...s, downloadDir: 'C:\\\\Users\\\\Public\\\\Downloads\\\\Rabbit' } }); })()`);
await run(`location.reload()`);
await wait(2500);

await click("Home");
await wait(500);
await shot("01-home-empty");

await analyze("https://www.youtube.com/watch?v=aqz-KE-bpKQ");
await shot("02-analyze-youtube-4k");

// NASA imagery is public domain.
await analyze("https://www.instagram.com/p/Dbd3EBdnW_u/");
await shot("03-analyze-image");

// Downloads of Blender open movies (CC BY), captured while running.
await analyze("https://www.youtube.com/watch?v=eRsGyueVLvQ");
await click("Download");
await wait(500);
await analyze("https://www.youtube.com/watch?v=WhWc3b3KhnY");
await click("Download");
await wait(500);
await analyze("https://www.youtube.com/watch?v=R6MlUcmOul8");
await click("Download");
await wait(500);
await click("Queue");
for (let i = 0; i < 40; i++) {
  await wait(500);
  if (await run(`/MB\\/s/.test(document.querySelector('main').innerText) && document.querySelectorAll('main .segs').length > 0`)) break;
}
await wait(2500);
await shot("04-queue-downloading");
for (let i = 0; i < 400; i++) {
  await wait(1000);
  if (await run(`/Active 0/.test(document.querySelector('main').innerText)`)) break;
}
await wait(800);
await shot("05-queue-done");
await click("Library");
await wait(1200);
await shot("06-library");
await click("Settings");
await wait(1200);
await shot("07-settings");
await send("Emulation.clearDeviceMetricsOverride");
ws.close();

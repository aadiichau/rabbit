// After ui-analyze.mjs: presses the Download button, opens the Queue, samples the newest row
// while it runs and saves screenshots (progress and final). Usage: node ui-download.mjs <shot-prefix>
import { writeFileSync } from "node:fs";
const targets = await (await fetch("http://127.0.0.1:9222/json/list")).json();
const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
let seq = 0;
const pending = new Map();
ws.onmessage = (m) => { const msg = JSON.parse(m.data); pending.get(msg.id)?.(msg); pending.delete(msg.id); };
const send = (method, params = {}) => new Promise((r) => { const id = ++seq; pending.set(id, r); ws.send(JSON.stringify({ id, method, params })); });
const run = async (js) => (await send("Runtime.evaluate", { expression: js, awaitPromise: true, returnByValue: true })).result?.result?.value;
const shot = async (path) => writeFileSync(path, Buffer.from((await send("Page.captureScreenshot", { format: "png" })).result.data, "base64"));
const prefix = process.argv[2];

const title = await run(`document.querySelector('main h2, main h1:not(:first-child)')?.textContent ?? ''`);
await run(`[...document.querySelectorAll('button')].find(b => b.textContent.trim().startsWith('Download'))?.click()`);
await new Promise((r) => setTimeout(r, 400));
await run(`[...document.querySelectorAll('button')].find(b => b.textContent.trim().startsWith('Queue'))?.click()`);
const t0 = Date.now();
let last = "", progressShot = false;
for (let i = 0; i < 480; i++) {
  await new Promise((r) => setTimeout(r, 250));
  const row = await run(`(() => {
    const main = document.querySelector('main') ?? document.body;
    return main.innerText.split('\\n').slice(0, 24).join(' | ');
  })()`);
  if (row !== last && i % 4 === 0) console.log(`${((Date.now() - t0) / 1000).toFixed(1)}s ${row.slice(0, 260)}`);
  last = row;
  if (!progressShot && /\d+(\.\d+)? ?(MB|KB)\/s/.test(row)) { await shot(`${prefix}-progress.png`); progressShot = true; }
  // The newest row follows the filter tabs ("Failed 0" etc.) and the "Pause all"/"Resume all" button.
  const firstRow = row.split(" | ").slice(6, 14).join(" ");
  if (/Active 0/.test(row) && /Done [1-9]|Failed [1-9]/.test(row) && i > 2) break;
  if (/Completed|failed|Cancelled|Open file|Show in folder|Retry/.test(firstRow)) break;
}
await new Promise((r) => setTimeout(r, 300));
await shot(`${prefix}-final.png`);
console.log("title:", title, "| final:", last.slice(0, 400));
ws.close();

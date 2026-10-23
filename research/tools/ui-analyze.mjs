// Drives the Rabbit dev window through its real UI: types a link, presses Analyze, waits for
// the result, prints what the preview shows and saves a screenshot. Usage:
//   node ui-analyze.mjs <url> <shot.png>
import { writeFileSync } from "node:fs";
const targets = await (await fetch("http://127.0.0.1:9222/json/list")).json();
const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
let seq = 0;
const pending = new Map();
ws.onmessage = (m) => { const msg = JSON.parse(m.data); pending.get(msg.id)?.(msg); pending.delete(msg.id); };
const send = (method, params = {}) => new Promise((r) => { const id = ++seq; pending.set(id, r); ws.send(JSON.stringify({ id, method, params })); });
const run = async (js) => (await send("Runtime.evaluate", { expression: js, awaitPromise: true, returnByValue: true })).result?.result?.value;

const [url, shot] = process.argv.slice(2);
await run(`[...document.querySelectorAll('button')].find(b => b.textContent.trim().startsWith('Home'))?.click()`);
await new Promise((r) => setTimeout(r, 300));
const statusLine = `(document.querySelector('main')?.innerText.match(/✓ \\d+ options · [0-9.]+ s · [^\\n]*/)?.[0] ?? '')`;
const before = await run(statusLine);
await run(`(() => {
  const input = document.querySelector('input[placeholder="Paste a link or share text"]');
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
  set.call(input, ${JSON.stringify(url)});
  input.dispatchEvent(new Event('input', { bubbles: true }));
})()`);
await new Promise((r) => setTimeout(r, 300));
const t0 = Date.now();
await run(`[...document.querySelectorAll('button')].find(b => b.textContent.startsWith('Analyze'))?.click()`);
let state;
for (let i = 0; i < 120; i++) {
  await new Promise((r) => setTimeout(r, 500));
  state = await run(`(() => {
    const main = document.querySelector('main') ?? document.body;
    const text = main.innerText;
    const status = ${statusLine};
    const done = (status && status !== ${JSON.stringify(before)}) || /(Couldn't read|isn't supported|requires signing in|Network problem|only shows notes)/.test(text);
    return { done, text: text.slice(0, 1500) };
  })()`);
  if (state?.done) break;
}
console.log(`ui wait ${Date.now() - t0} ms`);
console.log(state?.text);
const img = await send("Page.captureScreenshot", { format: "png" });
writeFileSync(shot, Buffer.from(img.result.data, "base64"));
ws.close();

// Opens Settings in the running Rabbit window, screenshots the Signed-in sites card while off,
// switches the master switch on (no source chosen, no cookies read), screenshots it, and switches it
// off again. Usage: node ui-sessions.mjs <shot-prefix>
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
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const prefix = process.argv[2];

await run(`[...document.querySelectorAll('button')].find(b => b.textContent.trim().startsWith('Settings'))?.click()`);
await wait(600);
await run(`document.querySelector('[aria-label="Signed-in sites"]')?.scrollIntoView({block: 'center'})`);
await wait(200);
console.log("off:", await run(`document.querySelector('[aria-label="Signed-in sites"]')?.getAttribute('aria-checked')`));
await shot(`${prefix}-off.png`);
await run(`document.querySelector('[aria-label="Signed-in sites"]').click()`);
await wait(1200);
await run(`document.querySelector('[aria-label="Session source"]')?.scrollIntoView({block: 'center'})`);
await wait(200);
const text = await run(`(() => { const r = document.querySelector('[aria-label="Session source"]')?.closest('.grid.gap-3'); return r ? r.innerText : 'card not found'; })()`);
console.log("on:\n" + text);
console.log("site switches disabled until a source is chosen:", await run(`[...document.querySelectorAll('[role=switch]')].filter(b => /session$/.test(b.getAttribute('aria-label'))).map(b => b.disabled)`));
await shot(`${prefix}-on.png`);
await run(`document.querySelector('[aria-label="Signed-in sites"]').click()`);
await wait(800);
console.log("back off:", await run(`document.querySelector('[aria-label="Signed-in sites"]')?.getAttribute('aria-checked')`));
ws.close();

// Minimal Chrome DevTools Protocol driver for the Rabbit dev window (WebView2 started with
// --remote-debugging-port=9222). Usage:
//   node cdp.mjs eval "<expression>"      evaluates in the page (awaits promises), prints JSON
//   node cdp.mjs shot <out.png>           saves a screenshot of the window
import { writeFileSync } from "node:fs";
const targets = await (await fetch("http://127.0.0.1:9222/json/list")).json();
const page = targets.find((t) => t.type === "page");
if (!page) throw new Error("no page target on :9222");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
let seq = 0;
const pending = new Map();
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
};
const send = (method, params = {}) => new Promise((r) => { const id = ++seq; pending.set(id, r); ws.send(JSON.stringify({ id, method, params })); });
const [cmd, arg] = process.argv.slice(2);
if (cmd === "eval") {
  const res = await send("Runtime.evaluate", { expression: arg, awaitPromise: true, returnByValue: true });
  console.log(JSON.stringify(res.result?.result?.value ?? res.result?.exceptionDetails?.exception?.description ?? res, null, 1));
} else if (cmd === "shot") {
  const res = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(arg, Buffer.from(res.result.data, "base64"));
  console.log("saved", arg);
}
ws.close();

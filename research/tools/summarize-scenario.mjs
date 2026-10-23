// Usage: node summarize-scenario.mjs <rea-output.json> <out-prefix>
import { readFileSync, writeFileSync } from "node:fs";

const [, , input, prefix] = process.argv;
const raw = readFileSync(input, "utf8").replace(/^﻿/, "");
const doc = JSON.parse(raw);

const keysOf = (o, depth = 0, path = "") => {
  if (depth > 3 || o === null || typeof o !== "object") return [];
  return Object.entries(o).flatMap(([k, v]) => {
    const p = path ? `${path}.${k}` : k;
    const kind = Array.isArray(v) ? `array[${v.length}]` : typeof v;
    return [`${p}: ${kind}`, ...(Array.isArray(v) ? [] : keysOf(v, depth + 1, p))];
  });
};

const found = { screenshots: [], accessibility: [], requests: [], console: [], errors: [], evidenceIds: new Set() };

const walk = (node, path) => {
  if (node === null || typeof node !== "object") return;
  if (Array.isArray(node)) { node.forEach((n, i) => walk(n, `${path}[${i}]`)); return; }
  for (const [k, v] of Object.entries(node)) {
    if ((k === "evidence_id" || k === "id") && typeof v === "string" && /^ev|evidence/i.test(v)) found.evidenceIds.add(v);
    if (k === "evidence_id" && typeof v === "string") found.evidenceIds.add(v);
    if (typeof v === "string" && /^iVBORw0KGgo/.test(v)) found.screenshots.push({ path: `${path}.${k}`, b64: v });
    if (typeof v === "string" && /accessib|snapshot_text|aria/i.test(k) && v.length > 40) found.accessibility.push(v);
    if (k === "url" && typeof v === "string" && /^https?:/.test(v)) found.requests.push(v);
    if (/console/i.test(path) && k === "text" && typeof v === "string") found.console.push(v);
    if (/error/i.test(path) && k === "message" && typeof v === "string") found.errors.push(v);
    walk(v, `${path}.${k}`);
  }
};
walk(doc, "$");

const lines = [];
lines.push("# Top-level structure");
lines.push(...keysOf(doc).slice(0, 120));
lines.push("", "# Evidence IDs", ...found.evidenceIds);
const hosts = new Map();
for (const u of found.requests) {
  try {
    const url = new URL(u);
    const key = `${url.host}${url.pathname.replace(/\/[0-9a-f-]{16,}|\.[0-9a-f]{8,}\./gi, "/*")}`;
    hosts.set(key, (hosts.get(key) ?? 0) + 1);
  } catch {}
}
lines.push("", `# Network URLs (${found.requests.length} total, ${hosts.size} unique paths)`);
for (const [k, n] of [...hosts.entries()].sort()) lines.push(`${n}\t${k}`);
lines.push("", "# Console", ...found.console.slice(0, 50));
lines.push("", "# Errors", ...found.errors.slice(0, 50));
writeFileSync(`${prefix}.summary.txt`, lines.join("\n"));
found.accessibility.forEach((a, i) => writeFileSync(`${prefix}.a11y${i}.txt`, a));
found.screenshots.forEach((s, i) => writeFileSync(`${prefix}.shot${i}.png`, Buffer.from(s.b64, "base64")));
console.log(`summary written; a11y=${found.accessibility.length} shots=${found.screenshots.length} reqs=${found.requests.length}`);

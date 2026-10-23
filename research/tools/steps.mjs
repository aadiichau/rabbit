// Usage: node steps.mjs <rea-output.json> <out-prefix>
// Writes each step's snapshot artifacts (accessibility/dom text, screenshots) and prints a compact outline.
import { readFileSync, writeFileSync } from "node:fs";

const [, , input, prefix] = process.argv;
const doc = JSON.parse(readFileSync(input, "utf8").replace(/^﻿/, ""));
const r = doc.data.normalized_result;
const clip = (_k, v) => (typeof v === "string" && v.length > 160 ? `${v.slice(0, 160)}…[${v.length}]` : v);

console.log("evidence:", doc.data.evidence_id);
console.log("completeness:", JSON.stringify(r.completeness));
r.steps.forEach((step, i) => {
  console.log(`--- step ${i}`, JSON.stringify(step, clip).slice(0, 1500));
  const visit = (node, path) => {
    if (node === null || typeof node !== "object") return;
    for (const [k, v] of Object.entries(node)) {
      if (typeof v === "string" && v.startsWith("iVBORw0KGgo")) writeFileSync(`${prefix}.step${i}.${k}.png`, Buffer.from(v, "base64"));
      else if (typeof v === "string" && v.length > 400) writeFileSync(`${prefix}.step${i}.${path}${k}.txt`, v);
      else if (typeof v === "object") visit(v, `${path}${k}.`);
    }
  };
  visit(step, "");
});
console.log("--- limitations");
for (const l of r.limitations) console.log("-", typeof l === "string" ? l : JSON.stringify(l));

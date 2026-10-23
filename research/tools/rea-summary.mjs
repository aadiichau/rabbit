// Summarizes a REA browser-scenario evidence file: per step, the URL, the "supported link formats"
// paragraph and whether a CAPTCHA image / server-side wording appears. Usage: node rea-summary.mjs <out.json>
import { readFileSync } from "node:fs";
const o = JSON.parse(readFileSync(process.argv[2], "utf8").replace(/^\W+\{/, "{"));
console.log("evidence", o.evidence_id);
for (const s of o.normalized_result.steps) {
  const t = s.artifacts?.accessibility?.value?.text ?? "";
  const formats = t.match(/We support the following link formats:[^\n]*/)?.[0] ?? "-";
  console.log(`${s.step_id} ${s.after_url?.url}\n  formats: ${formats.slice(0, 420)}\n  captcha img: ${t.includes('img "captcha"')} | "our servers": ${t.includes("our servers")}`);
}

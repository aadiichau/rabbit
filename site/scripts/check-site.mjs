// Checks the website before deployment: every local href/src in site/*.html points at a file that
// exists, every external link is https, and no placeholder text is left.
//   node site/scripts/check-site.mjs
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");
const generated = new Set(["stats.json", "badge.json"]); // written by stats.mjs at deploy time
let problems = 0;
for (const file of readdirSync(site).filter((f) => f.endsWith(".html") || f.endsWith(".css") || f.endsWith(".js"))) {
  const text = readFileSync(join(site, file), "utf8");
  const refs = [...text.matchAll(/(?:href|src)="([^"]+)"|url\(["']?([^"')]+)["']?\)|fetch\("([^"]+)"\)/g)].map((m) => m[1] ?? m[2] ?? m[3]);
  for (const ref of refs) {
    if (/^(#|mailto:|data:|javascript:)/.test(ref)) continue;
    if (/^https?:/.test(ref)) {
      if (ref.startsWith("http:")) {
        console.error(`${file}: insecure link ${ref}`);
        problems++;
      }
      continue;
    }
    const path = normalize(join(site, ref.split(/[?#]/)[0]));
    if (!generated.has(ref.replace(/^\.\//, "")) && !existsSync(path)) {
      console.error(`${file}: missing ${ref}`);
      problems++;
    }
  }
  if (/\b(TODO|OWNER|lorem ipsum)\b/i.test(text)) {
    console.error(`${file}: placeholder text`);
    problems++;
  }
}
if (problems) {
  console.error(`${problems} problem(s)`);
  process.exit(1);
}
console.log("site OK");

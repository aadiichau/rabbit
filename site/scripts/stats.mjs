// Generates the website's public download statistics from GitHub's own release data.
//   node site/scripts/stats.mjs --repo owner/name --out site
// Writes <out>/stats.json (for the website) and <out>/badge.json (a shields.io endpoint badge).
//
// What is counted: the `download_count` GitHub reports for the Windows installer (`*_x64-setup.exe`)
// and the portable ZIP (`*_x64-portable.zip`) of every published, non-prerelease release. Checksums,
// source archives and drafts are excluded. A download count is not a count of people or of installs.
// The token (the workflow's read-only GITHUB_TOKEN) is used server-side only and never written out.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const arg = (name) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : undefined;
};
const repo = arg("repo") ?? process.env.GITHUB_REPOSITORY;
const out = arg("out") ?? "site";
if (!repo || !/^[\w.-]+\/[\w.-]+$/.test(repo)) throw new Error("--repo owner/name is required");

const INSTALLER = /_x64-setup\.exe$/i;
const PORTABLE = /_x64-portable\.zip$/i;
const headers = { accept: "application/vnd.github+json", "user-agent": "rabbit-stats", "x-github-api-version": "2022-11-28" };
if (process.env.GITHUB_TOKEN) headers.authorization = `Bearer ${process.env.GITHUB_TOKEN}`;

async function releases() {
  const all = [];
  for (let page = 1; page <= 10; page++) {
    const r = await fetch(`https://api.github.com/repos/${repo}/releases?per_page=100&page=${page}`, { headers, signal: AbortSignal.timeout(30000) });
    if (r.status === 404) return all; // repository not visible yet
    if (!r.ok) throw new Error(`GitHub API ${r.status} (rate limit remaining: ${r.headers.get("x-ratelimit-remaining")})`);
    const batch = await r.json();
    all.push(...batch);
    if (batch.length < 100) break;
  }
  return all;
}

const fmt = (n) =>
  n >= 1e6 ? `${(n / 1e6).toFixed(n >= 1e7 ? 0 : 1)}M` : n >= 1e4 ? `${(n / 1e3).toFixed(n >= 1e5 ? 0 : 1)}k` : n.toLocaleString("en-US");

let stats;
try {
  const stable = (await releases()).filter((r) => !r.draft && !r.prerelease && r.published_at);
  stable.sort((a, b) => Date.parse(b.published_at) - Date.parse(a.published_at));
  const find = (r, pattern) => r.assets.find((a) => pattern.test(a.name) && a.state === "uploaded");
  const count = (r) => (find(r, INSTALLER)?.download_count ?? 0) + (find(r, PORTABLE)?.download_count ?? 0);
  const total = stable.reduce((sum, r) => sum + count(r), 0);
  const latest = stable.find((r) => find(r, INSTALLER));
  const describe = (a) => a && { name: a.name, size: a.size, url: a.browser_download_url, downloads: a.download_count };
  stats = {
    available: true,
    repo,
    generatedAt: new Date().toISOString(),
    stableReleases: stable.length,
    totalDownloads: total,
    latest: latest
      ? {
          tag: latest.tag_name,
          version: latest.tag_name.replace(/^v/, ""),
          name: latest.name,
          publishedAt: latest.published_at,
          releaseUrl: latest.html_url,
          installer: describe(find(latest, INSTALLER)),
          portable: describe(find(latest, PORTABLE)) ?? null,
          checksumsUrl: latest.assets.find((a) => a.name === "SHA256SUMS.txt")?.browser_download_url ?? null,
        }
      : null,
  };
} catch (e) {
  // Never invent numbers: the site shows "unavailable" and falls back to the Releases page.
  console.error(`statistics unavailable: ${e.message}`);
  stats = { available: false, repo, generatedAt: new Date().toISOString(), error: "GitHub release data could not be read" };
}

mkdirSync(out, { recursive: true });
writeFileSync(join(out, "stats.json"), JSON.stringify(stats, null, 2) + "\n");
writeFileSync(
  join(out, "badge.json"),
  JSON.stringify({ schemaVersion: 1, label: "downloads", message: stats.available ? fmt(stats.totalDownloads) : "n/a", color: "e5a23c" }) + "\n",
);
console.log(
  stats.available
    ? `stable releases: ${stats.stableReleases}, downloads: ${stats.totalDownloads}, latest: ${stats.latest?.tag ?? "none"}`
    : "stats unavailable",
);

// Fills the download details and statistics from stats.json, which the Pages workflow generates from GitHub's
// release data at deploy time (site/scripts/stats.mjs). No token or API call happens in the browser. If the file
// is missing or says the data is unavailable, the page keeps its static text and every download button keeps
// pointing at the repository's latest-release page.
(async () => {
  const set = (selector, text) => document.querySelectorAll(selector).forEach((el) => (el.textContent = text));
  const mib = (bytes) => `${(bytes / 1048576).toFixed(1)} MB`;
  const day = (iso) => new Date(iso).toLocaleDateString("en", { year: "numeric", month: "long", day: "numeric" });

  let stats;
  try {
    const response = await fetch("stats.json", { cache: "no-cache" });
    if (!response.ok) throw new Error(String(response.status));
    stats = await response.json();
  } catch {
    set('[data-stat="note"]', "Download statistics are unavailable right now. The download buttons open the latest release on GitHub.");
    return;
  }
  if (!stats.available) {
    set('[data-stat="note"]', "GitHub's release data couldn't be read at the last update, so no numbers are shown.");
    return;
  }

  set('[data-stat="downloads"]', stats.totalInstallerDownloads.toLocaleString("en"));
  set('[data-stat="releases"]', String(stats.stableReleases));

  const latest = stats.latest;
  if (!latest) {
    set('[data-stat="latest"]', "—");
    set('[data-stat="note"]', "No stable release has been published yet.");
    return;
  }
  set('[data-stat="latest"]', latest.version);
  set('[data-field="version"]', latest.version);
  set('[data-field="file"]', latest.installer.name);
  set('[data-field="size"]', mib(latest.installer.size));
  set('[data-field="date"]', day(latest.publishedAt));
  set("[data-release-meta]", `Version ${latest.version} · ${mib(latest.installer.size)} · Windows 10 / 11 · 64-bit · no admin rights needed`);
  document.querySelectorAll("[data-download]").forEach((a) => (a.href = latest.installer.url));
  document.querySelectorAll("[data-notes]").forEach((a) => (a.href = latest.releaseUrl));
  if (latest.checksumsUrl) {
    document.querySelectorAll("[data-checksum] .mono").forEach((el) => {
      const link = document.createElement("a");
      link.href = latest.checksumsUrl;
      link.className = "mono";
      link.textContent = "SHA256SUMS.txt";
      el.replaceWith(link);
    });
  }
  set('[data-stat="note"]', `Counted from GitHub's download numbers for the Windows installer of each published release. Updated ${day(stats.generatedAt)}. A download isn't a person or an install.`);
})();

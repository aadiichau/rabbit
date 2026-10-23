# research/

Development tools used to investigate platforms and capture screenshots. They are not part of the app.

- `tools/*.mjs` — standalone Node scripts that probe a platform's public pages or APIs
  (`node research/tools/<name>.mjs <url>`), summarise test runs, or drive the running app over the WebView2
  debugging port (`ui-*.mjs`, `cdp.mjs`).

Generated evidence referenced by the docs (`platform-report.json`, `quality-audit-*.txt`, regression logs,
downloaded samples, upstream source checkouts) is produced locally by the test suites and these tools. It isn't
committed because it contains downloaded media and machine-specific paths.

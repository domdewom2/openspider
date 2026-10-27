# JavaScript acceptance fixture

Own synthetic source for M2-R02; no reference-product resources or outputs.

- `facts.sql` supplies deterministic edge cases for population, signed word changes,
  zero denominators, exact robots tokens, disjoint text/canonical filters, duplicate
  link targets, link origins, and console severity.
- `scenario.toml` executes real scripts in the locally installed Chromium through
  `os-testsite`. It covers DOM changes, removal of robots directives, HTML versus
  HTTP canonicals, robots resource denial, an HTTP 500 stylesheet, tracking-resource
  exclusion, CSS backgrounds, dynamic images, lazy loading, and navigation timing.
- All `.test` requests resolve to the fixture server. The tracking script request
  to `google-analytics.com/excluded.js` is denied by the renderer policy before
  transmission; the crawl frontier also excludes that path.
- PNGs are generated RGB test data. `image.png` is 800×600 and exactly 20,000 bytes,
  including an ancillary padding chunk. At 400×300 its estimated saving is 15,000
  bytes. `offscreen.png` sits outside the fixed viewport. `screenshot.png` is a
  32×24 saved-capture fixture for export tests.

Coverage: TECH-101–111, TECH-114, TECH-015 [M2], ONPAGE-053/057,
ONPAGE-090/170 [M2], ONPAGE-172/178/180/181, OUT-043. Tests live in
`os-analyze/tests/tab_javascript*.rs`, `tab_titles.rs`, `tab_images.rs`,
`os-export/tests/reports_redirects.rs`, and the two JavaScript/screenshot export
modules. CSV and XLSX checks use the same persisted facts.

`screenshot.jpg` is a synthetic JPEG derived from the same 32×24 PNG fixture.
Mixed PNG/JPEG captures export as PNG, with decoded JPEG pixels preserved.
The current renderer writes one tile (tile 0) per page. Repeated exports replace
the complete screenshot folder, including its index; failed or cancelled
replacements retain the previous folder.

`app_screenshots.rs` supplies the test-only app command regression for this
fixture (mixed formats and a repeated export from three pages to two). The
production export module includes it only under `cfg(test)`, keeping direct
fixture database access outside production source files.

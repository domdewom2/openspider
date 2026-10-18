# M2-D17 export fixtures

All data is authored locally for these tests. No reference-product files are used.

- `paths.sql`: linear discovery, HTTP redirect, equal-length diamond, later depth improvement, sitemap orphan, API-only URL, list root, and queue exclusions. K2-42 governs the report: it retains the first-discovery chain, even after a later depth improvement. The stored first-discovery depth is exported with that chain.
- `panels.sql`: depth 0, two URLs at depth 1 (one 404), eleven at depth 11, response times 100/1000/2500/11000 ms, five instances of one spelling finding on three URLs, segment membership, and API status.
- `queue-site`: loopback crawl with twenty links; a five-URL crawl leaves sixteen queued URLs.
- `timeseries.json`: existing headers and reordered selections for the name-based column alignment test.

`reports::sidebar::export_site_structure` writes two CSVs into a directory or two sheets into one XLSX file. The individual directory/depth entries are also available to report writers. Overview and Issues use the existing shared report/count/text sources. `PanelTable` and `write_snapshot` accept complete ordered session snapshots without depending on the app crate.

`timeseries::catalog` derives numeric metrics from the tab/filter registry; `measure` computes a crawl measurement; `align` preserves existing headers and appends new selections. Date cells carry a numeric spreadsheet serial and the date format `yyyy-mm-dd hh:mm:ss`; the caller supplies the local UTC offset for the crawl start. Sheets, scheduling, CLI and dialogs remain with X03/T01/L01/U27 as specified by K2-48.

Goldens can be regenerated with `UPDATE_D17_GOLDENS=1` on the D17 export test targets. Only `tests/golden/out-m2-reports` is written by that switch.

## Fremdergänzungen (K2-61)

Only `tests/golden/out-cli/list_reports.txt` changes outside the package and the K2-60 stub list. The CLI registry automatically lists the additional D17 sidebar exports; the golden receives these six lines, without changing foreign tests or removing any existing line:

```diff
+Sidebar:Overview
+Sidebar:Issues
+Sidebar:Site Structure Depth
+Sidebar:Segments
+Sidebar:Spelling & Grammar
+Sidebar:API
```

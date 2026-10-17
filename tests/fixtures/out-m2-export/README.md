# M2-X01 export fixtures

Spec-derived SQLite rows for column selection, upload order, repeated link
instances, multi-selection details, cancellation and workbook consolidation.
No reference product data is used.

`basic.ospreset` is a portable TOML selection. `columns.json` deliberately includes
an unknown and a repeated column. `crawl.sql` includes a schemaless ignored list
entry: K2-48 gives CRAWL-027 priority over the older OUT-015 example.

Pagination's existing inlink entries belong to D05. The complete cross-package
check is in `pending_w1_x01.rs`, ignored until M2-D05 (C9/K2-26).

New consumers use `export_tab_with` / `export_view_with` with `ExportCtx` and
`preserve_upload_order`; multi-export enables upload order automatically. The
existing `export_tab` entry point preserves the M1 CLI contract. K06/L01 can adopt
the controlled entry points in their command implementations (K2-25).

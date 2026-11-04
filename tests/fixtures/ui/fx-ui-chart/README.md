Synthetic Overview data for the chart of the selected Overview entry (UI-102). No web server: the files are
`OverviewDto` answers of `sidebar.overview` for unit tests and the E2E fake bridge. Each entry is consistent in
itself only (as the spec requires); the numbers follow the test fixture of UI-102.

- `overview.json`: Summary (2 500 encountered, 2 400 displayed, 2 000 internal, 400 external, 1 100 indexable,
  100 non-indexable), Internal (partition of 11 content types, sum 2 000), External (partition of 4), Response Codes
  (`All` 1 000, partition of the seven classes, plus `Redirection (Meta Refresh)` and `Redirect Chain` outside the
  partition), Page Titles (no partition, one filter without matches), Canonicals (mixed: counted filters and filters
  waiting for crawl analysis) and Sitemaps (every category waiting for crawl analysis).
- `overview-unassigned.json`: as above, Response Codes `All` = 1 010 (residual category of 10).
- `overview-overflow.json`: as above, Response Codes `All` = 990 (category sum larger than the base, error case).

Percentages (`pct`) are computed like the core: tenths of a percent of the tab's `All`, halves rounded up; summary
rows relative to `Total URLs Encountered`.

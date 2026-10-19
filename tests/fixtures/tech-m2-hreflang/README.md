# M2-D07 fixture contracts

`relations.sql` models `hreflang/`, `out-hreflang` and `outside-head` from the spec
under the package directory required by K2-24. It is loaded into a fresh schema-v3
crawl file in analyze and export tests. URL IDs are deterministic and deliberately
cover overlapping defects: target 6 is both noindex and canonicalised; target 7
is canonicalised and returns an inconsistent code. Page 2 confirms via a sitemap.
Targets 8 and 9 cannot have their return annotations checked. Page 16 supplies
HTTP annotations and can be checked. Page 12 repeats a code across two sources.

`outside.html` exercises a head, noscript in head, an early head-ending img, and
a body annotation. The parse test runs regardless of validation; the full crawl
coverage test waits for D10 to implement the validation marker.

`detail.csv` is the expected single-selection detail export for URL 1. It uses
the confirmation precedence from TECH-089 with K2-46's Not Applicable status.
The SQL also has separate HTML/HTTP and sitemap truncation counters (2 + 3).

Der Paketbericht einschließlich Review-Fixes, K2-63-Erwartungsänderungen und
Prüfergebnissen steht in [docs/plan/M2-reports/d07.md](../../../docs/plan/M2-reports/d07.md).

Der Kern liefert den Kürzungshinweis als Schlüssel/Zahl
`("Hreflang Truncated", "5")`; U11/K06 übernehmen die Lokalisierung.
Die CSV enthält nur Annotationen und bleibt sprachunabhängig; die Warnung ist
separate Export-Metadaten (`Hreflang Truncated: 5`).

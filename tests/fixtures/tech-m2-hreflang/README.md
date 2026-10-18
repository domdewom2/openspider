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

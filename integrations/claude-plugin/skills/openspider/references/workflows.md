# Audit workflows

## Technical audit or indexability

1. Discover the requested saved snapshot with `list_crawls`. For a new target use
   `start_crawl` only when the crawl class is available and the requested scope is clear;
   keep its returned ID and poll `crawl_status`. A read-only server can audit saved crawls.
2. Use `crawl_overview` for coverage, state and analysis gaps, then `list_issues` in the
   user's language. Iterate all pages if the request is exhaustive.
3. For each relevant issue, pass its returned `query.tab` and `query.filter` to
   `query_urls`. Inspect examples with `url_details`: directives, canonicals, status and
   inlinks explain why a page is excluded or inaccessible.
4. Return priority, count, concrete URL evidence, explanation, proposed fix and retest.
   Distinguish intended exclusions from defects; ask about intent if it changes the advice.

## Redirects and internal links

Use the relevant issue query, or `query_urls` on `Response Codes` with the live filter
names. Inspect the redirect target in `url_details` and follow targets to establish each
chain or loop. Bound traversal and keep a visited set; an uncrawled destination is unknown.
Use the URL's `inlinks` to locate source pages that should link directly to the final target.
Count chains by starting URL and distinguish chains, loops and unavailable targets.

For internal-link questions use `url_details` for inlinks/outlinks, their totals and
cursors. A page with no crawled inlinks is not proven orphaned from the entire web.
For hreflang use issue queries and reciprocal/canonical details; distinguish missing
return links from an extraction or scope gap.

## Two-crawl comparison

Resolve previous and current snapshots with `list_crawls`, confirming their order from
the request and dates. Read both with `crawl_overview` and all relevant `list_issues`
pages, then `query_urls` for the same tab/filter and columns on both sides. Match by
URL, not local numeric URL IDs. A host/path migration requires the user's mapping;
do not silently match by basename. Report previous/current counts, delta, added/resolved
examples and configuration/coverage differences. Equal counts can conceal different URLs.

The current register offers snapshot reads rather than a dedicated comparison operation.
State that these are observed differences between snapshots; content changes require
stored evidence, and incomplete crawls do not establish a regression or resolution.

## Requested export

If the `write` class and `export` tool are exposed, read its live schema and preserve
the requested format, view/report and destination. Report the path returned by the tool.
On a destination or permission denial, explain it; do not widen allowed directories or
overwrite files to force the export. Without export access, provide an in-chat summary.

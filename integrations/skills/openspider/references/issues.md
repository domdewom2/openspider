# Interpreting issues

`list_issues` is the source for issue type, priority, thresholds, `what`, `why`, `fix`,
affected count and query. Use those values instead of maintaining a second issue catalog.
An Issue, Warning or Opportunity is a diagnostic classification; business impact depends
on the page's purpose. A high priority does not guarantee traffic loss.

| Evidence | Interpretation and next check |
| --- | --- |
| 4xx/5xx response | Confirm intended removal versus failure; inspect inlinks and response headers before recommending a redirect or server fix. |
| Redirect chain or loop | Trace each hop and destination; recommend direct internal links to the intended final URL, and resolve loops. |
| noindex, robots block, canonical elsewhere | Determine the intended indexable URL. Robots blocking and noindex differ; a blocked page's directives may not have been fetched. |
| Duplicate or missing title/description/headings | Inspect representative templates and canonical/indexability state. Similar text alone does not prove duplicate content. |
| Canonical conflict | Compare HTTP status, robots directives, HTML/header canonical and destination; do not assume a canonical is obeyed by a search engine. |
| hreflang finding | Check language/region codes, self/return references and indexable canonical targets within crawl scope. |
| Low inlink count | Inspect link sources and crawl coverage; orphan claims need additional URL sources such as sitemaps or analytics. |

Never treat a filter that is `not_evaluated` as empty. Record missing analysis, extraction,
rendering, authentication and interrupted crawl state alongside each affected conclusion.
Use server totals for site-level counts and label sampled examples. Re-run the same view
after a fix, with comparable configuration, to verify the result.

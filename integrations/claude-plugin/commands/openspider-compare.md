---
description: Compare issue and URL evidence between two saved OpenSpider crawls.
argument-hint: "[previous-crawl-id] [current-crawl-id]"
---

Read `${CLAUDE_PLUGIN_ROOT}/skills/openspider/SKILL.md` and follow its
references/workflows.md two-crawl workflow.
User selection (previous, then current): $ARGUMENTS

Discover OpenSpider MCP tools. If unavailable show `${CLAUDE_PLUGIN_ROOT}/README.md` installation
steps. Resolve both snapshots with `list_crawls`, clarifying order or mapping when
ambiguous. Read `crawl_overview` and paginated `list_issues` for both; compare like-for-like
`query_urls` results by URL, inspecting examples with `url_details`. Return previous/current
counts, delta, added/resolved examples and coverage/configuration caveats. The current
register has no dedicated comparison operation: label this as a snapshot comparison and
do not claim native change detection. Do not start new crawls or export without a request.

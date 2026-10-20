---
description: Explain and prioritize issues in a saved OpenSpider crawl.
argument-hint: "[crawl-id] [de|en]"
---

Read `${CLAUDE_PLUGIN_ROOT}/skills/openspider/SKILL.md` and follow its
references/issues.md interpretation guidance.
User selection: $ARGUMENTS

Discover OpenSpider MCP tools; if unavailable show the CLI installation/PATH instructions
from `${CLAUDE_PLUGIN_ROOT}/README.md`. Use `list_crawls` to identify the requested crawl. If the
selection is ambiguous, present the candidates. Use `crawl_overview`, then `list_issues`
in the requested language; paginate using `next_cursor`. Resolve relevant returned query
references with `query_urls` and inspect evidence with `url_details`. Return priority,
affected count, what/why/fix, example URLs and verification. Report incomplete analysis
and `not_evaluated` separately. This command only reads saved crawl evidence.

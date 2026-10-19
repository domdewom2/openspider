# Custom Search fixtures (M2-D14)

`regions.html` exercises decoded source, HTML5 head/body, visible text,
anchor exclusion and configured content areas. `xpath.html` deliberately omits
closing tags and contains void elements and encoded attribute values: XPath
runs on a bridge from the repaired HTML5 DOM, not an XML parse of raw HTML.
`regex.txt` covers flags, boundaries, lookbehind and bounded backtracking.
`bulk-keywords.txt` has 100 distinct keywords, three empty records and two
repeated records. `filters.json` is the profile shared by crawl and export tests.

`scenario.toml` serves the root with three stock matches, a 404 with one match,
a page with none, a 301 with a body, a bodyless 301, an HTML body without a
Content-Type, an external HTML URL and JS/image resources. In the default
configuration, four URLs are searched; Stock matches two and Missing stock
matches two. With Assume HTML enabled the response without Content-Type is
also searched. URL and Inlinks exports use these same persisted facts.

## Spec clarifications

- INTEG-028's `(a+)+$` example is handled by fancy-regex's linear matcher and
  immediately returns zero instead of a timeout. A backreference variant,
  `(?:(a+)\1)+$`, exercises the actual VM step limit and returns `Regex timeout`.
- Empty regex matches count, including pure lookaheads; `find_iter` advances so
  `x*` terminates. The configured wall-time budget starts once per page and is
  shared by all regex filters; text filters still run after it expires.
- Regex evaluation also bounds a single `find` by a deterministic backtrack cap:
  at least the library default of 1,000,000, or 8 times the largest region's UTF-8
  byte length, whichever is larger. The fixture-derived 200 KB HTML test verifies
  successful and unsuccessful lookbehind scans without a false timeout.
- Validation codes are `invalid_custom_search.limit`, `.duplicate_name`,
  `.empty_expression`, `.invalid_regex`, `.invalid_xpath`. U13B should use these
  codes rather than interpreting localized messages.
- UI dialogs and CSV/TXT file loading are U13B; the parser's Bulk Add function
  accepts already decoded single-column records and reports skipped/overflow counts.
  Rendered input (INTEG-030) is wired by E05.

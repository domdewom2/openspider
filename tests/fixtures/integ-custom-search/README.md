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
- INTEG-028 asks both for a positive count for a pure lookahead and for zero-length
  matches not to be counted. This implementation follows the explicit empty-match
  rule: `(?s)(?=.*rot)(?=.*blau)` counts zero; adding `.+` counts one.
- Regex evaluation uses the configured wall-time budget plus a deterministic
  50,000-backtrack cap per find. Hitting either limit produces `Regex timeout`;
  the step cap can stop a pattern before its wall-time budget expires.
- UI dialogs and CSV/TXT file loading are U13B; the parser's Bulk Add function
  accepts already decoded single-column records and reports skipped/overflow counts.
  Rendered input (INTEG-030) is wired by E05.

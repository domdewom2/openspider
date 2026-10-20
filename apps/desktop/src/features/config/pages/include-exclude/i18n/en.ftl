# Include / Exclude page (M2-U13)
config-ie-desc = Regular expressions, one per line. A rule has to match the whole URL unless partial matching is enabled. Exclude is checked before Include.
config-sec-include = Include
config-sec-exclude = Exclude
config-sec-match = Matching
config-hint-include = Only URLs matching at least one line are crawled. Empty = no restriction. Blank lines do not count toward line numbers.
config-hint-exclude = URLs matching any line are not crawled. Blank lines do not count toward line numbers.
config-hint-partial = Also match parts of the URL. Without it, use .* to match the rest.
config-f-scope-include = Include rules
config-f-scope-exclude = Exclude rules
config-f-scope-regex-partial-match = Rules also match parts of the URL
config-ie-tester = Regex tester
config-ie-verdict-included = Included
config-ie-verdict-excluded = Excluded
config-ie-verdict-notIncluded = Not included
config-ie-list-include = Include
config-ie-list-exclude = Exclude
config-ie-line = { $list } line { $line }
config-ie-encoded = Tested URL (encoded like the crawler does)
config-ie-partial = { $line } only matches part of the URL – add .* ?
config-ie-invalid = { $line } is not a valid regular expression and was skipped.

# M2-D01 fixtures

Independent, spec-derived HTML and SQLite inputs; no reference-product resources.

The HTML scenario is crawled through `os-testsite` in `tab_meta_m2.rs`:

| Page | Title width (20 px) | Description width (14 px) | Purpose |
| --- | ---: | ---: | --- |
| `wide.html` | 1080 | 1800 | Wide characters cross the pixel limit despite a short character count. |
| `narrow.html` | 350 | 480 | Narrow characters cross the character limit but stay below the pixel maximum. |
| `max-boundary.html` | 561 | 985 | Exact maximum is excluded from Over Pixels. |
| `min-boundary.html` | 200 | 400 | Exact minimum is excluded from Below Pixels. |

`facts.html` tests HTML5 head membership, nested image alt text and heading order.
`headings.json` covers the positive and negative heading cases, including empty
headings and ignored template contents. `store.sql` supplies explicit stored
boundary values and partial validation coverage, independent of the M2-D10 stub.

The separate `ui/fx-ui-serp/profiles.json` uses the calibrated UI-038 metrics:
70 × W at 20 px = 1260 px, 75 × W at 14 px = 900 px (K2-49).

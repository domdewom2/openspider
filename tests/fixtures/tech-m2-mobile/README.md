# Mobile fixtures (M2-D23)

Own HTML and synthesized Lighthouse reports for TECH-230–237 / INTEG-140, 146, 147.
No reference-product material and no external network access.

`lighthouse-cases.json` maps each page to an LHR; `expected.json` records the five integer scores.
These are test inputs for score mapping, not claims of measurements from an installed Lighthouse version.
Plugin fallback and alternate detection are additionally tested through the real crawler.
`js-flash-rendered.html` represents the DOM delivered by a renderer for `js-flash.html`.
`partial-target.html` is a report-only partial-score case, not an HTML fixture.

K2-09: use fixture reports for the local runner; bundling Lighthouse is M3.
TECH-230 leads tab availability; its explicit empty-column/overview-hint behavior takes precedence over
INTEG-133's conflicting instruction to remove the Mobile columns and filters.

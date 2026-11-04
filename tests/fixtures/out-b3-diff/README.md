# B3-DIFF – Crawl-Vergleich für Deployments (OUT-151, OUT-154, TECH-309, TECH-310)

Alle Dateien sind lokal für diese Tests geschrieben; es werden keine Dateien des Referenzprodukts verwendet.

Zwei Stände derselben Site als eigene Szenarien, beide gecrawlt unter `http://www.site.test/` (der Test löst `*.test`
über `network.resolve` auf die Testsite auf); die URLs beider Crawls sind identisch, ein URL-Mapping ist nicht nötig.
Weiterleitungen stehen als Routen in `scenario.toml` (301 ohne Body und ohne Content-Type, Inhaltsklasse `Unknown`);
fehlende Dateien beantwortet die Testsite mit 404 und `text/plain`.

## `v1` (Previous) → `v2` (Current)

| Seite | v1 | v2 | erwartet in „Crawl Changes“ (OUT-154) |
|---|---|---|---|
| `/keep.html` | 200 | 200, unverändert | – |
| `/gone.html` | 200, verlinkt | gelöscht, nicht mehr verlinkt | `Page Gone` (Missing in „Internal:HTML“) |
| `/broken-now.html` | 200 | 404, weiter verlinkt | `Page Gone`, `New Error`, `Indexability Changed` |
| `/broken.html` | – | 404, neu verlinkt | `New Error` |
| `/new.html` | – | 200 | – (New in „Internal:HTML“) |
| `/fixed.html` | 404 | 200 | `Indexability Changed`; Removed in „Response Codes:Client Error (4xx)“ |
| `/moved.html` | 200 | 301 → `/keep.html` | `New Redirect`, `Indexability Changed` |
| `/r1` | 301 → `/end1.html` (1 Sprung) | 301 → `/r1b` → `/end1.html` (2) | `Redirect Target Changed`, `New Redirect Chain` |
| `/r1b` | – | 301 → `/end1.html` | `New Redirect` |
| `/c1` | 301 → `/c2` → `/end2.html` (2) | 301 → `/c2` → `/c3` → `/end2.html` (3) | `Longer Redirect Chain` |
| `/c2` | 301 → `/end2.html` (1) | 301 → `/c3` → `/end2.html` (2) | `Redirect Target Changed`, `New Redirect Chain` |
| `/c3` | – | 301 → `/end2.html` | `New Redirect` |
| `/rt` | 301 → `/end1.html` | 301 → `/end2.html` | `Redirect Target Changed` |
| `/canon.html` | Canonical `/canon-a.html` | Canonical `/canon-b.html` | `Canonical Changed` (beide `Canonicalised`) |
| `/idx.html` | indexierbar | `noindex` | `Indexability Changed` |

Change Detection (TECH-300 ff.): „Canonical“ listet `/canon.html`; „Redirect Target“ listet `/moved.html`, `/r1`,
`/c2`, `/rt` (Population: in beiden Crawls HTML oder Weiterleitung, TECH-301); `/c1` behält das erste Ziel `/c2`.

Golden-Datei des Reports: `tests/golden/out-b3-diff/crawl_changes.csv`.

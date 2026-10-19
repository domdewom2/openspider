# M2-C01 – Vergleichsmodus (OUT-140 ff.)

Alle Dateien sind lokal für diese Tests geschrieben; es werden keine Dateien des Referenzprodukts verwendet.

Statt eines Umschalters `?version=1|2` liegen die beiden Stände der Site als eigene Szenarien vor. Beide werden unter
`http://www.site.test/` gecrawlt (der Test löst `*.test` über `network.resolve` auf die Testsite auf), die URLs beider
Crawls sind also identisch, ohne dass ein URL-Mapping nötig ist.

## `v1` (Previous) → `v2` (Current)

| Änderung (OUT-140, OUT-142, OUT-149) | Seiten |
|---|---|
| 3 Seiten neu | `/new1.html`, `/new2.html`, `/new3.html` |
| 1 Seite neu mit `noindex` | `/new-noindex.html` |
| 2 Seiten gelöscht (nicht mehr verlinkt, nicht gecrawlt) | `/gone1.html`, `/gone2.html` |
| 1 Title geändert, vorher „Duplicate“ | `/dup1.html` (Partner `/dup2.html` verliert den Duplikat-Status ebenfalls) |
| 1 Link entfernt → Tiefe ändert sich | Startseite verlinkt `/deep.html` nicht mehr; in v2 nur über `/a.html` (Tiefe 1 → 2) |
| 1 404 behoben | `/missing.html` (v1 ohne Datei, v2 vorhanden) |
| Linkwechsel `/b` → `/a` | `/start.html` verlinkt in v1 `/b.html`, in v2 `/a.html` |
| geänderter Ankertext | Link der Startseite auf `/c.html`: „Alt“ → „Neu“ |
| Wechsel nur des Linktyps | `/tc.html`: Hyperlink (v1) → Canonical (v2) auf `/tc-target.html` |

## `mapping` (OUT-146, CRAWL-034, TECH-306, OUT-151)

Zwei Hosts mit gleicher Struktur: `staging.site.test` mit Trailing Slashes (Verzeichnisse unter
`hosts/staging.site.test/`), `www.site.test` ohne (Routen in `scenario.toml`). Auf Staging gibt es zusätzlich `/dup` und
`/dup/`: beide bilden mit Host- und Slash-Regel auf `http://www.site.test/dup` ab (Kollision nach OUT-146).

Weitere Testdaten: die älteren Crawl-Dateien `tests/fixtures/crawl-files/schema-v1.ospider` und `schema-v2.ospider`
(nur gelesen und vor der Benutzung kopiert) für die Migration als Kopie (OUT-140).

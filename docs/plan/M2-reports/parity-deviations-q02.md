# Paritätsabweichungen M2-Filter – Arbeitsliste für Q02 (K2-66)

Stand: 20.10.2026, Integrationsrunde 5 (`m2/int-5`, Parity-Lauf 39382118963). Die Aktivierung der M2-Filter im
Overview-Mapping und der Spalten `Link Score`/`Inlinks` im Mapping `links_all` (aus M2-P01) liefert die Abweichungen
unten. Nach K2-66 werden die M2-Filter wie in M2.md 6.1 vorgesehen erst in Q02 scharf geschaltet; bis dahin zählt
`tests/parity/mappings/crawl_overview.toml` M2-Zeilen nur (Stand seit 96f5946), und `links_all.toml` ignoriert beide Spalten.

Wieder einschalten in Q02: in `crawl_overview.toml` unter `[blocks]` `filters = ["docs/plan/M2-filters.toml"]` ergänzen,
in `links_all.toml` die `ignore`-Einträge von `Link Score` und `Inlinks` durch `"Link Score" = "Link Score"` und
`"Inlinks" = "Inlinks"` ersetzen und den Test `m2_filters_are_prepared_but_not_compared_before_q02`
(`crates/os-parity/tests/real_repo_files.rs`) auf „scharf“ umstellen. Schwellen: Overview 100 %, `links_all` 97 %.

Betroffene Sites des Laufs: `out-basic-site`, `parity-onpage`, `parity-response`, `parity-synthetic`, `parity-technical`
(die fünf M2-Sites sind noch `pending`). Zahlen: Overview 72,9–76,9 % Zellübereinstimmung, `links_all` 90,3–92,0 %.

## 1 Overview: 30 Zeilen der Referenz fehlen in OpenSpider (auf allen fünf Sites)

Zuständig sind Pakete, die in Runde 5 nicht gemergt wurden (R01 bzw. R02 für Rendering, U/D-Pakete für die Tabs).

| Abschnitt | Fehlende Zeilen | Paket laut `M2-filters.toml` |
| --- | --- | --- |
| Response Codes | Blocked Resource, Redirection (JavaScript) jeweils als Gesamt-, Internal- und External-Zeile (6) | M2-R02 |
| JavaScript | All, Pages With Blocked Resources, Contains JavaScript Links, Contains JavaScript Content, Noindex/Nofollow Only In Original HTML, Canonical Only In Rendered HTML, Canonical Mismatch, Page Title Only In Rendered HTML / Updated By JavaScript, Meta Description Only In Rendered HTML / Updated By JavaScript, H1 Only In Rendered HTML / Updated By JavaScript, Pages With JavaScript Errors (15) | M2-R02 |
| Mobile | All, Viewport Not Set, Target Size, Content Not Sized Correctly, Illegible Font Size, Mobile Alternate Link (6) | M2-D23 |
| Accessibility | All (1) | M2-D21 |
| Custom JavaScript | All (1) | M2-D22 |
| Link Metrics | All (1) | M2-I05 |

Die JavaScript- und Response-Codes-Filter verlangen `rendering.mode=javascript`; die übrigen Pakete sind nicht gemergt.
Entscheidung für Q02: Zeilen liefern oder als `REF-FREE-`/`DEV-`-Eintrag in `tests/parity/deviations.toml` begründen.

## 2 Overview: abweichende Zähler vorhandener M2-Filter (61 Zellen je Site, Beispiele von `out-basic-site`)

| Zeile | Referenz | OpenSpider | Zuständig / Hinweis |
| --- | --- | --- | --- |
| Internal \| Other | 1 | 0 | M2-D02 (ONPAGE-023): Klassifizierung, Antworten ohne Content-Type (siehe DEV-014 für den Filter Other) |
| Security \| Missing Content-Security-Policy Header | 16 | 18 | M2-D11 (ONPAGE-039) |
| Security \| Missing X-Content-Type-Options Header | 16 | 18 | M2-D11 |
| Security \| Missing X-Frame-Options Header | 16 | 18 | M2-D11 |
| Security \| Missing Secure Referrer-Policy Header | 16 | 18 | M2-D11 |
| Security \| Bad Content Type | 0 | 1 | M2-D11 (ONPAGE-043) |
| URL \| Broken Bookmark | 0 | leer | M2-E04 (ONPAGE-079): Filter liefert keinen Zähler |
| Content \| Near Duplicates | 0 | leer | M2-D02 (ONPAGE-144): Zähler entsteht erst nach der Crawl-Analyse (`post_crawl:NearDuplicates`) |
| Content \| Spelling Errors | 0 | leer | M2-D19 (ONPAGE-155) |
| Content \| Grammar Errors | 0 | leer | M2-D19 (ONPAGE-156) |

Security: Die Differenz 18 gegenüber 16 deutet auf zwei Antworten, die OpenSpider mitzählt und die Referenz nicht
(vermutlich Weiterleitungen oder Antworten ohne Body), und fällt mit dem einen Treffer bei „Bad Content Type“ zusammen
(Referenz 0). Vor dem Scharfschalten klären, welche Population die Spec (ONPAGE-039 ff.) meint.

Leere Zähler: mögliche Ursache ist eine nicht ausgeführte Crawl-Analyse bzw. fehlende Zählung im Overview; in Q02 prüfen. Weitere abweichende Zeilen stehen in `report.json` der Läufe (je Site bis zu 61 Zellen, 10 Beispiele im Bericht).

## 3 `links_all`: Spalten `Link Score` und `Inlinks` (`out-basic-site`, 12 Zeilen)

| Spalte | Fehler | Beispiele (Referenz → OpenSpider) |
| --- | --- | --- |
| Link Score | 10 von 12 | `/contact.html` 90 → 96, `/services.html` 13 → 32, `/imprint.html` 29 → 55, `/about.html` 29 → 55, `/faq.html` 13 → 32, `/blog/` 10 → 27, `/products.html` 90 → 96, `/orphan.html` 3 → 9, `/privat/intern.html` 25 → 49, `/products/widget.html` 17 → 40 |
| Inlinks | 2 von 12 | `/contact.html` 14 → 13, `/products.html` 14 → 13 |

Zuständig: M2-D08 (Link Score, TECH-145; Verteilung und Skalierung der Werte weichen systematisch ab, nicht nur
einzelne Seiten) und TECH-130 (Inlinks: Selbstlinks zählen in OpenSpider nicht, K-07, vgl. DEV-010/DEV-011; die
Differenz 1 bei Seiten mit Selbstlink ist damit möglicherweise nur ein fehlender `DEV-`-Eintrag für die Spalte).
`parity-synthetic` (91,2 %) und `parity-technical` (92,0 %) zeigen dasselbe Muster.

## 4 Reihenfolge für Q02

1. Rendering-abhängige Zeilen (Abschnitt 1) entscheiden: Zähler 0 liefern oder begründet ausnehmen.
2. Security-Population und Bad Content Type klären (M2-D11), Internal Other (M2-D02).
3. Leere Zähler (Broken Bookmark, Near Duplicates, Spelling, Grammar) füllen.
4. Link Score gegen die Referenzwerte kalibrieren (M2-D08), Inlinks-Selbstlinks als `DEV-` begründen oder angleichen.
5. Mapping scharf schalten (siehe oben) und `os-parity run --all` über alle zehn Sites.

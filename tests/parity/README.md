# tests/parity – Paritätsabgleich: Mappings, Profil, Fixture-Generator (M1-P10)

| Pfad | Inhalt |
|---|---|
| `mappings/<export>.toml` | Spaltenmapping Referenz → OpenSpider je Export (Format: `docs/plan/M1.md` 5.2) |
| `deviations.toml` | dokumentierte Paritätsabweichungen (`DEV-nnn`) und Referenzeinschränkungen der Free-Version (`REF-FREE-nn`, K-53) |
| `evidence.toml` | Zuordnung Spec-ID ↔ Export für den Status `verified` (`os-parity coverage --parity-report … --evidence …`) |
| `thresholds.toml` | Schwellen des Abnahmekriteriums A3 für `os-parity run` |
| `profiles/parity.osconfig` | OpenSpider-Konfiguration für Paritätsläufe; entspricht der tatsächlichen Referenzkonfiguration (Referenzprodukt (Free-Version), K-53) |
| `coverage/<site>.toml` | Absichtserklärung je Tab/Filter: welche Adresse Treffer sein soll, welche nicht |
| `gen/` | Generator der Fixtures: `python3 -B tests/parity/gen/generate.py` (deterministisch, nur Python-Standardbibliothek) |
| `fixtures_check.rs` | Prüftest der Fixtures (siehe unten) |

Die Fixtures liegen unter `tests/fixtures/{out-basic-site,parity-*}/` und werden aus `gen/` erzeugt – Änderungen am
Generator vornehmen und neu erzeugen, nicht die erzeugten Dateien bearbeiten. Nach jeder Änderung an einer Site müssen die
Referenz-Exporte dieser Site neu erzeugt werden (Anleitung: `tests/golden/parity/README.md`). `out-basic-site/exclude.osconfig`
und `columns.toml` sind von Hand gepflegt.

## fixtures_check.rs

Die Datei ist ein Integrationstest des Pakets `os-testsite` und wird dort per Eintrag in `crates/os-testsite/Cargo.toml` eingebunden
(der Eintrag gehört dem Orchestrator, E4):

```toml
[[test]]
name = "parity_fixtures"
path = "../../tests/parity/fixtures_check.rs"
```

Er braucht nur `os_testsite`, `tokio`, `toml` und `flate2` (alle bereits Abhängigkeiten von `os-testsite`) und prüft ohne Netz:

- BFS-Rohcrawl jeder Site über den os-testsite-Client: höchstens 500 URLs, Abgleich mit dem Request-Log;
- Selbstständigkeit: nur `127.0.0.1` (intern) und `localhost` (extern), keine `.test`-Hosts, kein Inline-JavaScript,
  keine Platzhalter (Ausnahme: `{{BASE_URL}}` in fünf Dateien von `out-basic-site`: index.html, about.html, products.html, robots.txt, sitemap.xml, siehe Kommentar in `gen/site_basic.py`),
  keine Verzögerung über 1 s;
- die Absichtserklärung (`coverage/*.toml`) gegen die ausgelieferten Seiten, mit einem kleinen unabhängigen Orakel nach den Spec-Regeln;
- jeder Tab der Gruppen Internal … Sitemaps hat Treffer und Nicht-Treffer; jeder Filter ist abgedeckt oder mit Grund ausgenommen;
- Manifeste, Mappings, Profil, Abweichungsliste und README sind vorhanden und konsistent.

## Stand M1-P29 (Referenzexporte (Free-Version))

Die Referenzexporte liegen unter `tests/golden/parity/<site>/ref/` (unverändert, nur dort; A8). Der Lauf:

```sh
cargo build -p os-cli -p os-parity
target/debug/os-parity run --all --openspider target/debug/openspider --out target/parity
# Fehlersuche: mehr Beispiele je Spalte im Bericht
OS_PARITY_MAX_EXAMPLES=500 target/debug/os-parity run --site parity-onpage --openspider target/debug/openspider
```

Anpassungen an das reale Referenzformat (Mapping-Optionen, siehe `crates/os-parity/src/mapping.rs`):

- `format = "blocks"` + `[blocks]`: das Crawl Overview der Referenz ist ein Blockformat (Kopfzeilen, dann je Abschnitt eine
  Kopfzeile) mit Dezimalkomma; der Runner überführt es in `(Section, Item, URLs, % of Total)`. Nur Zeilen zu M1-Filtern
  (`docs/plan/M1-filters.toml`) müssen in OpenSpider vorkommen.
- Spaltenoption `transform = [...]`: `ignore_case`, `zero_as_empty`, `xpath_tags`, `hex_trim_zeros`, `decimal_comma`
  (nur reine Darstellungsunterschiede; inhaltliche Abweichungen gehören nach `deviations.toml`).
- `dedupe_reference = true`: Referenzzeilen mit gleichem Schlüssel zählen als eine (Redirect-Reports: eine Zeile je Quelle).
- Nummerierte Spalten (`Title 2`, `H1-2`, `HTML hreflang 3 URL` …) schreibt die Referenz nur bei vorhandenen Daten; fehlen sie,
  gilt die Referenz als leer, sobald OpenSpider die Spalte liefert.
- Manifest `skip = "…"`: Referenz liegt vor, der Export gehört aber nicht zum M1-Umfang (übersprungen mit Grund).
- Abweichungen: `delta`, `key_prefixes`, mehrere Spalten (`a|b`) und Exporte (`export = "x|y"`).

## N06: beide Referenzgenerationen und strenge Filterbelege

Nummerierte Spalten, die in der Referenz fehlen, erwarten in OpenSpider ausschließlich leer oder numerisch Null.
Fehlen sie auf beiden Seiten, entsteht keine Spaltenlücke. Eine vorhandene Referenzspalte bleibt Pflicht.
`optional_ref = true` an einer Spalte erlaubt deren Fehlen in einer Referenzgeneration; vorhandene Werte
werden unverändert verglichen. Diese Option ersetzt keine fehlenden Produktspalten.

Kettenmappings setzen `chain_end_target = true`: Status und Status Code im zusätzlichen Endzielblock
`n+1` werden gegen Final Status und Final Status Code geprüft. Die eigentlichen Sprünge bleiben im Vergleich.
Leere, nicht erzeugte Sprungspalten erzeugen dabei keine Lücke. Die Anzahl liefert je nach Report
Number of Redirects, Number of Redirects/Canonicals oder Number of Canonicals.

Reports und Bulk-Exporte werden je Gruppe angefordert. Schlägt eine Gruppe fehl, werden ihre Namen einzeln
wiederholt. Ein unbekannter Name betrifft damit nur seinen eigenen Export. Abweichende Dateistämme stehen ausdrücklich als `os_file` im Manifest, insbesondere `Links:… (Bulk)`
und Hreflang-Reports mit dem Suffix Report. Der eindeutige historische Bindestrich-Alias bleibt erhalten.
Crawl Path bleibt nach SQ-14 bis zur CLI-Unterstützung in N11 mit Begründung übersprungen.

`deviations.toml` unterstützt zusätzlich `file` (Referenzdateiname, mehrere mit `|`). Der Runner filtert diese
Ausnahmen vor dem Vergleich. TECH-141 darf beispielsweise nur die Zeilen des Localhost-Filterexports
entschuldigen. Belege in `evidence.toml` unterstützen die Selektoren `sites` und `files`; alle gewählten
Exporte müssen verglichen sein. Fehlende oder ungemappte Spalten und fehlende angeforderte Belegspalten
sperren verified, auch bei null Zellfehlern.

B3g nach SQ-01 wird direkt durch os-parity berechnet:

```sh
target/debug/os-parity coverage --parity-report target/parity/report.json \
  --write docs/plan/M2-acceptance/parity/verified.json
```

Ohne `--evidence` wertet dieser Modus die kanonischen M1-/M2-Filterkataloge gegen das vollständige Laufjournal
aus: je Filter-ID Tab-All auf allen referenzfähigen Sites und zusätzlich dessen eigener Filterexport, soweit
vorhanden. Jeder Beleg verlangt identische URL-Mengen, null Zellfehler und vollständige Spalten; DEV ist erlaubt,
REF-FREE sperrt. Historische M1-PDF-Ausnahmen DEV-016/022/024 geben nach Q02-P06 ebenfalls keine M2-Gutschrift. Nicht verglichene Present-Exporte sperren ebenfalls. Change Detection ist ausgeschlossen;
der Nenner beträgt 155 und hängt nicht von den grünen Exporten ab. Gemeinsam verwendete Spec-IDs verlangen
alle zugehörigen Filterbelege. SQ-10 sperrt Links/Internal bis zur Link-Score-Kalibrierung in M3.

Die Auswertung beschreibt Paritätsbelege. Der Coverage-Modus mit `--evidence` und Testergebnissen prüft zusätzlich
den Implementierungsstatus aus den Spec-getaggten Tests. Er bleibt für die M1-Statusdatei verfügbar.

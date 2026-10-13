# tests/parity – Paritätsabgleich: Mappings, Profil, Fixture-Generator (M1-P10)

| Pfad | Inhalt |
|---|---|
| `mappings/<export>.toml` | Spaltenmapping Referenz → OpenSpider je Export (Format: `docs/plan/M1.md` 5.2) |
| `deviations.toml` | dokumentierte Paritätsabweichungen (leer mit Schema-Kommentar) |
| `thresholds.toml` | Schwellen des Abnahmekriteriums A3 für `os-parity run` |
| `profiles/parity.osconfig` | OpenSpider-Konfiguration für Paritätsläufe |
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

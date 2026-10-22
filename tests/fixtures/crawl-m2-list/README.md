# M2-E06 Fixtures

Eigene lokale Testdaten für CRAWL-029/030/031, TECH-087 und OUT-085/098.
Der Pflichtbericht mit Entscheidungen, Besitznachweisen und Prüfstatus steht in
[`docs/plan/M2-reports/e06.md`](../../../docs/plan/M2-reports/e06.md).

- `scenario.toml`: Redirect-, Canonical- und Hreflang-Seiten sowie eine
  Canonical-Kette über drei Unterordner.
- `dupes.txt`: zehn Uploadzeilen mit Duplikaten und ausgeschlossener Eingabe.
- `serp.csv`, `serp.xls`, `serp.xlsx`: fünf identische SERP-Entwürfe.
- `serp-de.txt`: deutsche Kopfzeilen; `serp-missing-header.csv`: Fehlerfall.
- `export_tests.rs`: produktive CSV-/XLSX-Exporte und zweifache SERP-Rundreise,
  eingebunden durch `crates/os-export/tests/list_m2_e06.rs`.

Reproduzieren:

```sh
cargo test -p os-engine --test list_m2 --test serp_m2 --test hreflang_crawl_m2
cargo test -p os-export --test list_m2_e06 --test pending_w1_d12
```

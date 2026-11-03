# M2-N33 Fixtures: defekte `.xls`-Dateien

Absichtlich defekte OLE2-/CFB-Container (Compound File Binary, Version 3, Sektorgröße 512) für den Listen- und
SERP-Import (CRAWL-027, CRAWL-031). Anlass: RUSTSEC-2026-0436 (calamine 0.36.1, Panic und unbegrenzte Allokation im
CFB-Leser, behoben in calamine 0.38.0). Bericht: [`docs/plan/M3-reports/n33.md`](../../../docs/plan/M3-reports/n33.md).

Alle Dateien sind von Hand nach der öffentlichen CFB-Struktur gebaut (Kopf 512 Byte, danach Sektoren); sie enthalten
keinen `Workbook`-Strom und damit keine Tabellendaten.

| Datei | Byte | Aufbau | calamine 0.36.1 | calamine 0.38.0 |
|---|---|---|---|---|
| `truncated-header.xls` | 300 | gültige Signatur, Kopf nach 300 Byte abgeschnitten | Fehler | Fehler |
| `truncated-directory.xls` | 1 224 | FAT in Sektor 0, Verzeichnis in Sektor 1, Datei endet nach 200 Byte des Verzeichnissektors (Verzeichnisstrom 200 Byte, kein Vielfaches von 128) | Panic (Slice-Index) | Fehler „Workbook-Strom fehlt“ |
| `difat-loop.xls` | 1 024 | FAT-Länge im Kopf `0xFFFFFFFF`, DIFAT-Kette beginnt in Sektor 0 und verweist im letzten Eintrag wieder auf Sektor 0 | Endlosschleife mit wachsender Liste | Fehler „Sektor hinter Dateiende“ |
| `no-fat.xls` | 1 024 | kein FAT-Sektor (alle DIFAT-Einträge frei), Verzeichnis in Sektor 0 | Panic (Index) | Panic (Index) – der Import fängt ihn ab |

Erwartung (Test `crates/os-engine/tests/list_corrupt_xls.rs`): Listen- und SERP-Import liefern für jede Datei einen
Fehler der Datei, ohne Panic, innerhalb einer festen Zeit und mit begrenztem Speicher.

```sh
cargo test -p os-engine --test list_corrupt_xls
```

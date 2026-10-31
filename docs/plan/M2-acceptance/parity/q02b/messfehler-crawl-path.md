# OUT-023: nachgewiesener Messfehler und freigegebener Fix

Der Paketeintrag M2-Q02b verlangt für os-parity-Code ausdrücklich „nur bei nachgewiesenem Messfehler (melden, dann Freigabe)“. Die Freigabe wurde im Orchestrator-Nachtrag vom 30.10. ausdrücklich erteilt; Umsetzung und Negativtests sind im K2-96-Nachlauf enthalten. Das ist keine fehlende Produktfunktion: N11 hat den CLI- und Oberflächenexport implementiert und getestet.

## Reproduktion und Beleg

- `tests/golden/parity/parity-m2-links/manifest.toml`: `report:Crawl Path Report`, Auswahl `/deep/8.html`, vorhandener Skip aus der Zeit vor N11.
- `crates/os-parity/src/run.rs`: `ExportRequest`/`CommandExporter::args` führen nur Tabs, Bulk-Exporte, Menüreports und Sitemap-Varianten. `--export-crawl-path` wird nie erzeugt.
- `crates/os-export/src/reports/crawl_path.rs::entries()` liefert absichtlich keine Menüreport-Einträge. OUT-023 ist ein Auswahl-Export.
- Native Probe [crawl-path-probe.json](crawl-path-probe.json): Referenz hat 17 Zeilen: acht `Hyperlink`-Verbindungen von Start bis `/deep/8.html` und neun leere Trenner. `Target Address`/`Step` fehlen.
- Native Wertprobe [inspect.json](inspect.json): Source-Tiefen 7 bis 0, Status 200/OK, Anchor `Next depth` bzw. am Start `/deep/1.html`. Diese Probe wird nicht als erfolgreicher Paritätsvergleich gewertet.
- Soll nach OUT-023: neun OS-Knoten, Step 0–8, Ziel zuerst, Start zuletzt. Die zwei N11-Fixture-Tests gehören zum vollständigen Workspace-Lauf; sie ersetzen keine Runner-Freischaltung.

## Konkrete Änderung für die Freigabe

1. In `run.rs` einen eigenen Auswahlauftrag für Crawl Path führen, getrennt von `reports`. URL-Auswahl aus dem Manifest erst nach Site-Start an die tatsächliche Basis binden; Argument `--export-crawl-path http://127.0.0.1:8908/deep/8.html`, Ausgabedatei `crawl_path_report.csv`. Fehlende/mehrdeutige Auswahl als Messfehler zurückweisen. Keine Behandlung als `--save-report`.
2. In `mapping.rs`/`csvio.rs` ein explizites Verbindungsformat für dieses Mapping ergänzen. Referenz: nur vollständig leere Trenner verwerfen, acht echte Verbindungen erhalten; keine beliebige unverständliche Zeile wegfiltern. OS: pro Target-Block benachbarte Knoten zu Verbindungen projizieren: `Source=Address[Step+1]`, `Destination=Address[Step]`, `Type=Link Type[Step]`, `Anchor=Anchor[Step]`, `Crawl Depth=Crawl Depth[Step+1]`, Statuscode aus dem verbundenen Knoten. Bei gleichen 200-Codes belegt diese Fixture die Statusrichtung nicht; diese muss durch einen Redirect-Fixture-Test festgelegt werden.
3. Identität der Verbindung über Source/Destination/Ordinal, zusätzlich Type/Anchor/Tiefe/Statuscode vergleichen. Die Anfangs-/Endknoten, Target-Auswahl und Steps vor der Projektion validieren, damit ein verlorener Knoten, umgekehrte Reihenfolge oder vermischte Target-Blöcke nicht grün werden kann. `Follow`/`Alt Text` gehören nicht zum OUT-023-Knotenvertrag und brauchen einen expliziten Mapping-Grund.
4. Manifest-Skip entfernen und `os_file = "crawl_path_report.csv"` setzen. Nicht benötigte `Target Address`-Selektion auf Referenzseite durch die eigene Auswahl-/Layoutlogik ersetzen.
5. Tests mit drei Knoten, einem Redirect, zwei Target-Blöcken und negativen Fällen (fehlende Kante, falscher Anchor, falsche Tiefe, falsche Zielauswahl). Reale CLI-Integration auf der unveränderten deep8-Fixture: neun Knoten → acht Verbindungen, identische URL-/Verbindungsmenge, ≥97 %.
6. Danach vollständiges `os-parity run --all` und `coverage`; bestehende 129/155 dürfen nicht sinken. Referenzdatei, Export-Schwellen und Produktcode bleiben unverändert.

Betroffene zusätzliche Dateien: `crates/os-parity/src/{run,mapping,csvio}.rs` sowie passende Runner-/Layouttests. Die Freigabe soll genau diesen Messfix umfassen; weitere Produktkorrekturen sind nicht Bestandteil dieses Vorschlags.

## Umsetzung nach Freigabe

Runner, Mapping, CSV-Projektion und Manifest sind umgesetzt. Reale deep8-Probe: 9 Knoten → 8 Verbindungen, 100 % Zellübereinstimmung, identische Verbindungsmenge. Zielstatus wird anhand der Redirect-Probe von der Quelltiefe unterschieden. Separate Zielblöcke bleiben erhalten; vermischte oder unvollständige Blöcke schlagen fehl. Der vollständige Endlauf und die Abschlussprüfungen stehen in bericht-q02b.md.

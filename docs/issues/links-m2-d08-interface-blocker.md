# M2-D08: Schnittstellenbedarf für TECH-146

Stand: 2026-10-17, Ausgangscommit `324c76a`, Branch `m2/d08`.
Der Worktree enthielt beim Start keine uncommitteten Änderungen.

## Befund und Abbruch nach C10

TECH-146 verlangt die Anchors-Aggregation für Inlinks und Outlinks sowie
umschaltbare Ausschlüsse für Self-Links und Nofollow. Der vorhandene
Store-Erweiterungspunkt in `crates/os-store/src/queries/anchors.rs` erhält
lediglich Datei, URL-IDs, Offset und Limit. Die Richtung und die Optionen
erreichen ihn nicht. Damit können zwei Anfragen mit gleichen URL-IDs und
unterschiedlichen Optionen keine unterschiedlichen Ergebnisse erzeugen.

Die Ursache liegt in der nach M2.md 3.14.13 eingefrorenen, nicht zu D08
gehörenden Datei `crates/os-analyze/src/details/mod.rs`, Zeilen 46–56:

```rust
fn $name(file: &CrawlFile, _: &CrawlConfig, req: &DetailReq<'_>) -> Result<DetailRows, ViewError> {
    let data = os_store::queries::$module::detail_rows(file, req.url_ids, req.offset, req.limit)
```

`DetailReq` enthält `params` und `sort`, aber der Adapter verwirft beide.
`store_source! { anchors => anchors, ... }` bindet genau diesen Adapter ein.
Die öffentlichen F05b-Felder `LinkFilter.exclude_self`, `exclude_nofollow`,
`anchor` und `mode` ändern diesen Informationsverlust nicht.

## Benötigte Änderung außerhalb von owns

Der Integrator muss einen eigenen Anchors-Adapter mit einem vereinbarten
Optionsvertrag bereitstellen. Schematischer Diff-Auszug für
`crates/os-analyze/src/details/mod.rs`:

```diff
-store_source! { anchors => anchors, path => crawl_path, sd => structured_data }
+store_source! { path => crawl_path, sd => structured_data }
+// Eigener anchors-Adapter: Richtung, Filter und Sortierung weiterreichen.
+// Die konkreten Optionstypen müssen im Schnittstellenvertrag ergänzt werden.
```

Der Store-Vertrag muss mindestens Richtung (Inlinks/Outlinks), Self-Link-
Ausschluss und Nofollow-Ausschluss transportieren. Für die Auswahl einer
Ankergruppe muss der Einzel-Link-Pfad dieselbe Textnormalisierung verwenden.
Die Anbindung der öffentlichen Inlinks-/Outlinks-Ansichten an diesen Adapter
muss ebenfalls im Schnittstellenpaket geprüft werden.

Dies ist keine Änderung eines eigenen Stub-Eintrags in einem F05-Vertragstest
oder einer Stub-Liste; K2-60 erlaubt die erforderliche Produktionsänderung
nicht. Nach C10 wird das Paket deshalb vor der Implementierung abgebrochen.
Keine eingefrorene Datei und kein Stub wurden geändert.

## Übergabe

- Umgesetzte Spec-IDs: keine; Paket nicht abgeschlossen.
- Blockierte Kriterien: TECH-146, damit vollständige Paketabnahme M2-D08.
- Tests: keine neuen Tests; die verpflichtenden Workspace-Prüfungen werden
  nach Behebung des Schnittstellenbedarfs und Umsetzung des Pakets benötigt.
- Fortsetzung: erst nach bereitgestelltem Optionsvertrag und Adapter oder
  einer ausdrücklichen Erweiterung des Dateibesitzes.

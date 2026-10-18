# M2-D04: Canonicals

Die Fixture ist eigenständig aus der Spec erstellt. `attributes.html` und
`headers.txt` prüfen erlaubte und unerlaubte Attribute, relative Schemas und
Fragmente einschließlich leerem Fragment. `scenario.toml` ist eine lokale
Mini-Site für den echten Engine-/Store-Pfad. `graph.sql` beschreibt eine
reproduzierbare Crawl-Datei mit Canonical-, Header-, Redirect- und gemischten
Ketten, Schleife, unbekanntem Ziel, PDF-/Bild-Ressourcen, externen Links und
einer durch Respect Canonical verborgenen Quelle. `support.rs` lädt diese
Datei für die Analyse- und Exporttests.

## Paketbericht

Umgesetzt: TECH-019, TECH-020, TECH-021, TECH-022, TECH-023, TECH-025,
TECH-026, OUT-054, OUT-083, OUT-086, OUT-087. Mitgeprüft: TECH-011 [M2],
TECH-008 [P1], OUT-085 [P1]; TECH-015 als PDF-Regressionskriterium aus M2-S05.

Die fünf zusätzlichen Canonical-Filter besitzen Issue-Klassifizierung,
eigene Texte in Deutsch und Englisch sowie passende Doku-Seiten. Die zwölf
Inlinks-Exporte verwenden die tatsächlichen Tab-Prädikate und das gemeinsame
Standard-Linkformat. Die Reports teilen einen SQLite-basierten Kettenwalker
mit dynamischen Sprungspalten, Schleifenerkennung, Sprunglimit, temporären
Redirects und Listenreihenfolge. Der Nachlauf aktualisiert Unlinked-Flags
wiederholbar, stempelt den Änderungsfeed und rollt bei Abbruch zurück.
Bereits bekannte Canonical-Ziele werden auch mit ausgeschaltetem
Canonical-Crawling bewertet; unbekannte Ziele werden nicht als Unlinked
oder Non-Indexable Canonical gemeldet.

PDFs ohne Canonical stehen gemäß TECH-011/015 in All und Missing. DEV-035
ist deshalb entfernt. Die bisherigen M1-Tests wurden entsprechend ergänzt;
der alte leere Respect-Canonical-Ignore wurde durch einen vollständigen
C9-Integrationstest ersetzt.

## Fremdergänzungen (K2-61)

`crates/os-engine/src/worker.rs`, in `assemble` nach den Store-Schaltern:
Die vorhandenen `CanonicalRef.invalid_attribute`-Werte wurden bisher nicht
nach `url_ext` transportiert. Der zusätzliche Aufruf überträgt die neuen
D04-Attribute und den eigenen Coverage-Marker `canonical.checked`. Dieser
wird nur bei aktivierter HTML-Validierung gesetzt, wenn HTML tatsächlich
geparst wurde oder die Ressource als Nicht-HTML keine Head-Prüfung benötigt.
Der echte lokale Crawl prüft HTML und HTTP-Canonical am PDF.

```diff
+    // M2-D04 (K2-61): transportiert Canonical-Attribute und den Prüfmarker des Dokumentbaums.
+    if keep_canonicals {
+        batch.ext.extend(
+            os_parse::canonical::page_facts(
+                &canonicals,
+                cfg.advanced.html_validation
+                    && (parsed.is_some() || facts.content_class != ContentClass::Html),
+            )
+            .into_iter()
+            .map(|(key, value)| os_model::ExtWrite {
+                url: item.id,
+                key,
+                value,
+            }),
+        );
+    }
```

Vier Goldens erhalten ausschließlich je eine zusätzliche PDF-Zeile aus der
eigenen Populationserweiterung (mit `UPDATE_GOLDEN=1` neu erzeugt):

| Datei | Vollständiger Zeilendiff |
|---|---|
| `tests/golden/tech/canonical-basic/canonicals__all.csv` | `+/none.pdf,0,,` |
| `tests/golden/tech/canonical-basic/canonicals__missing.csv` | `+/none.pdf,0,,` |
| `tests/golden/tech/canonical-extract/canonicals__all.csv` | `+/plain.pdf,0,,` |
| `tests/golden/tech/canonical-extract/canonicals__missing.csv` | `+/plain.pdf,0,,` |

## Stub-Ablösung (K2-60)

Aus `crates/os-engine/f05a-stubs.toml` sind ausschließlich die fünf Einträge
mit Besitzer M2-D04 entfernt: `canonical::response_facts`, Bulk Canonicals,
Canonical Chains, Non-Indexable Canonicals und Redirect & Canonical Chains.

## Abweichungen und Integrationsgrenzen

Keine fachliche Abweichung von der Spec. C9/K2-26: Ein vollständiger Test in
`crates/os-analyze/tests/pending_w1_d04.rs` wartet auf einen fremden Welle-1-Rumpf:

- M2-E01: Erzeugung des `respect_hidden=canonical`-Flags; die D04-Auswertung
  wird bereits ohne Ignore gegen die Store-Fixture geprüft.

Disabled, NeedsRecrawl und Incomplete werden gegen die Store-Fixture
geprüft. Der echte Crawl prüft den eigenen Coverage-Marker, leeres Fragment
und HTML-/HTTP-Unterscheidung ohne Ignore. D04 erzeugt die Canonical-Coverage selbst.

Always Follow im Listenmodus gehört nach K2-32 zu E06. D04 prüft den
Standardlauf einschließlich `Not Followed`. Keine offene fachliche Frage.
Kein Push, Merge oder PR; daher keine CI-Lauf-ID. Desktop-Dateien sind
unverändert, die zusätzlichen npm-Prüfungen sind nicht erforderlich.

## Verifikation

Lokal auf macOS arm64 mit rustc 1.99.1; finaler Stand, ohne parallele Builds:

```sh
cargo fmt --all --check
cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings
cargo test --workspace --exclude openspider-desktop --locked
cargo deny check licenses bans sources
bash scripts/check-neutral.sh
```

Alle Befehle erfolgreich. Workspace: **2.327 bestanden, 0 fehlgeschlagen,
40 ignoriert**, einschließlich des einen D04-C9-Tests für E01; 260 Testziele
inklusive Dokumentationstests. Fmt, Clippy und der vollständige Testlauf
dauerten zusammen lokal etwa **411 Sekunden**. Die Lizenzprüfung meldet
`bans ok, licenses ok, sources ok`; die Neutralitätsprüfung hat keine
Fundstellen (nur bestehende Warnungen über verwaiste Allowlist-Ausnahmen).

D04 ergänzt 20 aktive Fixture-/Regressionstests über Parser, Analyse und
Exporte; zusätzlich werden die 22 vorhandenen Canonical-Tab-Tests fortgeführt.
Die Tests prüfen auch bekannte gegenüber unbekannten Zielen bei deaktiviertem
Canonical-Crawling, CSV/XLSX, Header-Reihenfolge, alle zwölf Filter-Inlinks,
Schleifen, eigenständig verlinkte Zwischenglieder und die längenabhängige
Kopfzeile. Keine Fremdtests geändert.

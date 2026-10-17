# N00 – Spec-Tags in den M2-F02-Tests (Ergebnisbericht)

Branch `m2/n00-spec-tags`, Basis `be2a4fc` (`origin/main`, „m2: Merge M2-F05b (Welle 0e)“). Nur Kommentare, keine Testlogik.

## Auftrag

In `crates/os-analyze/tests/{filters_registry,catalog_split}.rs` und `crates/os-store/tests/{writer_m2_roundtrip,schema_v3_tables}.rs`
je Test den passenden `// spec: <ID>`-Kommentar ergänzen. Erlaubt sind die IDs aus Paket M2-F02 (`docs/plan/M2-packages.json`:
`spec_ids` OUT-233, OUT-234; `extra_spec_ids` OUT-184) und aus `docs/plan/M2.md` Abschnitt 3.4.

## Ergebnis

| Datei | Tests | getaggt |
|---|---|---|
| `crates/os-analyze/tests/filters_registry.rs` | 10 | 0 |
| `crates/os-analyze/tests/catalog_split.rs` | 6 | 0 |
| `crates/os-store/tests/writer_m2_roundtrip.rs` | 11 | 0 |
| `crates/os-store/tests/schema_v3_tables.rs` | 6 | 1 |

Getaggt ist nur `schema_v3_tables.rs::a_reopened_v3_file_has_all_tables_and_views_and_the_integrity_check_passes` mit
`// spec: OUT-233` und `// spec: OUT-234` (Schreibweise wie `read_api_manifest.rs`). Der Test öffnet eine neue v3-Datei
erneut nur lesend und prüft, dass genau die sechs `q_`-Sichten der Version 1 in der Datei gespeichert sind (`q_manifest`
nach OUT-233; `q_columns`, `q_config`, `q_links`, `q_sitemap_urls`, `q_urls` nach OUT-234 „Sichten, Version 1“ und
„Zugriff: Die Sichten sind in der Datei gespeichert“). Das ist die Regel aus M2.md 3.4a („jede Datei ab Schema v3 enthält die
Sichten der Version `read_api_version = 1`“). Die einzelnen Akzeptanzkriterien von OUT-233 und OUT-234 belegen weiterhin
`read_api_manifest.rs`, `read_api_views.rs`, `os-cli/tests/read_api_export.rs` und `scripts/ci/read_api_smoke.py`. Beide IDs
gehören F02 selbst; der Tag verschiebt ihren Status in `os-parity coverage` nicht.

Außerdem ist der Kopfkommentar von `schema_v3_tables.rs` angepasst: Dort stand „Infrastrukturtest ohne Spec-Tag“. Jetzt steht
dort, dass nur der letzte Test die `q_`-Sichten prüft (OUT-233, OUT-234).

Entscheidung (Nutzer, 17.10.2026): nur echte Treffer taggen. Die übrigen 32 Tests bleiben ohne Tag. Gründe siehe nächster
Abschnitt.

## Nicht getaggte Tests und Gründe

Die Kopfkommentare aller vier Dateien weisen die Tests bewusst als Infrastrukturtests ohne Spec-Tag aus (`writer_m2_roundtrip.rs`:
Tags nur, „wo sie ein Akzeptanzkriterium wirklich prüfen“). Ein Tag auf eine 3.4-ID würde die ID in `os-parity coverage` als
`implemented` zählen (mindestens ein getaggter Test mit `ok`), obwohl das Fachpaket die Funktion noch nicht geliefert hat.
OUT-184 passt auf keinen dieser Tests; die Migration als Kopie belegt `schema_v3_migrate.rs` (dort schon `OUT-184 [M2]`).

**`filters_registry.rs` (10 Tests), `catalog_split.rs` (6 Tests):** Sie prüfen Register und Issue-Katalog nach M2.md 3.5,
nicht Schema und Writer nach 3.4. Aus F02 und 3.4 passt keine ID. Inhaltlich nahe lägen UI-004 (Tab-Reihenfolge, Paket
M2-U10) bzw. UI-063/UI-064 (Fachbedeutung des Katalogs, belegt in `issues_catalog.rs` und `issues_texts.rs`). Diese IDs
stehen weder in F02 noch in 3.4.

**`writer_m2_roundtrip.rs` (11 Tests):**

| Test | Tabelle | ID laut 3.4 (Paket) | Grund ohne Tag |
|---|---|---|---|
| `ext_values_roundtrip_in_their_columns_and_upsert_replaces` | `url_ext` | keine | 3.4 nennt für `url_ext` keine Spec-ID |
| `ext_rows_stamp_url_ext_and_the_change_feed_lists_the_url` | `url_ext`, `changed_since` | keine | Writer-Regel aus dem Text von 3.4, ohne Spec-ID |
| `invalid_ext_key_and_unknown_url_are_rejected_without_touching_the_file` | `url_ext` | keine | Fehlerfälle des Writers, ohne Spec-ID |
| `structured_data_roundtrip_links_issues_to_items_and_replaces_per_url` | `sd_items`, `sd_issues` | TECH-180 (D09) | Kriterien betreffen die JSON-LD-Extraktion, nicht das Speichern |
| `render_results_roundtrip_with_children_and_replace_per_url` | `render_*`, `screenshots` | keine („Rendering“) | 3.4 nennt keine ID; der Dateikopf nennt CRAWL-084, das weder in F02 noch in 3.4 steht |
| `accessibility_spelling_forms_and_custom_values_roundtrip` | `a11y_*`, `spelling_findings`, `forms`, `custom_*` | TECH-250 (D21), ONPAGE-153 (D19), ONPAGE-034/035 (D11); `custom_*` ohne ID | Kriterien betreffen axe-Lauf, Sprachwahl und Security-Filter, nicht das Speichern |
| `minhash_signatures_roundtrip_as_little_endian_blobs` | `minhash` | ONPAGE-144 (D02) | Kriterien betreffen Shingling, Jaccard und den Filter |
| `api_values_roundtrip_upsert_and_unknown_urls_land_in_api_urls` | `api_values`, `api_urls` | keine („04 A, E–J“) | 3.4 nennt keine Spec-ID |
| `page_html_is_stored_with_the_page_and_replaced_or_cleared_on_recrawl` | `page_html` | CRAWL-063 (E01) | Grenzfall, siehe unten |
| `sitemap_error_and_entry_extras_roundtrip` | `sitemaps.error`, `sitemap_entries` | CRAWL-060 (E02) | Grenzfall, siehe unten |
| `m2_parts_in_the_same_batch_as_the_page_share_one_transaction_and_sequence` | alle M2-Teile | keine | Transaktions- und `change_seq`-Regel aus dem Text von 3.4, ohne Spec-ID |

**`schema_v3_tables.rs` (5 weitere Tests):**

| Test | Tabelle | ID laut 3.4 (Paket) | Grund ohne Tag |
|---|---|---|---|
| `view_cache_keeps_an_opaque_blob_per_key_and_parameter_hash` | `view_cache` | K-1a (Klärung, keine Spec-ID) | keine Spec-ID |
| `serp_edits_has_one_row_per_url_with_a_default_device` | `serp_edits` | ONPAGE-194 (D01) | Das Kriterium „nach erneutem Öffnen der Crawl-Datei ist die Bearbeitung noch sichtbar“ prüft der Test nicht; er prüft nur Schlüssel und Standardgerät |
| `near_duplicates_and_url_segments_are_unique_per_pair` | `near_duplicates`, `url_segments` | ONPAGE-144/145 (D02), K-14 | Kriterien betreffen Filter und Lower-Tab |
| `api_status_and_api_errors_roundtrip` | `api_status`, `api_errors` | keine | 3.4 nennt keine Spec-ID |
| `sitemap_columns_added_by_v3_default_to_empty_values` | `sitemaps`, `sitemap_entries` | CRAWL-060 (E02) | prüft nur Standardwerte neuer Spalten |

## Spec-Unklarheiten und Hinweise für den Planer

- **Grenzfall CRAWL-063:** Kriterium 1 („byte-identisch zum dekomprimierten Response-Body, Zeichensatz wird mitgespeichert“)
  prüft der Writer-Test nur zur Hälfte. Er zeigt, dass der Writer übergebene Bytes und Zeichensatz unverändert speichert. Den
  Vergleich mit dem Response-Body und die zstd-Kompression (Kriterium 2) prüft er nicht. Den Tag soll E01 mit dem Engine-Test
  setzen.
- **Grenzfall CRAWL-060:** Kriterium 4 `[M2]` („Fehler wird an der Sitemap vermerkt“) braucht den Parser (E02). Der Writer-Test
  zeigt nur, dass `sitemaps.error` gespeichert wird.
- Die Dateiköpfe von `writer_m2_roundtrip.rs` nennen IDs außerhalb von 3.4 (CRAWL-084, INTEG-001, INTEG-020/040/060). Die
  M2.md-Tabelle 3.4 nennt für `render_*`, `custom_*` und `api_*` keine IDs. Falls diese Zuordnung verbindlich sein soll, gehört
  sie in die Tabelle 3.4.

## Schnittstellenbedarf

Keiner.

## Prüfungen

Plattform: Claude-Code-Cloud-Session, Linux 6.18 x86_64, 4 CPU; rustc 1.97.0, cargo 1.97.0.

| Befehl | Ergebnis | Laufzeit |
|---|---|---|
| `cargo fmt --all --check` | grün | < 5 s |
| `cargo clippy -p os-analyze -p os-store --all-targets --locked -- -D warnings` | grün | 138 s (mit Kompilieren) |
| `cargo test -p os-analyze -p os-store --locked` | grün: 544 bestanden, 0 fehlgeschlagen, 20 ignoriert (unverändert) | 81 s (Testbinärdateien vorgebaut) |
Workspace-Lauf: entfällt, CI nach Push (Entscheidung Nutzer: reine Kommentaränderung). Vorher lief schon ein
`cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` durch (grün, 119 s). Der
anschließende Workspace-Testlauf brach beim Kompilieren ab, weil das Speicherkontingent der Session erschöpft war
(`No space left on device`). Das war kein Testergebnis.

Die Desktop-App (`openspider-desktop`, `npm run -w apps/desktop …`) ist nach M1.md 2.7 ausgenommen: Kein Pfad unter `apps/**`
ist berührt. `cargo deny` ist nicht gelaufen: Es gibt keine Änderung an Manifesten oder `Cargo.lock`.

Größe des Target-Verzeichnisses: 30 GB beim Abbruch des Workspace-Laufs (Debug-Profil mit Debuginfo). Nach Abschluss gelöscht.

CI-Lauf-ID: `linux-core` 39080882190 für Commit `5bb032d`
(https://github.com/domdewom2/openspider/actions/runs/39080882190). Ergebnis stand beim Eintragen noch aus. Andere Workflows
starten bei diesem Push nicht: `ci.yml` (macOS-/Windows-Matrix) und `parity.yml` laufen bei Push nur auf `main`, `ui-e2e.yml`
nur bei Änderungen unter `apps/desktop/**`, `crates/os-app/**` oder `crates/os-model/**`.

# M2-Q02b – Paritätslauf 2 nach N06–N11 und K2-96

Der vollständige Endlauf ist grün: **218/218 Exporte**, **null offene Zellen oder URL-/Beziehungsabweichungen**, drei begründete Skips, **10/10 Sites present**. Die zusätzliche Messung gegenüber den ursprünglichen 217 Vergleichen ist der freigeschaltete Crawl Path. **verified 129/155 → 129/155 (83,226 %)**; M1-Nachlauf **38/38**. Alle 23 ursprünglichen roten Exporte sind geschlossen.

Basis: `m2/q02b` von lokalem `m2/int-17` (`f7ad3f29`), N06–N11 integriert. Produkt- und Spec-Korrekturen sowie der Crawl-Path-Messfix wurden durch den Orchestrator-Nachtrag vom 30.10. ausdrücklich freigegeben (K2-96). Keine Referenzdatei, Export-Schwelle, Abhängigkeit oder fremde Paketdatei geändert. Tokio 1.53.3; kein Baum-Update, kein Push.

## B3a–g

| Kriterium | Endbeleg |
|---|---|
| B3a – M1 | **38/38 grün**, null Word-Count-Fehler, mit historischen A3-Mappings aus `b4aebd7c` und aktuellen OS-Ausgaben/DEV-Regeln. [Native Diffs](q02b/m1-k68.json), [Nachlauf](q02b/m1_nachlauf.py). |
| B3b – neue Exporte / Internal-All | **218/218 grün**, alle Export-Schwellen erfüllt. Internal-All auf allen zehn Sites nach begründeten DEV-Regeln identische URL-Menge; ungültige Uploadzeilen nach OUT-015 bleiben erhalten, Reihenfolge grün. [Endzahlen](q02b/endzahlen.json), [Internal-Mengen](q02b/internal-urlsets.json). |
| B3c – Overview | **Alle zehn Overviews grün**, 100-%-Zählerprüfung nach eng begründeten DEV-/REF-Regeln. Auch Below 200 Pixels jetzt exakt 23/23. [Endjournal](q02b/lauf-final.json.gz). |
| B3d – neue Sites | **5/5 neue Sites present**, insgesamt 10/10. [Endzahlen](q02b/endzahlen.json). |
| B3e – Abweichungen | **Keine unerklärte Restabweichung.** Alle verbleibenden zugelassenen Abweichungen haben Spec-ID und Begründung in `deviations.toml`; keine neuen pauschalen Ausnahmen. [Leere Fix-Restliste](q02b/restbefunde.json), [Entscheide](q02b/entscheidungen-q02b.md), [Original-23](q02b/original-23.json). |
| B3f – ohne Referenz | **217/217 IDs** mit K2-14-Grund und vollständigem nativen Fixture-Status, jeweils mindestens ein erfolgreicher getaggter Test. [Liste und Testbelege](q02b/ohne-referenz.json); kein verified-Zuschlag und keine vollständige B2/UI-Abnahme behauptet. |
| B3g – verified je Filter-ID | **129/155 = 83,226 %**, Ausgangswert gehalten, Ziel ≥93 erfüllt. Native `os-parity coverage` nach SQ-01/K2-84, ohne eigene Statusberechnung: [vor K2-96](q02b/coverage-before-k296.json), [nachher](q02b/coverage-after.json), [verified-IDs je Tab](q02b/verified-je-tab.json). |

`ONPAGE-147` bleibt trotz grünem Soft-404-Export ohne verified: historischer M1-PDF-DEV im Content-All-Beleg zählt nicht als M2-Verifikation. Internal/Links bleiben nach SQ-10 bis zur Link-Score-Kalibrierung ohne verified. REF-FREE erzeugt keine verified-Gutschrift.

## Fachliche Schließung und Messung

- OUT-098: SERP Summary nur für interne HTML-Seiten mit 2xx; Fehler- und Weiterleitungsseiten ausgeschlossen.
- OUT-087: indexierbare Unlinked-Canonical-Ziele nur im eigenen Unlinked-Filter/Report; nicht indexierbare Unlinked-Ziele bleiben hier enthalten.
- OUT-093: fehlender Rücklinkcode leer; kein kopierter Hinlinkcode und keine Leerwert-Ausnahme im Mapping.
- UI-038/ONPAGE-096: im Profil 20 px Regular `/` von 6 auf 8 px, `.` von 6 auf 5 px. **1.021 native Titelmessungen, 297 verbessert, keiner verschlechtert**, 1.020 exakt; bestehender STRASSE-Restwert innerhalb der unveränderten 3-%-Metrik. 936 nichtleere Titel zusätzlich gegen echte CLI-Endwerte geprüft. Ziffern, andere Schriftprofile und die 200-px-Grenze unverändert. K2-63: ausschließlich zwei eigene Golden-Zellen für `A <b>B</b>` von 106 auf 108 px nachgezogen (eine Slash-Glyphe +2); keine Referenzdatei betroffen. [Kalibrierungsbeleg](q02b/pixel-k296.json), [native Messquelle](q02b/pixel_measure.rs).
- Internal: fragmentfreie Canonical-Anzeige bei unverändert gespeicherter Annotation; Text Ratio trennt in den Body reparierte title-Elemente, ohne Inhaltswortzahl oder normale Inline-Grenzen zu ändern. Die drei offenen Internal-Zellen sind geschlossen.
- OUT-031/ONPAGE-012: Größe bekannter ungecrawlter interner Ziele im Linkexport auf 0 präzisiert; unbekannte externe Größen bleiben leer. Kein fachlicher Grund für die vorher unbestimmte Null-/Leer-Differenz, eng auf diese Population begrenzt. K2-63: zwölf entsprechende eigene Größen-Goldenwerte leer → 0, keine sonstige Zelle und keine Referenzdatei verändert; [Einzelbeleg](q02b/golden-k263.json).
- OUT-023: Runner verwendet `--export-crawl-path` mit erst nach Site-Start gebundener Ziel-URL. Neun Knoten → acht Verbindungen, **100 %**, identische Verbindungsmenge. Nur vollständig leere Referenz-Trenner entfallen. Prüfung der Steps, Ziel-/Startknoten, Zyklen und Zielblöcke vor der Projektion; Source/Destination/Ordinal sowie Typ, Anchor, Quelltiefe und Zielstatus bleiben geprüft. Redirect-/Mehrziel- und Negativtests sichern die Projektion. [Freigabe und Umsetzung](q02b/messfehler-crawl-path.md).

Die bereits begründeten DEV-/REF-Fälle bleiben erhalten: TECH-133/CRAWL-089, TECH-137 einschließlich external-99 mit Overview 2/1, ONPAGE-147 deutsche Soft-404, K2-95-Lesbarkeit, SQ-10/13e/15 und die belegten Folgepopulationen. SQ-09 ist durch den neuen M1-Nachlauf bestätigt. Alle Einzelgründe und die sechs früheren Mappingkorrekturen stehen im [Entscheidungsprotokoll](q02b/entscheidungen-q02b.md).

Die drei Skips sind ausschließlich die in der Referenz nicht verfügbaren Near-Duplicates-Tab-/Bulk- und Form-on-HTTP-Bulk-Exporte. Crawl Path wird vollständig gemessen. Keine Referenzdatei verändert.

## G01–G25 und M2-backlog

Alle Gruppen sind nachgemessen; die vorher offenen G02/G03/G06/G15/G17/G19/G21 sind fachlich geschlossen. G01/G04/G07/G10–G14/G16/G20/G22/G24/G25 erfüllen die integrierten Mess-/Spec-Regeln. G08/G09 und die übrigen G19-Populationen sind adressgenau begründet. Die dauerhaften Restklassen G05 (Referenzartefakte), G18 (TECH-145 Link Score, Kalibrierung M3) und G23-R (OUT-083 Chain Type / ausdrücklich festgelegter Selbst-Refresh) bleiben dokumentierte DEV-/REF-Fälle; keine unerklärte rote Zelle. Link Score bleibt als Auftrag für M2-backlog/M3, ohne zusätzliche verified-Gutschrift.

## Prüfungen

Linux-Build-Server, parallele N12/N13-/M3-Arbeit berücksichtigt. Schwere Befehle mit `timeout 3600s nice -n 10`, Cargo `-j 6 --locked`, `CARGO_INCREMENTAL=0`, Secret Store memory und installiertem Chromium für Renderingtests. Der verbindliche Workspace-Nachlauf verwendet **C5 `line-tables-only`** für Dev und Test; vorbereitende Builds mit Profil 0 wurden vor dem Testbeginn beendet. Keine Symbolbereinigung.

| Prüfung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | grün |
| Clippy Workspace/all-targets/-D warnings | grün; Desktop ausgeschlossen, N13 hier nicht integriert |
| Workspace-Tests | **Exit 0, 536 Targets, 3.988 bestanden, 0 Fehler, 43 bestehende Ignores**; vollständiger C5-Gesamtbefehl, 24.8 min einschließlich Neubau. [Beleg](q02b/workspace-summary.json) |
| Frontend | Typecheck grün, **90 Dateien / 857 Tests**; check:static **29 Tests**; npm-Lizenzen **474 Pakete grün** |
| `cargo deny check licenses bans sources` | grün |
| `os-parity run --all` | **Exit 0, 218/218**, drei begründete Skips |
| `os-parity coverage` | **129/155 → 129/155** |
| historische M1/A3-Vergleiche | **38/38**, null Word-Count-Fehler |
| Neutralität | grün |

**PAKET FERTIG:** B3a–g einzeln belegt und Paritätsgate geschlossen. INTEG-009/pending_w1_i01 sowie pending_w1_k04 sind im aktuellen Gesamtbefehl grün; die Untersuchung der Rennenfamilie bleibt ausdrücklich N13, Q02b hat deren Logik nicht geändert. Q04 kann den B3-Beleg übernehmen. Der erste C5-Lauf mit alter Pixel-Golden-Erwartung und die vollständige zweite Messung mit den drei inzwischen korrigierten Test-/Goldenfehlern bleiben erhalten; nur der abschließende vollständig grüne Gesamtbefehl ist der aktuelle C4-Beleg.

Die ergänzende native M2-Code-Coverage wurde ausschließlich zur B3f-Testzuordnung mit dem Rust-Gesamtlog ausgeführt. Ihr vollständiger B2-/Anhang-B-Modus bleibt **Exit 1** (29 offene IDs, vier bestehende ungültige Ignore-Gründe und 62 offene Anhang-B-Zuordnungen, unter anderem ohne Frontend-JUnit). Diese Befunde sind in `q02b/spec-coverage-k296.log.gz` und `q02b/q02b-spec-coverage.json.gz` erhalten; kein vollständiges B2-/UI-Gate behauptet. Die davon getrennte native Filter-Coverage für B3g ist **Exit 0, 129/155 verified**; alle 217 K2-14-IDs besitzen vollständige Fixture-Belege.

Keine Frontendänderung, deshalb keine zusätzlichen E2E-Suiten. Keine CI-Lauf-ID, kein Push; keine native Plattform-/Screenreader-Abnahme behauptet. Eigene Build- und RAM-Artefakte wurden nach Sicherung aller Belege entfernt ([Cleanup](q02b/cleanup.json)); keine fremden Prozesse oder Targets angefasst.

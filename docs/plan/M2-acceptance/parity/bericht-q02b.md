# M2-Q02b – Paritätslauf 2 nach N06–N11

**PAKET OFFEN.** Die vollständige Triage und die beschlossenen DEV-Einträge sind geliefert; das verlangte vollständig geschlossene Paritätsgate ist noch nicht erreicht. Endmessung: **212/217 Exporte grün, fünf rot, vier übersprungen**, zehn Sites (alle fünf neuen Sites `present`). **verified 129/155 → 129/155 (83,226 %)**. Die fünf roten Exporte brauchen die unten verlinkten Fix-/Spec-Aufträge; Crawl Path braucht den freizugebenden Runner-Messfix.

Basis: Branch `m2/q02b`, Integrationsstand `m2/int-17`, Startcommit `f7ad3f29`. N06–N11 sind enthalten. Zusätzlich bleibt der C4-Befund INTEG-009 offen (View-Version wird vor api.fetch stale). Kein Produktcode, keine Referenzdatei und keine Export-Schwelle geändert; Tokio **1.53.3**, Cargo.lock unverändert, kein Baum-Update. Keine Frontendänderung, kein Push.

## B3a–g

| Kriterium | Ergebnis / Beleg |
|---|---|
| B3a – 38 M1-Vergleiche | **38/38 grün**, Word Count null Zellfehler. Neuer nativer Nachlauf mit historischen A3-Mappings aus `b4aebd7c`, aktuellen OS-Ausgaben und aktuellen DEV-Regeln: [m1-k68.json](q02b/m1-k68.json), [Nachlaufskript](q02b/m1_nachlauf.py). Die erweiterten M2-Overview-Zeilen werden separat unter B3c bewertet. |
| B3b – neue Exporte / Internal-All | **offen:** fünf Exporte unterschreiten Schwelle oder URL-Mengenregel. Internal-All auf allen zehn Sites nach dokumentierten DEV-Regeln identisch; Liste roh 56/58 Zeilen, die zwei ungültigen Uploadzeilen werden nach OUT-015 ausdrücklich erhalten (DEV-040/040b). Keine weggefilterten gültigen Listenzeilen; Reihenfolgeprüfung grün. |
| B3c – Overview | **offen:** eine unentschuldigte Zählerzelle auf m2-links (Below 200 Pixels: 90 Titel mit 202 px Referenz / 199 px OS kreuzen die Grenze). Alle übrigen Unterschiede sind behoben oder mit angewandten DEV-/REF-Regeln belegt. [Restbefunde](q02b/restbefunde.json), [vollständiges Endjournal](q02b/lauf-final.json.gz). |
| B3d – neue Sites | **5/5 present**, insgesamt 10/10 Sites. [Endzahlen](q02b/endzahlen.json). |
| B3e – Begründung je Abweichung | **Triage vollständig, fachliche Schließung offen:** 10 unentschuldigte Einzelbefunde = acht Zellen + zwei URL-/Beziehungszeilen. Jeder hat Spec-ID, Gruppe, Datei, Beispiel und Fix-Auftrag. Drei Zellen davon liegen innerhalb grüner Internal-Exporte; sie werden ausdrücklich mitgeführt. Ein Auftrag wird nicht als DEV oder bestanden gezählt. |
| B3f – ohne Referenz | **217 IDs** mit K2-14-Grund und mindestens einem erfolgreichen getaggten Fixture-Test gelistet; **216/217** haben vollständigen nativen Fixture-Status. INTEG-009 bleibt wegen `err-view-stale` im Hauptlauf offen. [Liste und Testbelege](q02b/ohne-referenz.json); native M1-Coverage ergänzt M1-IDs, die im M2-Paketkatalog nicht geführt werden. Keine verified-Gutschrift, keine pauschale Ausnahme gemischter IDs. |
| B3g – verified je Filter-ID | **129/155 = 83,226 %, Ziel ≥93 erfüllt und Ausgangswert gehalten.** Beide Zahlen durch `os-parity coverage`, ohne eigene Statusberechnung. [vorher](q02b/coverage-before.json), [nachher](q02b/coverage-after.json), [IDs je Tab](q02b/verified-je-tab.json). Eine in mehreren Tabs vertretene ID (z. B. ONPAGE-009) wird im Nenner nur einmal gezählt. |

`ONPAGE-147` ist trotz grünem eigenem Soft-404-Export weiterhin nicht verified: der Content-Tab-All-Beleg verwendet auf einer Alt-Site den historischen M1-PDF-DEV, der nach Q02-P06 keine M2-Verifikation ergibt. Ebenso bleiben Internal/Links nach SQ-10 bis zur Link-Score-Kalibrierung ohne verified. Die Coverage-Regel wurde nicht umgangen.

## Geschlossene Befunde und DEV-Entscheide

Von den ursprünglichen **23** roten Exporten wurden **18** geschlossen; es entstand kein neuer roter Export. [Original-23 mit Vorher/Nachher](q02b/original-23.json).

- Beschlossene TECH-133/CRAWL-089-, TECH-137- und deutsche Soft-404-Fälle als eng begrenzte DEV-046…049 erfasst. TECH-137 enthält sowohl external-99.html als auch Overview 2/1. SQ-09 mit neuem M1-Nachlauf erfüllt, kein Wortzählungs-DEV.
- DEV-053…056 schließen die konkret nachgemessenen Restfälle: Header-Selbst-Refresh nach OUT-081/083, TECH-139 als Folge von DEV-008, Missing Size Attributes als Folge von DEV-028 und den vorgeschriebenen `This page`-Treffer aus TECH-144. Eigene Filter-/Instanzexporte und native Abweichungsbelege sind erhalten.
- Bestehende SQ-10/13e/15-Regeln DEV-037, DEV-042 und REF-ART-01 bestätigt. Ergänzungen für ungültige Uploadzeilen, Not-Crawled-Status, Unknown/Other-Klassifizierung, eigene Loopback-Ziele und Ketten-Endstatus betreffen ausschließlich spezifizierte Fälle.
- Zwei Typ-Aliasse, falsche Bildspaltenzuordnung, XPath-Darstellung und zwei Hreflang-Beziehungszuordnungen im Mapping korrigiert. Die Pixelprüfung in zwei bislang bitgenauen Mappings folgt jetzt derselben bereits festgelegten UI-038/K-64-Metrik (≤3 %) wie Internal/Page Titles; 99/97/100-%-Export-Schwellen bleiben unverändert.
- REF-ART-02 ist durch eine native Typ-Probe belegt: der Non-200-Hreflang-Referenzreport enthält zusätzlich einen normalen Hyperlink. DEV-052 bildet die bereits entschiedene Missing-Head-Regel K2-93 ab; keine Ausnahme für den Validation-Tab insgesamt.

Vollständige Gründe, Werte und Abgrenzungen: [Entscheidungsprotokoll](q02b/entscheidungen-q02b.md). Alle Ausnahmen sind in `tests/parity/deviations.toml` maschinenlesbar; eigene Pfade und Gegenwerte verhindern pauschale Entschuldigungen.

## Restliste für Nachbesserung / M2-backlog

| Gruppe / Spec | Rest / nächster Schritt |
|---|---|
| G21 / ONPAGE-096, UI-038 | **23/113** vollständig durch graph/10.html … graph/99.html erklärt: Referenz **202 px**, OS **199 px**, 1,485 %. Population nativ identisch. Kalibrierung an der Grenze; keine URL-Regel und Schwelle 200 bleibt. [Beleg](q02b/pixel-boundary.json). |
| G02 / OUT-031, ONPAGE-012 | nicht gecrawlter nofollow-Link: Size **0/leer**, Export **95,652 %** bei 97-%-Schwelle. Status ist separat begründet; Größe nicht pauschal entschuldigt. |
| G19 / OUT-098 | SERP enthält zusätzlich **real404.html**; 2xx-Grundmenge gegenüber ONPAGE-009 präzisieren und fixen. Pixelwerte sind nach UI-038 korrekt gemessen. |
| G17 / OUT-087, TECH-020 | zusätzliche Beziehung **canonical-unlinked → canonical-unlinked-target**. Spec verlangt indexierbare unlinked-Ziele, Referenzreport lässt sie aus; Reportabgrenzung entscheiden. |
| G02 / OUT-093 | drei leere Referenz-Sprachcodes gegen OS **de/en/en**. Semantik Hin-/Rücklinkcode prüfen; keine Leerwert-Blanket-Regel. |
| G15/G06 / TECH-022, ONPAGE-011/013 | innerhalb grüner Internal-Ausgabe: einmal Fragmentanzeige, zweimal Text Ratio bei repariertem HTML. Gleichermaßen als Fix-Aufträge erfasst. |
| G03 / OUT-023 | Crawl Path nicht freigeschaltet: Runner kennt den N11-CLI-Weg nicht, Referenzverbindungen und OS-Knoten brauchen eine geprüfte Projektion. [Messfehler und konkreter Fix-Vorschlag](q02b/messfehler-crawl-path.md). |
| G18 / TECH-145 | Link-Score-Kalibrierung bleibt M3; bestehender DEV-037, keine zusätzliche verified-Gutschrift. |

K2-96 ist in der Basis noch kein eingetragener Beschluss. Wo eine konsistente Referenz ohne fachlichen Spec-Grund nachgewiesen wird, muss der Orchestrator Spec und Fix-Auftrag unter K2-96 nachführen. Q02b erfindet weder diesen Beschluss noch eine fachliche Begründung für ungeklärte Fälle. [Konkreter Entscheidungsbedarf](q02b/entscheidungen-q02b.md).

## G01–G25

G01/G02: dynamische Spalten und Mappings nach N06 nachgemessen; zwei Hreflang-Zuordnungen, Bild-Length und Typ-Aliasse korrigiert, OUT-093/Größe bleiben offen. G03: Crawl Path offen. G04: native Coverage-Regel und Nenner bestätigt. G05: REF-ART-01/02 belegt. G06: zwei Text-Ratio-Zellen offen. G07: Inhalts-Spaltensätze vorhanden, historischer PDF-Beleg zählt nicht verified. G08: K2-95/DEV-049, keine Body-Ausnahme. G09: neue Bildexporte grün, alte 404-Populationsfolge DEV-055 belegt. G10–G14: integrierte N07/N09/N10-Regeln nachgemessen, keine neue pauschale Ausnahme. G15: Internal-Fragmentanzeige offen. G16: integriert nachgemessen. G17: OUT-087 offen, Non-200-Beifang als REF-ART-02 belegt. G18: M3/DEV-037. G19: entschiedene DEV-Fälle einschließlich TECH-136/139 belegt; SERP offen. G20: neuer K-68-Beleg 38/38. G21: Sigma-Fälle nach N07 grün, neuer 202/199-px-Grenzfall offen. G22: Sitemap-Exporte grün. G23: Listenreihenfolge grün, Chain Type DEV-042, HTTP-Selbst-Refresh DEV-053. G24/G25: nach N07/N11 nachgemessen, kein offener Befund.

## Durchführung und Prüfungen

Linux, 16 Kerne; parallele N12/N13-/M3-Arbeit berücksichtigt. Schwere Befehle mit `timeout 3600s nice -n 10`, Cargo `-j 6 --locked`, `CARGO_INCREMENTAL=0`, Secret Store memory, explizites installiertes Chromium für Rust-Renderingtests. Dev-/Test-Debugsymbole auf 0 wie im Integrationsnachweis, Abweichung vom C5-Debugprofil nur zum Platzsparen. Keine Rust-/Frontend-Testquelle oder Golden-Erwartung angepasst; die Messkorrekturen sind im Paketbericht mit alter/neuer Zuordnung und Spec-ID begründet (K2-63).

| Prüfung | Ergebnis |
|---|---|
| fmt | grün |
| Clippy Workspace/all-targets | grün; `--exclude openspider-desktop`, N13 hier nicht integriert |
| Workspace-Tests | **536 Targets vollständig gemessen: 3977 bestanden, 1 Fehler, 43 bestehende Ignores.** Hauptlauf Exit 124 nach 3600 s; 36 verbleibende Programme und 17 Doc-Test-Targets separat grün. INTEG-009 (`err-view-stale`) bleibt offen; kein einzelner grüner Workspace-Befehl behauptet. [Aufteilung](q02b/workspace-summary.json) |
| Frontend | Typecheck grün; **90 Dateien / 857 Tests**, zusätzlich check:static **29 Tests**, 474 npm-Lizenzen grün |
| cargo deny | bans/licenses/sources grün |
| os-parity run --all | Exit 1, **212/217**, fünf rote Exporte, vier Skips |
| coverage | **129/155 → 129/155** |
| M1-Nachlauf | **38/38**, null Word-Count-Fehler |
| Neutralität / Planprüfung | grün |

Die drei unverfügbaren Referenzexporte bleiben transparent übersprungen (Near Duplicates Tab/Bulk, Form-on-HTTP-Bulk). Der vierte Skip ist Crawl Path: der überholte Manifesttext „CLI folgt in N11“ wurde berichtigt. Er nennt jetzt ausdrücklich die fehlende Runner-Ansteuerung und das Verbindungs-Mapping sowie die ausstehende Code-Freigabe.

Unter 25 GB wurde zuerst eigenes `target/debug/incremental` entfernt. Das anschließende Strippen fertiger Testprogramme beeinträchtigte einen Backtrace-Test; der Erstlauf wurde abgebrochen und bleibt mit vier bisherigen Fehlern erhalten ([first-failures.json](q02b/first-failures.json)). Alle Testprogramme wurden für den Wiederholungslauf neu und **ohne Strippen** gebaut. Dieser erreichte nach 3600 s beim Speicher-Skalierungstest das Limit. Die 36 noch nicht abgeschlossenen Programme wurden danach direkt aus den unveränderten kompilierten Artefakten ausgeführt; alle grün. Separater nativer Doc-Test-Lauf: 17/17 Targets grün. Der Gesamtbestand entspricht den 536 Integrations-Targets; ein einzelner grüner Workspace-Befehl liegt nicht vor. Der Ressourcenwächter lagert nur vor Testbeginn fertige Programme in ein eigenes ausführbares tmpfs aus, erhält die Binary-Verzeichnisstruktur und räumt erst fertig durchgelaufene Targets ab. Keine fremden Prozesse/Artefakte angefasst. Das eigene `target/` und die gezielt angelegten RAM-Profilverzeichnisse wurden vor Abgabe gelöscht (Nachweis cleanup.json).

Keine CI-Lauf-ID: kein Push beauftragt. Keine native Plattform-/Screenreader-Abnahme behauptet. Reproduzierbare Auswertung: [auswertung.py](q02b/auswertung.py); diese liest ausschließlich Runner-Journale und eigene OS-Ausgaben, keine Referenzdateien.


INTEG-009 ist separat in [workspace-summary.json](q02b/workspace-summary.json) mit Datei, Testname und nächstem Prüfschritt erfasst. Der vollständige Fallbestand wurde gemessen, der geforderte vollständig grüne Workspace-Befehl ist **nicht** erfüllt. Eine Behebung der View-/API-Produktlogik oder ein Test-Retry gehört nicht zur autorisierten Mess-/Klassifizierungsarbeit.

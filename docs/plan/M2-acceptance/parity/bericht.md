# M2-Q02 – Paritätslauf M2

**PAKET OFFEN. B3 ist nicht erfüllt.** Stand 30.10.2026, Linux x86_64, Branch `m2/q02`.
Basis `11949427`: Integrationsrunde 15 (`origin/parity/ref-m2b`, Produktstand `origin/m2/int-16`) plus Referenzexporte.
Keine Produktfehler repariert, keine Referenzdatei verändert, kein Push. Die 38 M1-Vergleiche bestehen mit den bisherigen
Mappings; die scharfe M2-Auswertung zeigt auf allen prüfbaren Sites offene Befunde.

## Ergebnis je Site

Der endgültige echte Crawl mit den zwei temporär aktivierten Mappings und den dokumentierten Ausnahmen
steht in [roh/lauf.json](roh/lauf.json). „Fehler“ bedeutet Export-/Mappingfehler vor dem Zellvergleich.
„Verfehlt“ umfasst Wert-, Mengen- und Reihenfolgefehler. Eine bestandene Prozentschwelle allein genügt nicht.

| Site | Einträge | Verglichen | Bestanden | Verfehlt | Fehler | Übersprungen |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| out-basic-site | 7 | 7 | 5 | 2 | 0 | 0 |
| parity-onpage | 12 | 12 | 11 | 1 | 0 | 0 |
| parity-technical | 8 | 8 | 6 | 2 | 0 | 0 |
| parity-response | 8 | 7 | 6 | 1 | 0 | 1 |
| parity-synthetic | 4 | 4 | 2 | 2 | 0 | 0 |
| parity-m2-onpage | 42 | 37 | 6 | 31 | 3 | 2 |
| parity-m2-technical | 90 | 82 | 50 | 32 | 8 | 0 |
| parity-m2-links | 23 | 13 | 6 | 7 | 10 | 0 |
| parity-m2-security | 22 | 19 | 16 | 3 | 2 | 1 |
| parity-m2-list | 5 | 5 | 1 | 4 | 0 | 0 |
| **Gesamt** | **221** | **194** | **109** | **85** | **23** | **4** |

Alle zehn Sites sind present, davon fünf neue (B3d erfüllt). Neue Sites: 179 present, drei unavailable;
156 Vergleiche, 79 bestanden, 77 verfehlt, 23 Fehler. Legacy: 38 Vergleiche; acht davon werden durch die
zusätzlichen M2-Prüfungen rot. Der alte M1-skip für OUT-084 ist im Manifest noch aktiv und gehört zur Nachbesserung.
Die drei unavailable-Dateien sind zwei Near-Duplicate-Exporte (Referenzanalyse standardmäßig aus) und ein nicht angebotener
Security-Bulk-Export. Das sind dokumentierte Referenzbesonderheiten, kein Beleg für eine Lizenzsperre.

Der erste Lauf mit unveränderten Mappings steht in [roh/basis.json](roh/basis.json): **38/38 M1-Vergleiche bestanden**;
auf den neuen Sites 134 Vergleiche, 65 bestanden, 69 verfehlt, 45 Export-/Mappingfehler. Der eindeutige Bindestrich-Fallback
erschließt im endgültigen Lauf weitere Dateien. Die beiden Läufe crawlen wirklich; die endgültigen Zahlen stammen
nicht aus einem Replay oder aus gefälschten OpenSpider-Ausgaben.

## Exporte, Spalten und Klassifizierung

- [exporte.csv](exporte.csv): jede Site/Datei, Zeilenzahlen, erreichte und geforderte Schwelle, URL-Menge, Reihenfolge und Ergebnis.
- [spalten.csv](spalten.csv): alle 2.457 verglichenen Spalten mit Zell- und Fehlerzahl, auch fehlerfreie Spalten.
- [befunde.csv](befunde.csv): **5.142 Einzelbefunde**: 2.070 Implementierungsfehler, 935 Mapping-/Formatfehler, 2.137 Referenzbesonderheiten. Dazu gehören auch Spaltenlücken und Mengenunterschiede; das ist keine Anzahl unabhängiger Produktfehler.
- [abweichungen.csv](abweichungen.csv): angewandte DEV-/REF-FREE-Regeln mit Spec-ID, Begründung, Umfang und Beispielen. Diese bereits entschuldigten Werte sind nicht noch einmal in den 5.142 Befunden enthalten.
- [nachbesserung.md](nachbesserung.md): 217 nach Crate, Kategorie, Spalte und Spec gebündelte Aufträge, mit Datei, Beispiel-URL, Soll-/Istwert, Schwere und Gate-Relevanz. Vollständige betroffene URL-Mengen stehen in der CSV.

Beim Abgleich der Spec wurden zusätzlich die Rendering-Filter ONPAGE-178/180 aus dem Nenner ausgeschlossen (160 statt vorläufig 162); ONPAGE-179 bleibt ohne Konfigurationsvoraussetzung referenzfähig.

Referenzbesonderheiten werden nicht stillschweigend aus der Messung entfernt: z. B. 0 statt leer für nicht vorhandene
Elemente, die Zuordnung fehlender Content-Types zu Other und die Behandlung ungültiger Listeneingaben. TECH-141 nimmt den eigenen Start-Host+Port ausdrücklich vom Localhost-Filter aus; ONPAGE-140/141 fügt zwischen benachbarten Inline-Ankern keine Block-Leerzeichen ein. Auch die Inlinks-Selbstlinkfälle sind Spec-Abweichungen, keine neuen Fachfehler. OUT-080 verlangt für needs_config/needs_rendering leere Zähler; die historischen Aufforderungen, diese generell mit Null zu füllen, wurden deshalb korrigiert. Die Spec kann
dabei den OpenSpider-Wert verlangen. Solche Fälle sind kein Auftrag, das Produkt an einen abweichenden Referenzwert anzupassen;
die noch nötige genaue Mapping-/Ausnahmeregel ist vor einer grünen Paritätsabnahme nachzutragen.
Die Klassifizierung ist eine Übergabe anhand der Exporte und der Spec, keine Behauptung bereits nachgewiesener interner Ursachen.

Akzeptierte Referenzeinschränkungen stehen in `tests/parity/deviations.toml`: REF-FREE-03–17 begrenzen genau die in
M2.md 6.1 / K2-14 nicht referenzfähigen Gruppen und vorhandenen Overview-Zellen. Fehlende Zeilen, Link Score, Security,
leere Zähler für Missing Size Attributes und fachliche Fehler werden dadurch nicht pauschal entschuldigt. Die alten REF-FREE-01/02 sind auf
die fünf alten Sites begrenzt: Die neuen Manifeste dokumentieren eine lizenzierte Erzeugung und rechtfertigen diese
historische Free-Sitemap-Ausnahme nicht. Die vollständige Bearbeitung von K2-66 steht in
[arbeitsliste-k2-66.md](arbeitsliste-k2-66.md); die Arbeitslisten-Quelldatei liegt außerhalb owns und bleibt unverändert.

## M2-B02 und B3g: verified

**0 / 160 referenzfähige Filter-Spec-IDs = 0,000 %; Ziel ≥ 60 % (mindestens 96 IDs). Nicht erfüllt.**
Der Nenner zählt eindeutige Spec-IDs der beiden kanonischen Filterkataloge, keine mehrfachen Tab-/Filterzeilen.
Ausgenommen sind Erweiterungen, dynamische Filter und die nicht referenzfähigen Funktionsgruppen aus 6.1 / K2-14.
Rote oder fehlende Exporte verkleinern den Nenner nicht. Definition, vollständige ID-Listen und Ablehnungsgründe stehen in
[verified.json](verified.json). Für 52 ausgeschlossene IDs nennt [ohne-referenz.csv](ohne-referenz.csv) vorhandene
spec-abgeleitete Testquellen; deren vollständige Ausführung gehört zu B2/Q01/Q04 und wird hier nicht behauptet.

`tests/parity/evidence.toml` enthält ein `export_result`-Journal für **alle 221 Manifest-Einträge** mit erreichtem
Schwellwert beziehungsweise Fehler-/Skipstatus. Die bestehenden granularen `evidence`-Einträge bleiben erhalten;
ONPAGE-012 wird um die neuen Internal-/Listen-Mapping-Slugs erweitert. Diese Größen-Spalten-ID ist keine Filter-ID
und erhöht den B3g-Zähler nicht. Unvollständige M2-Tab-Zuordnungen stehen als `candidate_evidence` mit allen zugehörigen
Mapping-Slugs in derselben Datei und erzeugen bewusst keinen verified-Status.

Der strenge Abgleich verlangt identische URL-Mengen, null Zellfehler, vollständige relevante Spalten und keine
angewandte REF-FREE-Regel. Schon External:All hat trotz 100 % Zellübereinstimmung fehlende Referenzspalten
`Redirect URL`/`Redirect Type`. Das aktuelle Coverage-Lesemodul prüft diese Vollständigkeit nicht selbst;
Q02-P02 verlangt dessen Nachbesserung. 15 zusätzliche Fixture-Tests für External sind grün; daraus wird kein
vollständiger Referenzbeleg erfunden. Die vorhandenen Belegdateien sind keine Neuberechnung des gesamten M2-Implementierungsstatus.

## M2-S04 und M2-S06

**M2-S04 / ONPAGE-005, A-4b: ICU-Root Secondary vorläufig beibehalten; Einzelentscheidung offen.**
Die Referenz und OpenSpider zählen sieben Page-Title-Duplikate. Das Titelinventar enthält neben den sieben Unicode-Fällen
nur das exakte Paar `/` und `/?s=stone`. Zum aktuellen ICU-Verhalten passt damit: zwei exakte Titel, zwei NFC/NFD-Titel,
drei Sigma-Titel; Straße/STRASSE verschieden. Ein reines `to_lowercase` erklärt diesen Gesamtzähler nicht.
**Der aggregierte Zähler beweist jedoch nicht getrennt, ob das zusätzliche Zweierpaar NFC/NFD oder Straße/STRASSE ist.**
Ein Duplicate-Filterexport mit den einzelnen Adressen fehlt im gelieferten Satz. Deshalb keine Spec-Änderung und keine
vollständige Bestätigung aller drei Gruppen. Nächster Schritt: Orchestrator liefert `Page Titles:Duplicate` für dieselbe
Site aus dem gespeicherten Crawl; Q02-S04 dann adressgenau schließen. Belege:
[probe-titel.json](roh/probe-titel.json), [probe-overview.json](roh/probe-overview.json).

**M2-S06 / ONPAGE-060, A-19b: bestätigt.** `/tail` ist im kombinierten Kettenbericht auf beiden Seiten enthalten,
Schleife wahr und drei Sprünge. Der Referenzkopf `Number of Redirects/Canonicals` benötigt einen zusätzlichen Alias zum
bestehenden Mapping; die isolierte Probe vergleicht ihn über os-parity. Die abweichenden Endzielwerte sind eine andere
Frage (OUT-081/083 und historische DEV-030), keine Widerlegung der Kettenmitgliedschaft.
Beleg: [probe-ketten.json](roh/probe-ketten.json).

**M2-S06 / ONPAGE-141, A-35b: bestätigt.** `/list-item-text.html` enthält im Inhaltsbereich acht Texttoken und zwei
Listenelemente, eines davon ohne Link. Referenz und OpenSpider liefern `Word Count = 10`.
Beleg: [probe-content.json](roh/probe-content.json). Die Probe-Dateien verwenden absichtlich `Q02_PROBE` als
OpenSpider-Zellwert, um auch gleiche Referenzwerte über den vorgesehenen os-parity-Leser sichtbar zu machen;
sie sind keine produktiven Fehlermessungen und gehen nicht in Schwellen oder verified ein. Echte Produktwerte stehen im Laufbericht.

Listenmodus: der Vergleich nutzt additiv `Original Url` + Vorkommensnummer, wenn diese Spalte beidseitig existiert;
`Address` wird weiter als Wert verglichen. Upload-Reihenfolge wird gesondert geprüft. Die Referenz enthält 56 Zeilen,
OpenSpider 58; die zwei zusätzlichen ungültigen Eingaben sind gemäß CRAWL-030 zu behalten. Referenzdateien bleiben unverändert.

## Befehle und Ausführung

Alle schweren Befehle mit `timeout 3600s nice -n 10`, Cargo mit `-j 8`, `--locked` und ohne cargo update.
Tokio bleibt auf 1.53.3. Die Umgebung setzt für Build/Test `CARGO_INCREMENTAL=0`,
`CARGO_PROFILE_DEV_DEBUG=line-tables-only` und `CARGO_PROFILE_TEST_DEBUG=line-tables-only`.

```bash
timeout 3600s nice -n 10 cargo build --locked -p os-cli -p os-parity -j 8
timeout 3600s nice -n 10 env OS_PARITY_MAX_EXAMPLES=100000 OPENSPIDER_SECRET_STORE=memory \
  target/debug/os-parity run --all --openspider target/debug/openspider --out target/parity
# Ausgabe von scharf.py als --root einsetzen; Original-Mappings bleiben unberührt:
python3 docs/plan/M2-acceptance/parity/scharf.py
timeout 3600s nice -n 10 env OS_PARITY_MAX_EXAMPLES=100000 OPENSPIDER_SECRET_STORE=memory \
  target/debug/os-parity run --all --root <temporäres-overlay> \
  --openspider target/debug/openspider --out target/parity-measured
python3 docs/plan/M2-acceptance/parity/auswerten.py target/parity-measured/report.json
python3 docs/plan/M2-acceptance/parity/belege.py target/parity-measured/report.json
cargo fmt --all --check
timeout 3600s nice -n 10 cargo clippy --locked -p os-parity --all-targets -j 8 -- -D warnings
timeout 3600s nice -n 10 cargo test --locked -p os-parity -j 8
# Nach dem bekannten Fehler zusätzlich alle restlichen Test-Targets ausführen:
timeout 3600s nice -n 10 cargo test --locked -p os-parity -j 8 --no-fail-fast
timeout 3600s nice -n 10 cargo test --locked -p os-analyze --test tab_external -j 8
bash scripts/check-neutral.sh
```

`OS_PARITY_MAX_EXAMPLES` liegt über jeder Fixture-Zeilenzahl; die Befundliste übernimmt damit alle Zellfehler,
nicht bloß die standardmäßig zehn Beispiele pro Spalte. Referenzen wurden ausschließlich durch os-parity eingelesen.
Die Zusatzproben erzeugen Sentinel-Kopien aus OpenSpider-Exporten, keine Kopien bearbeiteter Referenzdateien.
Keine Lasttests; Q03 kann parallel weiterlaufen. Keine vollständige Workspace-/Frontend-/CI-Prüfung beansprucht.
CI-Lauf-ID: keine, gemäß Auftrag kein Push; die Plattformmatrix bleibt beim Orchestrator.

## Abschluss und Übergabe

Fmt, Clippy und Neutralitätsprüfung sind grün. Der vollständige os-parity-Testlauf bleibt wegen genau eines bestehenden
Tests rot: `new_manifests_preserve_list_source_sitemap_variants_and_crawl_path_target`,
`crates/os-parity/tests/real_repo_files.rs:248`, erwartet Pending und erhält Present. Mit `--no-fail-fast`: 169 Tests bestanden, ein Test fehlgeschlagen, null ignoriert, kein weiteres fehlgeschlagenes Target. Die in elf Target-Zusammenfassungen ausgewiesenen Testzeiten summieren sich auf 1,35 s (ohne Buildzeit). Beide neuen Formattests bestehen. Logs: [tests.log](roh/tests.log),
[clippy.log](roh/clippy.log), [external-tests.log](roh/external-tests.log).

Gate-relevante Übergabe: **os-analyze** (Security-Populationen, Link Score, Filter-/Grundmengen, Canonicals/Hreflang),
**os-parse** (Lesbarkeit, einzelne Pixel-/Textwerte), **os-export** (leere Overview-Zähler, Ausgabeformate und Sitemap-Attribute),
**os-cli/os-parity** (Reportanforderung, veralteter Manifest-skip, unvollständige Mappings/Evidence-Prüfung und überholte Testerwartung).
Ungültige Listenzeilen sind eine Referenzbesonderheit mit **Gate nein als Fachfehler**.
Die verbindliche Einzelzuordnung mit Beispielen steht in [nachbesserung.md](nachbesserung.md).

**Nächster Schritt:** Orchestrator übernimmt die außerhalb owns liegenden Mapping-/Teständerungen, beauftragt die
Fachnachbesserungen und den fehlenden Duplicate-Filterexport; danach denselben vollständigen Lauf wiederholen.
`target/` wird vor Übergabe gelöscht, der Branch bleibt ohne Push mit sauberem Git-Status.

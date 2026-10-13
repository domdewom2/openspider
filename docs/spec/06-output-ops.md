# 06 – Exporte, Reports & Betrieb (ID-Präfix `OUT`)

Clean-Room-Spezifikation. Grundlage: öffentliche Bedienungsanleitung, FAQ, Tutorials und Release-Notes (Versionen 18.0–24.3) des Referenzprodukts. Alle Beschreibungen sind eigene Formulierungen. Wo das Referenzprodukt Verhalten nicht öffentlich festlegt oder wir bewusst abweichen, ist das als **(OS-Festlegung)** markiert – diese Punkte sind verbindlich für OpenSpider, aber keine Paritätsbehauptung.

Zuständige Crates (siehe `docs/ARCHITECTURE.md`): `os-export` (Formate, Bulk-Exporte, Reports, Sitemaps), `os-analyze` (Ketten, Orphans, hreflang-Rücklinks, Vergleich), `os-cli` (Kommandozeile, Scheduling-Ziel), `os-integrations` (Google Drive/Sheets, E-Mail-OAuth), `apps/desktop` (Dialoge).

## Übersicht / Inhaltsverzeichnis

| Abschnitt | IDs | Inhalt |
|---|---|---|
| [0 Konventionen](#0-konventionen) | – | Spaltennamen, Fixtures, Golden-Dateien, Begriffe |
| [1 Export-Engine & Formate](#1-export-engine--formate) | OUT-001 – OUT-016 | CSV, XLSX, XLS, Google Sheets, Dateinamen, Limits, Streaming, Segment-Filter, leere Exporte |
| [2 Tab- und Detail-Exporte](#2-tab--und-detail-exporte) | OUT-020 – OUT-027 | Export aktueller Tab/Filter, untere Detail-Tabs (inkl. Lighthouse, Accessibility, Header, Cookies, Konsole, N-grams), rechte Panels (inkl. Spelling & Grammar, Semantic Search), Mehrfachauswahl, Crawl-Pfad |
| [3 Bulk-Exporte](#3-bulk-exporte) | OUT-030 – OUT-075 | Queued URLs, Links, Web, Path Type, filterbasierte Exporte je Tab, All Error Inlinks, Issues, URL Inspection, KI, Multi-Export |
| [4 Reports](#4-reports) | OUT-080 – OUT-119 | Crawl Overview, Redirects, Canonicals, Pagination, Hreflang, Insecure Content, SERP Summary, Orphan Pages, Structured Data, PageSpeed, JavaScript, Mobile, Accessibility, HTTP-Header, Cookies |
| [5 XML- und Bild-Sitemaps](#5-xml--und-bild-sitemaps) | OUT-120 – OUT-134 | Seitenauswahl, lastmod, priority, changefreq, Bilder, hreflang, Aufteilung, Segmente |
| [6 Crawl-Vergleich](#6-crawl-vergleich) | OUT-140 – OUT-153 | Compare-Modus, Added/New/Removed/Missing, Change Detection, URL-Mapping, Site Structure, Auto-Compare |
| [7 Geplante Crawls](#7-geplante-crawls-scheduling) | OUT-160 – OUT-171 | Aufgaben, Intervalle, OS-Integration, Exporte, Historie, Benachrichtigungen, Import/Export |
| [8 Kommandozeile / Headless](#8-kommandozeile--headless) | OUT-180 – OUT-209 | alle Optionen, Exit-Codes, Fehlerverhalten |
| [9 Looker-Studio-Export](#9-looker-studio-export-custom-crawl-overview) | OUT-210 – OUT-214 | Zeitreihen-Zeile je Crawl, Metrikkatalog, Segmente |
| [10 Konfigurationsdateien & Crawl-Dateien](#10-konfigurationsdateien--crawl-dateien) | OUT-220 – OUT-232 | Profile, Standardkonfiguration, Auth-Profile, Einstellungen, Crawl-Verwaltung, Aufbewahrung |
| [11 Logging & Diagnose](#11-logging--diagnose) | OUT-240 – OUT-245 | Logdateien, Level, Rotation, Log-Paket, Nutzungsstatistik |
| [12 Update-Mechanismus](#12-update-mechanismus) | OUT-250 – OUT-255 | Prüfen, Herunterladen, Signatur, Abschalten |
| [13 Plattformanforderungen](#13-plattformanforderungen) | OUT-260 – OUT-272 | macOS arm64/x64, Windows x64, Installer, Signatur, Pfade, Ressourcen, Zertifikate |
| [14 Benachrichtigungen & Automatisierungs-Schnittstellen](#14-benachrichtigungen--automatisierungs-schnittstellen) | OUT-280 – OUT-285 | E-Mail-Konten, Abschlussmail, Anhänge, Desktop-Benachrichtigung, MCP-Server |

Prioritäten: **P0** = MVP-Kern, **P1** = für Parität nötig, **P2** = später/Nische.

---

## 0 Konventionen

- **Kanonische Spaltennamen:** Alle Spalten-, Tab-, Filter-, Bulk-Export- und Report-Namen haben einen festen englischen Bezeichner (z. B. `Address`, `Status Code`, `Indexability`). Diese Bezeichner sind die Schlüssel für CLI, Presets und Looker-Export und bleiben sprachunabhängig. Die UI darf übersetzte Anzeigenamen zeigen; Exporte nutzen standardmäßig die kanonischen englischen Namen **(OS-Festlegung,** Option „Spaltenüberschriften in UI-Sprache“ unter Einstellungen → Export). Hintergrund: Die Referenz exportiert in UI-Sprache, wodurch Vorlagen in anderen Sprachen brechen – das vermeiden wir.
- **Spaltenkatalog:** Die vollständigen Spalten der Haupt-Tabs (Internal, External, Response Codes …) definieren die Spec-Dateien der jeweiligen Bereiche. Dieses Dokument definiert die Spalten von Bulk-Exporten und Reports, die nicht 1:1 einer Tab-Ansicht entsprechen.
- **Fixtures:** Jede Testfixture liegt unter `tests/fixtures/out-<name>/` und wird vom `os-testsite`-Server auf `http://127.0.0.1:<port>/` ausgeliefert. Erwartete Exporte liegen als Golden-Dateien unter `tests/golden/out-<name>/`. Platzhalter `{BASE}` steht für die Fixture-Basis-URL.
- **Vergleich von Golden-Dateien:** Zeitstempel, Antwortzeiten und Crawl-IDs werden vor dem Vergleich normalisiert (durch `<TS>`, `<MS>`, `<ID>` ersetzt).
- **Zahlenformat in Exporten:** Dezimalpunkt `.`, kein Tausendertrennzeichen, Prozentwerte als Zahl 0–100 mit max. 2 Nachkommastellen (CSV) bzw. als numerische Zelle (XLSX). Datum/Zeit ISO 8601 mit Zeitzonenoffset (`2026-10-13T09:35:00+02:00`).
- **Leere Werte:** Fehlende Werte sind leere Zellen, nicht `null`, `-` oder `0`. Eine echte `0` wird als `0` ausgegeben.
- **„Inlinks-Export zu Filter X“** bezeichnet in diesem Dokument immer: alle Links (Hyperlinks und Ressourcen-Referenzen) im Crawl, deren Ziel-URL im Filter X enthalten ist, im Inlink-Spaltenformat aus OUT-031.

---

## 1 Export-Engine & Formate

### OUT-001 Gemeinsame Export-Pipeline
- **Priorität:** P0
- **Beschreibung:** Alle tabellarischen Exporte (Tab, Detail-Tab, Bulk, Report, Multi-Export, CLI, Scheduling) laufen über eine gemeinsame Pipeline: Datenquelle (Abfrage auf `os-store`) → Zeilen-Stream → Formatschreiber (CSV/XLSX/XLS/Google Sheets). UI, CLI und Scheduling liefern für dieselbe Auswahl byte-identische Dateien (abgesehen von Zeitstempeln im Dateinamen).
- **Eingaben/Konfiguration:** Exportziel (Datei/Ordner/Google Drive), Format, optional Segment, optional Spaltenauswahl.
- **Ausgabe (Spalten/Filter/Export):** Eine Tabelle mit Kopfzeile (kanonische Spaltennamen) und einer Zeile pro Datensatz.
- **Akzeptanzkriterien:**
  - [ ] Ein Export von „Internal:All“ über die UI und über `openspider --load-crawl <id> --export-tabs "Internal:All"` ergibt nach Normalisierung identische CSV-Dateien.
  - [ ] Die Kopfzeile ist immer vorhanden, auch wenn keine Datenzeilen existieren (außer OUT-012 „Leere Exporte überspringen“ greift).
  - [ ] Die Pipeline arbeitet streamend: Ein Export mit 1 000 000 Zeilen und 60 Spalten erhöht den Speicherbedarf des Prozesses um höchstens 300 MB gegenüber dem Leerlauf (Messung in CI mit synthetischem Crawl).
  - [ ] Sortierung im Export entspricht der Standardsortierung der Datenquelle (für Tab-Exporte: aktuelle UI-Sortierung, siehe OUT-020; sonst Crawl-Reihenfolge = aufsteigende interne URL-ID).
- **Testfixture:** `out-basic-site`: 12 HTML-Seiten, 2 Bilder, 1 CSS, 1 JS, ein 404, ein 301, ein externer Link; Golden-Exporte für Internal:All in allen Formaten.

### OUT-002 CSV-Format
- **Priorität:** P0
- **Beschreibung:** CSV nach RFC 4180.
- **Eingaben/Konfiguration:** Einstellungen → Export: „UTF-8-BOM schreiben“ (Standard: an), „Trennzeichen“ (Komma Standard; Semikolon und Tab wählbar, P2), „Zeilenende“ (CRLF Standard **(OS-Festlegung)**).
- **Ausgabe:** Datei `*.csv`, Kodierung UTF-8.
- **Akzeptanzkriterien:**
  - [ ] Felder, die Trennzeichen, `"`, CR oder LF enthalten, werden in `"` eingeschlossen; enthaltene `"` werden verdoppelt.
  - [ ] Zeilenumbrüche innerhalb eines Feldes (z. B. mehrzeiliger Title) bleiben erhalten und brechen das Parsen mit einem RFC-konformen Parser nicht.
  - [ ] Mit aktivem BOM beginnt die Datei mit `EF BB BF`; ohne BOM nicht.
  - [ ] Nicht-ASCII-Zeichen (z. B. `ü`, `日本`, Emoji) werden unverändert UTF-8-kodiert ausgegeben.
  - [ ] Zahlen werden ohne Anführungszeichen und mit Dezimalpunkt geschrieben, unabhängig von der Systemsprache (Test unter `LANG=de_DE`).
  - [ ] Ungültige UTF-8-Sequenzen aus Quelldaten werden durch U+FFFD ersetzt; der Export bricht nicht ab.
- **Testfixture:** `out-csv-edge`: Seiten mit Title `a,b`, Title mit `"Zitat"`, Title mit Zeilenumbruch (`<title>Zeile1\nZeile2</title>`), Title `日本語 🕷`, Seite in ISO-8859-1 deklariert mit ungültigen Bytes.

### OUT-003 Excel-Workbook (XLSX)
- **Priorität:** P0
- **Beschreibung:** Export als Office-Open-XML-Arbeitsmappe.
- **Eingaben/Konfiguration:** Format „Excel Workbook (.xlsx)“.
- **Ausgabe:** Datei `*.xlsx`, ein Arbeitsblatt je Export (bzw. mehrere bei Konsolidierung, OUT-007).
- **Akzeptanzkriterien:**
  - [ ] Numerische Spalten (Status Code, Größen, Längen, Pixelbreiten, Antwortzeiten, Zählwerte) sind als Zahlenzellen gespeichert, Texte als Inline-/Shared-Strings, Datumswerte als Datumszellen mit Format `yyyy-mm-dd hh:mm:ss`.
  - [ ] Kein Zellwert wird je als Formel geschrieben; ein Title `=SUM(A1)` erscheint in Excel als Text `=SUM(A1)`.
  - [ ] Erste Zeile ist fixiert und hat einen Autofilter über alle Spalten **(OS-Festlegung)**.
  - [ ] Texte über 32 767 Zeichen werden auf 32 767 Zeichen gekürzt; die letzten Zeichen werden durch `…[truncated]` ersetzt und ein Hinweis im Export-Log vermerkt.
  - [ ] Mehr als 1 048 575 Datenzeilen: Fortsetzung auf weiteren Blättern `<Name> (2)`, `<Name> (3)` … jeweils mit Kopfzeile; Warnung an den Nutzer.
  - [ ] Blattnamen sind max. 31 Zeichen lang, enthalten keine der Zeichen `[]:*?/\` und sind innerhalb der Datei eindeutig (Suffix ` (2)` bei Kollision).
  - [ ] Datei öffnet ohne Reparaturdialog in Excel (Win/Mac), LibreOffice und Numbers (manueller Test, einmal pro Release).
- **Testfixture:** `out-csv-edge` (wie OUT-002) + synthetischer Crawl mit 1,1 Mio. URLs (generiert, nicht gecrawlt) für den Zeilenüberlauf.

### OUT-004 Excel 97–2004 (XLS)
- **Priorität:** P2
- **Beschreibung:** Legacy-Binärformat für alte Excel-Versionen.
- **Eingaben/Konfiguration:** Format „Excel 97–2004 (.xls)“; CLI `--export-format xls`.
- **Ausgabe:** Datei `*.xls` (BIFF8).
- **Akzeptanzkriterien:**
  - [ ] Max. 65 535 Datenzeilen und 256 Spalten pro Blatt; Überlauf wie bei OUT-003 auf Folgeblätter, Spalten über 256 werden abgeschnitten und gewarnt.
  - [ ] Texte max. 32 767 Zeichen.
  - [ ] Ist kein lizenzkompatibler BIFF8-Schreiber verfügbar, darf das Format in der UI ausgeblendet sein; die CLI meldet dann `export format 'xls' not supported` mit Exit-Code 2.
- **Testfixture:** `out-basic-site`.

### OUT-005 Google-Sheets-Export
- **Priorität:** P1
- **Beschreibung:** Export direkt in eine Google-Tabelle im Google Drive des Nutzers.
- **Eingaben/Konfiguration:** Google-Konto (OAuth über System-Browser mit PKCE, Scope nur `drive.file`); Ordnerstruktur in Drive: `OpenSpider/<Projektname>/<Aufgaben- bzw. Crawlname>/` **(OS-Festlegung;** Referenz nutzt einen festen Stammordner mit Projekt-/Crawl-Unterordnern).
- **Ausgabe:** Eine Google-Tabelle pro Export (bzw. eine mit mehreren Blättern bei Konsolidierung); Rückgabe des Links an UI/CLI-Log.
- **Akzeptanzkriterien:**
  - [ ] Werte werden im Modus „roh“ geschrieben (keine Interpretation als Formel, keine automatische Datumserkennung); `=1+1` bleibt Text.
  - [ ] Zellenlimit: Bevor geschrieben wird, wird `Zeilen × Spalten` gegen das konfigurierbare Limit geprüft (Standard 10 000 000 Zellen, entspricht dem aktuellen Google-Limit; Referenz dokumentiert 5 Mio.). Überschreitung → Kürzung auf die maximal mögliche Zeilenzahl, deutliche Warnung in UI/Log mit Angabe der abgeschnittenen Zeilen, Empfehlung CSV/XLSX.
  - [ ] Schreibvorgänge erfolgen in Blöcken (≤ 10 000 Zeilen je Request); bei Timeout/HTTP 429/5xx bis zu 5 Wiederholungen mit exponentiellem Backoff (1 s, 2 s, 4 s, 8 s, 16 s).
  - [ ] „Überschreiben“: existiert die Tabelle gleichen Namens im Zielordner, wird ihr Inhalt ersetzt (ID bleibt gleich, damit Verknüpfungen erhalten bleiben). Wurde sie zwischenzeitlich vom Nutzer gelöscht oder in den Papierkorb gelegt, wird sie neu angelegt statt abzubrechen.
  - [ ] Abgelaufenes/entzogenes Token → Exporte nach Drive schlagen mit klarer Meldung „Google-Konto erneut verbinden“ fehl; lokale Exporte desselben Laufs werden trotzdem geschrieben.
  - [ ] Tokens werden im OS-Schlüsselbund (macOS Keychain / Windows Credential Manager) gespeichert, nie in Konfigurationsdateien.
- **Testfixture:** Mock-Server für die Sheets-/Drive-API im `os-testsite` (Endpunkte simulieren 429, Timeout, gelöschte Datei); `out-basic-site` als Datenquelle.

### OUT-006 Exportdialog und Formatwahl
- **Priorität:** P0
- **Beschreibung:** Jeder Export in der UI öffnet einen Speichern-Dialog mit Formatwahl; das zuletzt gewählte Format und Verzeichnis werden pro Nutzer gemerkt.
- **Eingaben/Konfiguration:** Format: CSV, Excel Workbook, Excel 97–2004 (falls verfügbar), Google Sheets (falls Konto verbunden).
- **Ausgabe:** Datei bzw. Google-Tabelle.
- **Akzeptanzkriterien:**
  - [ ] Vorgeschlagener Dateiname gemäß OUT-008.
  - [ ] Nach erfolgreichem Export bietet eine Meldung „Ordner öffnen“ bzw. „In Google Sheets öffnen“ an.
  - [ ] Existiert die Zieldatei, fragt das Betriebssystem-Dialogfenster nach Überschreiben; bei Ablehnung wird nichts geschrieben.
  - [ ] Format und Verzeichnis des letzten Exports werden beim nächsten Export vorbelegt (auch nach Neustart).
- **Testfixture:** UI-E2E-Test mit `out-basic-site`.

### OUT-007 Konsolidierung mehrerer Exporte in einer Arbeitsmappe
- **Priorität:** P1
- **Beschreibung:** Werden mehrere Exporte gleichzeitig erzeugt (Multi-Export, Scheduling, CLI), können sie statt in Einzeldateien als Blätter einer einzigen XLSX-Datei bzw. Google-Tabelle abgelegt werden.
- **Eingaben/Konfiguration:** Option „In einer Arbeitsmappe zusammenfassen“ (Standard aus); CLI `--single-workbook` **(OS-Festlegung,** Flagname).
- **Ausgabe:** Eine Datei `<crawlname>_export.xlsx` bzw. eine Google-Tabelle mit je einem Blatt pro Export; Blattreihenfolge = Auswahlreihenfolge.
- **Akzeptanzkriterien:**
  - [ ] Blattnamen werden aus „Tab Filter“ bzw. „Bulk-Export-Name“ gebildet und nach den Regeln aus OUT-003 gekürzt/eindeutig gemacht.
  - [ ] Bei CSV ist die Option deaktiviert (ausgegraut) bzw. wird in der CLI mit Warnung ignoriert.
  - [ ] Exporte, die keine Tabellen sind (Screenshots, Quelltext, Sitemaps), werden nicht in die Arbeitsmappe aufgenommen, sondern weiterhin als Dateien geschrieben.
- **Testfixture:** `out-basic-site`, Multi-Export von Internal:All, Response Codes:All, Links:All Inlinks.

### OUT-008 Dateinamen-Konvention
- **Priorität:** P0
- **Beschreibung:** Einheitliche, vorhersagbare Dateinamen, die CLI-Nutzer per Skript weiterverarbeiten können.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Dateiname = kanonischer Name in Kleinbuchstaben, Leer- und Sonderzeichen durch `_` ersetzt, Bestandteile mit `_` verbunden. Beispiele **(OS-Festlegung)**: `internal_all.csv`, `response_codes_client_error_(4xx).csv` → normalisiert `response_codes_client_error_4xx.csv`, `all_inlinks.csv`, `redirect_chains.csv`, `crawl_overview.csv`.
- **Akzeptanzkriterien:**
  - [ ] Erlaubte Zeichen im Dateinamen: `a-z 0-9 _ - .`; mehrere `_` hintereinander werden zu einem zusammengefasst; führende/abschließende `_` entfallen.
  - [ ] Dateinamen sind ≤ 120 Zeichen (Kürzung vor der Erweiterung) und auf Windows gültig (keine reservierten Namen wie `con`, `nul`).
  - [ ] Bei Segment-Exporten wird `_segment_<segmentname>` angehängt.
  - [ ] Die Zuordnung kanonischer Name → Dateiname ist in einer Tabelle in `os-export` hinterlegt und per `openspider --help export-tabs --verbose` einsehbar.
- **Testfixture:** Unit-Test über alle registrierten Exportnamen (keine Kollision zweier Exporte auf denselben Dateinamen).

### OUT-009 Spaltenauswahl und -reihenfolge
- **Priorität:** P1
- **Beschreibung:** Tab-Exporte übernehmen die in der UI eingestellte Spaltenauswahl und -reihenfolge (sichtbare Spalten in sichtbarer Reihenfolge). Ein „Spalten-Konfigurator“ erlaubt, Spalten in Masse ein-/auszublenden und neu zu ordnen; „Spalten zurücksetzen“ stellt den Standard wieder her.
- **Eingaben/Konfiguration:** Spaltenkonfiguration pro Tab (wird mit dem Konfigurationsprofil gespeichert, OUT-220).
- **Ausgabe:** Export-Spalten = sichtbare Spalten.
- **Akzeptanzkriterien:**
  - [ ] Wird in „Internal“ die Spalte `Meta Keywords 1` ausgeblendet und `Title 1` vor `Status Code` verschoben, enthält der Export diese Reihenfolge und keine Meta-Keywords-Spalte.
  - [ ] CLI/Scheduling verwenden die Spaltenkonfiguration aus der übergebenen Konfigurationsdatei; ohne Konfigurationsdatei die Standardspalten.
  - [ ] Bulk-Exporte und Reports haben feste Spalten (nicht konfigurierbar) **(OS-Festlegung)**.
  - [ ] `Address` bleibt immer erste Spalte und kann nicht ausgeblendet werden.
- **Testfixture:** `out-basic-site` mit gespeicherter Spaltenkonfiguration `tests/fixtures/out-basic-site/columns.toml`.

### OUT-010 Exportfortschritt und Abbruch
- **Priorität:** P1
- **Beschreibung:** Lang laufende Exporte zeigen Fortschritt und sind abbrechbar, ohne die UI zu blockieren.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Fortschrittsanzeige (Zeilen geschrieben / gesamt, Prozent).
- **Akzeptanzkriterien:**
  - [ ] Die UI bleibt während eines Exports bedienbar (Tabellen scrollbar).
  - [ ] „Abbrechen“ stoppt innerhalb von 2 s; die unvollständige Datei wird gelöscht (lokal) bzw. die halbfertige Google-Tabelle bleibt mit Hinweisblatt „unvollständig“ bestehen.
  - [ ] Ein Export während eines laufenden Crawls exportiert den Stand zum Startzeitpunkt des Exports (konsistenter Snapshot über SQLite-Lesetransaktion).
- **Testfixture:** synthetischer Crawl mit 500 000 URLs.

### OUT-011 Segment-Einschränkung von Exporten
- **Priorität:** P1
- **Beschreibung:** Ist ein Segment aktiv (rechts „Segments“ bzw. Segment-Auswahl), beziehen sich Tab-Exporte, Bulk-Exporte und Reports nur auf URLs dieses Segments. In Scheduling/CLI kann pro Export ein Segment gewählt werden oder „alle Segmente einzeln“.
- **Eingaben/Konfiguration:** Segmentauswahl; CLI `--segment <name>` bzw. `--segment all` **(OS-Festlegung,** Flagname).
- **Ausgabe:** gefilterte Exporte, Dateiname mit Segmentsuffix (OUT-008).
- **Akzeptanzkriterien:**
  - [ ] Bei Inlink-Exporten gilt das Segment für die **Ziel**-URL (Segment „/blog/“ → alle Links, die auf Blog-URLs zeigen) **(OS-Festlegung)**.
  - [ ] Mit `--segment all` entsteht pro konfiguriertem Segment eine Datei sowie eine ungefilterte Datei.
  - [ ] Bulk-Exporte funktionieren auch bei aktivem Segment (bekannter Fehler der Referenz, der hier nicht auftreten darf).
- **Testfixture:** `out-segments`: Seiten unter `/blog/`, `/shop/`, `/info/`; Segmentdefinitionen „Blog“ (Pfad enthält `/blog/`), „Shop“.

### OUT-012 Leere Exporte überspringen
- **Priorität:** P1
- **Beschreibung:** Option, Exporte ohne Datenzeilen gar nicht erst zu schreiben (Multi-Export, Scheduling, CLI). Leere Exporte werden dann auch nicht an E-Mails angehängt.
- **Eingaben/Konfiguration:** Checkbox „Leere Exporte überspringen“ (Standard aus); CLI `--skip-empty-exports` **(OS-Festlegung,** Flagname).
- **Ausgabe:** nur nicht-leere Dateien.
- **Akzeptanzkriterien:**
  - [ ] Bei aktivierter Option und einem Crawl ohne 4xx-URLs wird `response_codes_client_error_4xx.csv` nicht erzeugt; ohne Option wird eine Datei nur mit Kopfzeile erzeugt.
  - [ ] Die Zusammenfassung (Log/CLI-Ausgabe) listet übersprungene Exporte namentlich.
- **Testfixture:** `out-basic-site` ohne 5xx-Seite.

### OUT-013 Ausgabeordner: Überschreiben vs. Zeitstempel-Ordner
- **Priorität:** P0
- **Beschreibung:** Für automatisierte Exporte (CLI, Scheduling) wird festgelegt, ob Dateien im Zielordner überschrieben werden oder je Lauf ein eigener Unterordner mit Zeitstempel angelegt wird. Gilt analog für Google Drive.
- **Eingaben/Konfiguration:** „Dateien überschreiben“ / „Zeitstempel-Ordner anlegen“.
- **Ausgabe:** Zeitstempel-Ordnername `YYYY-MM-DD_HH-MM-SS` in lokaler Zeit **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Zeitstempel-Modus: zwei Läufe in derselben Minute erzeugen zwei unterschiedliche Ordner; fällt die Sekunde zusammen, wird `_2` angehängt.
  - [ ] Überschreiben-Modus: vorhandene gleichnamige Dateien werden ersetzt; andere Dateien im Ordner bleiben unangetastet.
  - [ ] Keiner der beiden Modi und Zielordner nicht leer → Abbruch **vor** dem Crawl mit verständlicher Fehlermeldung (CLI: Exit-Code 2). (Abweichung: Die Referenz scheitert hier still.)
  - [ ] Zielordner existiert nicht → er wird angelegt; ist das nicht möglich (Rechte), Abbruch vor dem Crawl mit Exit-Code 2.
- **Testfixture:** CLI-Integrationstest mit temporären Verzeichnissen.

### OUT-014 Kopieren aus Tabellen in die Zwischenablage
- **Priorität:** P2
- **Beschreibung:** Markierte Zellen/Zeilen jeder Tabelle können als tabulatorgetrennter Text kopiert werden (zum Einfügen in Tabellenkalkulationen).
- **Eingaben/Konfiguration:** Strg/Cmd+C, Kontextmenü „Kopieren“, „URL kopieren“.
- **Ausgabe:** TSV in der Zwischenablage, optional mit Kopfzeile („Mit Spaltenköpfen kopieren“).
- **Akzeptanzkriterien:**
  - [ ] Drei markierte Zeilen ergeben drei TSV-Zeilen; Tabs/Zeilenumbrüche in Zellen werden durch Leerzeichen ersetzt.
  - [ ] Kopieren von mehr als 100 000 Zeilen fragt vorher nach.
- **Testfixture:** UI-Test mit `out-basic-site`.

### OUT-015 Exporte in Listenmodus-Reihenfolge
- **Priorität:** P1
- **Beschreibung:** Im Listenmodus kann der Export die URLs exakt in der Reihenfolge der hochgeladenen Liste ausgeben – einschließlich Duplikaten und nachträglich korrigierter URLs (z. B. ergänztes Schema).
- **Eingaben/Konfiguration:** automatisch im Listenmodus für Tab-Exporte des „Internal“/„All“-Bereichs; Option im Exportdialog „Upload-Reihenfolge beibehalten“ (Standard an).
- **Ausgabe:** Zusätzliche erste Spalte `Original Url` (wie hochgeladen) vor `Address` **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Liste `B, A, B, example.com/c` → Export enthält 4 Zeilen in dieser Reihenfolge; Zeile 4 hat `Original Url` = `example.com/c`, `Address` = `http://example.com/c` (bzw. die normalisierte Form).
  - [ ] Nicht gecrawlte Listeneinträge (ungültige URL, durch Exclude ausgeschlossen) erscheinen mit leerer `Address` und `Status` = `Invalid`/`Excluded`.
  - [ ] Duplikate werden nur einmal gecrawlt, aber mehrfach exportiert.
- **Testfixture:** `out-list-order`: Listendatei `list.txt` mit Duplikaten, einer URL ohne Schema, einer ungültigen Zeile und einer ausgeschlossenen URL.

### OUT-016 Formel- und Injection-Schutz für CSV
- **Priorität:** P2
- **Beschreibung:** Optionaler Schutz gegen CSV-Injection, da gecrawlte Inhalte (Titles, Anker) fremdkontrolliert sind.
- **Eingaben/Konfiguration:** Einstellungen → Export: „Zellen, die mit `= + - @` beginnen, in CSV mit `'` maskieren“ (Standard aus, um Daten unverändert zu lassen) **(OS-Festlegung)**.
- **Ausgabe:** maskierte Zellen.
- **Akzeptanzkriterien:**
  - [ ] Mit Option: Title `=HYPERLINK("x")` wird als `'=HYPERLINK(""x"")` geschrieben; Zahlen wie `-5` (numerische Spalten) werden nicht maskiert.
  - [ ] Ohne Option bleibt der Wert unverändert.
- **Testfixture:** `out-csv-edge` (Seite mit Title `=HYPERLINK("http://evil")`).

---

## 2 Tab- und Detail-Exporte

### OUT-020 Export des aktuellen Haupt-Tabs inkl. Filter
- **Priorität:** P0
- **Beschreibung:** Die Schaltfläche „Export“ über der Haupttabelle exportiert genau die aktuell sichtbare Ansicht: gewählter Tab, gewählter Filter, aktive Suche (inkl. Regex/Spaltensuche), aktive Segment-Einschränkung und aktuelle Sortierung.
- **Eingaben/Konfiguration:** aktueller UI-Zustand.
- **Ausgabe:** Datei mit sichtbaren Spalten (OUT-009), Zeilen in sichtbarer Reihenfolge.
- **Akzeptanzkriterien:**
  - [ ] Tab „Response Codes“, Filter „Client Error (4xx)“ → Export enthält genau die Zeilen dieses Filters (Anzahl = Zähler in der Filterliste).
  - [ ] Mit Suchbegriff `/blog/` in der Suchleiste enthält der Export nur passende Zeilen.
  - [ ] Nach Sortierung nach `Word Count` absteigend ist die erste Exportzeile die mit der höchsten Wortzahl.
  - [ ] Für jeden Tab und jeden Filter aller Spec-Bereiche existiert ein Export (automatisierter Test iteriert über die Filter-Registry und prüft, dass ein Export ohne Fehler entsteht).
  - [ ] Während eines laufenden Crawls ist der Export möglich (Snapshot, OUT-010).
- **Testfixture:** `out-basic-site`.

### OUT-021 Export der unteren Detail-Tabs
- **Priorität:** P0 (Inlinks, Outlinks), P1 (übrige)
- **Beschreibung:** Für die in der Haupttabelle markierte(n) URL(s) lassen sich die Detailtabellen des unteren Fensters exportieren: URL Details, Inlinks, Outlinks, Image Details, Resources, Duplicate Details, Structured Data Details, Spelling & Grammar Details, Lighthouse Details, Accessibility Details, HTTP Headers, Cookies, Chrome Console Log, N-grams. Zugriff über die Export-Schaltfläche des unteren Fensters oder Rechtsklick → Export → <Detailtab>. Detail-Tabs, deren Daten für die gewählte URL nicht erhoben wurden (z. B. Lighthouse ohne PSI/Lighthouse-Integration), sind im Export-Menü ausgegraut; der Tooltip nennt die nötige Einstellung.
- **Eingaben/Konfiguration:** Auswahl in der Haupttabelle; Filter des Detail-Tabs (z. B. Inlinks-Filter „Auto/Current/Previous“ im Vergleichsmodus; N-grams-Auswahl n = 1…6 und Inhaltsbereich).
- **Ausgabe:** Spalten wie im jeweiligen Detail-Tab; Inlinks/Outlinks im Format aus OUT-031. Kanonische Exportpfade **(OS-Festlegung)**: `Detail:<Detailtab>` (z. B. `Detail:Lighthouse Details`), damit Detail-Exporte auch per CLI für eine URL-Liste (`--detail-export "<Detailtab>" --urls <datei>`, P2) ansprechbar sind.
  - Lighthouse Details: Langformat `Address`, `Category` (Performance/Accessibility/Best Practices/SEO), `Audit ID`, `Audit Title`, `Score` (0–1, leer bei informativen Audits), `Display Value`, `Numeric Value`, `Numeric Unit`, `Savings (ms)`, `Savings (Bytes)`, `Item` (je Detailzeile eines Audits, z. B. Ressourcen-URL), `Item Value`.
  - Accessibility Details: zwei Teiltabellen, im XLSX als zwei Blätter, im CSV als zwei Dateien `…_violations.csv` und `…_locations.csv` **(OS-Festlegung)**: (a) Violations: `Address`, `Issue`, `Rule ID`, `Guidelines`, `User Impact`, `Priority`, `Occurrences`; (b) Location: `Address`, `Rule ID`, `Location` (CSS-Selektor), `HTML Snippet`, `Failure Summary`.
  - HTTP Headers: `Address`, `Direction` (`Request`/`Response`), `Header Name`, `Header Value` (Redaktion wie OUT-048).
  - Cookies: Spalten wie OUT-049.
  - Chrome Console Log: `Address`, `Level`, `Message`, `Source`, `Line`, `Column`, `Timestamp`.
  - N-grams: `N-gram`, `n`, `Count`, `Density %` (= Count × n / Wortanzahl × 100), `In Title` (True/False), `In H1`, `In H2`, `In Meta Description`.
- **Akzeptanzkriterien:**
  - [ ] Inlinks-Export einer URL mit 3 eingehenden Verweisen (2× Hyperlink, 1× Canonical) liefert 3 Zeilen mit `Type` = `Hyperlink`, `Hyperlink`, `Canonical`.
  - [ ] URL Details: zwei Spalten `Name`, `Value` mit allen Eigenschaften der URL; Anzahl Zeilen = Anzahl Spalten der Haupttabelle „Internal:All“ (alle, auch ausgeblendete).
  - [ ] Structured Data Details: eine Zeile pro Eigenschaft mit Spalten `Subject`, `Predicate`, `Object`, `Format` (JSON-LD/Microdata/RDFa), `Validation Type`, `Severity`, `Issue`.
  - [ ] Duplicate Details: Spalten `Address`, `Near Duplicate Address`, `Similarity` (Prozent).
  - [ ] Spelling & Grammar Details: `Error Type` (Spelling/Grammar), `Error`, `Suggestion`, `Section` (Title/Meta Description/Content …), `Context`, `Count`.
  - [ ] Lighthouse Details: Mock-PSI-Antwort mit 3 Audits, davon eines mit 4 Detailzeilen → 6 Zeilen (1 + 1 + 4); `Score` des informativen Audits ist leer, nicht 0.
  - [ ] Accessibility Details: Seite mit 2 Bildern ohne alt → Violations-Tabelle 1 Zeile `image-alt` mit `Occurrences`=2; Location-Tabelle 2 Zeilen mit unterschiedlichen Selektoren.
  - [ ] HTTP Headers: Seite mit 2 `Set-Cookie`-Headern → 2 Zeilen `Set-Cookie`, `Direction`=Response; `Authorization` im Request erscheint als `<redacted>`.
  - [ ] Chrome Console Log: Seite mit `console.warn("w")` und `console.error("e")` → 2 Zeilen mit `Level` warning bzw. error; ohne JS-Rendering ist der Export deaktiviert.
  - [ ] N-grams: Text „rot blau rot blau rot“ mit n = 2 → Zeilen `rot blau` (Count 2) und `blau rot` (Count 2); n = 1 → `rot` (3), `blau` (2); Sortierung `Count` absteigend, bei Gleichstand alphabetisch.
- **Testfixture:** `out-detail-tabs`: Seite `/target` mit 2 Hyperlinks und einem Canonical darauf, JSON-LD-Block, Rechtschreibfehler „Beispeil“, Nah-Duplikat `/target-copy`, Seite `/a11y` mit 2 Bildern ohne alt, Seite `/console` mit Konsolenmeldungen, Seite `/ngram` mit Text „rot blau rot blau rot“, Header-Seite mit 2 `Set-Cookie`; Mock-PSI-Antwort.

### OUT-022 Mehrfachauswahl-Export der Detail-Tabs
- **Priorität:** P1
- **Beschreibung:** Werden in der Haupttabelle mehrere URLs (Strg/Cmd- oder Umschalt-Klick, „Alle markieren“) gewählt, exportiert der Detail-Export die Daten aller gewählten URLs in einer Datei.
- **Eingaben/Konfiguration:** Mehrfachauswahl.
- **Ausgabe:** Wie OUT-021; bei Tabellen ohne natürliche Quell-/Zielspalte wird eine erste Spalte `Address` (die gewählte URL) ergänzt.
- **Akzeptanzkriterien:**
  - [ ] Auswahl von 3 URLs mit je 2 Inlinks → 6 Zeilen.
  - [ ] Auswahl von 50 000 URLs funktioniert ohne UI-Blockade (streamend).
- **Testfixture:** `out-detail-tabs`.

### OUT-023 Crawl-Path-Report einer URL
- **Priorität:** P1
- **Beschreibung:** Für eine gewählte URL wird der kürzeste Pfad ab Start-URL ausgegeben, über den der Crawler sie gefunden hat. Erreichbar über Rechtsklick → Export → Crawl Path Report (nicht im Reports-Menü).
- **Eingaben/Konfiguration:** gewählte URL(s).
- **Ausgabe:** Spalten `Target Address` (die gewählte URL, wichtig bei Mehrfachauswahl), `Step` (0 = Zielseite), `Address`, `Status Code`, `Link Type` (Typ des Verweises, über den `Address` vom nächsthöheren Schritt aus erreicht wurde: Werte wie `Type` in OUT-031), `Anchor`, `Crawl Depth`. Reihenfolge: Zielseite zuerst, Start-URL zuletzt (von unten nach oben lesen). `Link Type`/`Anchor` der letzten Zeile (Start-URL bzw. Sitemap) sind leer.
- **Akzeptanzkriterien:**
  - [ ] Für `/a/b/c` (Start → `/a` → `/a/b` → `/a/b/c`) entstehen 4 Zeilen, `Step` 0–3, letzte Zeile = Start-URL; es gilt immer: Anzahl Zeilen − 1 = `Crawl Depth` der Zielseite.
  - [ ] Weiterleitungen sind eigene Schritte: Start → `/r` (301) → `/z` ergibt 3 Zeilen, die Zeile `/z` hat `Link Type` = `HTTP Redirect`, `/r` hat `Status Code` 301.
  - [ ] Mehrere gleich kurze Pfade: verwendet wird der Pfad über den zuerst entdeckten Verweis (Crawl-Reihenfolge); dasselbe Ergebnis bei wiederholtem Export (deterministisch) **(OS-Festlegung)**.
  - [ ] Für eine URL, die nur aus der XML-Sitemap stammt (Orphan), endet der Pfad mit der Sitemap-URL und `Link Type` = `XML Sitemap`.
  - [ ] Für eine URL, die nur über GA/GSC bekannt ist (nicht gecrawlt), ist der Report leer (nur Kopfzeile) mit Hinweis „nicht über Links erreicht“.
  - [ ] Im Listenmodus: Pfad besteht nur aus der Listen-URL selbst und ggf. Weiterleitungen.
  - [ ] Mehrfachauswahl von 3 URLs → eine Datei, Blöcke je `Target Address` in Auswahlreihenfolge.
- **Testfixture:** `out-crawl-path`: lineare Linkkette über 3 Ebenen, eine 301-Zwischenstation, zwei gleich lange Pfade zu `/diamond` (über `/l` und `/r`, `/l` zuerst verlinkt) + Seite nur in `sitemap.xml`.

### OUT-024 Exporte im unteren Fenster: SERP Snippet, Rendered Page, View Source
- **Priorität:** P2
- **Beschreibung:** Speichern einzelner Inhalte des unteren Fensters: gerenderter Screenshot (PNG), gespeichertes HTML (Original/gerendert) als `.html`, HTTP-Header als `.txt`.
- **Eingaben/Konfiguration:** gewählte URL; die Daten müssen gespeichert worden sein (HTML speichern, Rendering, Header-Extraktion).
- **Ausgabe:** einzelne Dateien.
- **Akzeptanzkriterien:**
  - [ ] Speichern ist deaktiviert, wenn die Daten nicht vorhanden sind (Tooltip nennt die nötige Einstellung).
  - [ ] Gespeichertes HTML ist byte-identisch mit dem empfangenen Body (nach Dekompression).
- **Testfixture:** `out-detail-tabs` mit aktivierter HTML-Speicherung.

### OUT-025 Export der rechten Übersichtsfenster
- **Priorität:** P1
- **Beschreibung:** Die rechten Panels (Overview, Issues, Site Structure, Segments, Response Times, Spelling & Grammar, Semantic Search, API) sind exportierbar.
- **Eingaben/Konfiguration:** Export-Schaltfläche im jeweiligen Panel.
- **Ausgabe:**
  - Overview: identisch mit OUT-080 Crawl Overview.
  - Issues: Spalten `Issue Name`, `Issue Type` (Issue/Warning/Opportunity), `Issue Priority` (High/Medium/Low), `URLs`, `% of Total`, `Description`, `How To Fix`. Die Texte in `Description` und `How To Fix` sind eigenständig verfasste OpenSpider-Texte (gepflegt in `os-analyze/issues/*.toml`, de/en); Issue-Beschreibungen der Referenz werden weder übernommen noch paraphrasierend nachgebaut (Clean-Room-Regel).
  - Site Structure: zwei Tabellen (XLSX: zwei Blätter; CSV: `site_structure_directories.csv` und `site_structure_depth.csv`) **(OS-Festlegung** der Aufteilung):
    - Verzeichnisse: `Path` (Verzeichnis), `URLs` (gesamt), `Indexable`, `Non-Indexable`.
    - Tiefe: `Crawl Depth` (Zeilen `1` … `10`, `10+`), je Antwortcode-Klasse eine Spalte (`2xx`, `3xx`, `4xx`, `5xx`, `No Response`, `Blocked`) und `Total`. Die Referenz zeigt Tiefen 1 bis 10+; OpenSpider ergänzt eine Zeile `0` für die Start-URL(s) **(OS-Festlegung, Abweichung** – beim Paritätsabgleich wird Zeile `0` ausgeblendet).
  - Response Times: Spalten `Response Time Range` (Buckets 0–1 s, 1–2 s, 2–3 s, 3–4 s, 4–5 s, 5–6 s, 6–7 s, 7–8 s, 8–9 s, 9–10 s, über 10 s), `URLs`, `% of Total`.
  - Segments: je Segment eine Zeile mit `Segment`, `URLs`, `Indexable`, `Non-Indexable`, `Issues`, `Warnings`, `Opportunities`.
  - Spelling & Grammar: die häufigsten eindeutigen Fehler (max. 100, wie im Panel) mit `Error`, `Error Type` (Spelling/Grammar), `Rule` (nur Grammatik), `Suggestion`, `URLs` (Anzahl betroffener URLs), `Occurrences` **(OS-Festlegung** für `Rule`/`Occurrences`).
  - Semantic Search: Ergebnisliste der zuletzt ausgeführten Abfrage mit `Query`, `Rank`, `Address`, `Similarity` (Kosinus, 0–1, 4 Nachkommastellen), `Embedding Model`; bei Anzeige „Centroid“ stattdessen `Address`, `Distance To Centroid`.
- **Akzeptanzkriterien:**
  - [ ] Issues-Export enthält genau die Zeilen, die im Issues-Panel angezeigt werden (gleiche Zähler).
  - [ ] Für jedes registrierte Issue existieren nicht-leere `Description`- und `How To Fix`-Texte in Deutsch und Englisch (Registry-Test).
  - [ ] Response-Times-Buckets sind halboffene Intervalle `[a, b)`; eine Antwortzeit von exakt 1,000 s fällt in 1–2 s.
  - [ ] Site Structure Tiefe: Fixture mit Start-URL, 2 Seiten in Tiefe 1 (eine davon 404) und 11 Seiten in Tiefe 11 → Zeile `1`: `2xx`=1, `4xx`=1; Zeile `10+`: `2xx`=11; Zeile `0`: 1. Summe `Total` über alle Zeilen = Anzahl interner URLs mit bekannter Tiefe.
  - [ ] Spelling & Grammar: Fehler „Beispeil“ auf 3 Seiten (insgesamt 5-mal) → eine Zeile `URLs`=3, `Occurrences`=5; bei mehr als 100 eindeutigen Fehlern enthält der Export genau 100 Zeilen, sortiert nach `URLs` absteigend, dann `Error` alphabetisch.
  - [ ] Semantic Search: Export ohne vorherige Abfrage ist deaktiviert; mit Mock-Embeddings und Abfrage „rote Schuhe“ stimmen Reihenfolge und Werte mit der Panel-Anzeige überein.
- **Testfixture:** `out-basic-site` mit Fixture-Server-Option `delay_ms` je Seite (100, 1000, 2500, 11000 ms); `out-site-structure` (Tiefenkette bis 11, eine 404 in Tiefe 1); `out-detail-tabs` (Rechtschreibfehler); Mock-Embedding-Endpunkt mit festen Vektoren.

### OUT-026 Export der Visualisierungen
- **Priorität:** P2
- **Beschreibung:** Crawl-Baum, Verzeichnisbaum, Kraftdiagramme und Inhalts-Cluster lassen sich als SVG/PNG und als HTML-Datei (öffnet im externen Browser) speichern.
- **Eingaben/Konfiguration:** Visualisierungsfenster → Speichern.
- **Ausgabe:** `.svg`, `.png` (Auflösung wählbar 1×/2×), `.html` (eigenständig, ohne Netzwerkzugriff lauffähig).
- **Akzeptanzkriterien:**
  - [ ] HTML-Export enthält Daten und Skripte inline und funktioniert offline.
  - [ ] Visualisierungs-Exporte sind nur lokal speicherbar (nicht nach Google Drive).
- **Testfixture:** `out-basic-site`.

### OUT-027 Export der Suchergebnisse aus „View Source“/HTML-Suche
- **Priorität:** P2
- **Beschreibung:** Treffer einer Suche über gespeichertes HTML können als Liste (`Address`, `Line`, `Match`) exportiert werden.
- **Eingaben/Konfiguration:** Suchbegriff/Regex.
- **Ausgabe:** Tabelle.
- **Akzeptanzkriterien:**
  - [ ] Suche nach `gtag\(` (Regex) auf 3 von 10 Seiten → 3 Adressen mit Zeilennummer.
- **Testfixture:** `out-detail-tabs`.

---

## 3 Bulk-Exporte

Menü „Bulk Export“. Alle Bulk-Exporte beziehen sich auf den gesamten Crawl (bzw. das aktive Segment, OUT-011), unabhängig von der aktuellen Tabellenauswahl. Kanonische Pfade für CLI/Presets: `"<Untermenü>:<Eintrag>"`, verschachtelt mit weiterem `:` (z. B. `"Response Codes:Internal:Client Error (4xx) Inlinks"`).

### OUT-030 Queued URLs
- **Priorität:** P1
- **Beschreibung:** Exportiert alle entdeckten, aber noch nicht gecrawlten URLs (Warteschlange) – typischerweise bei pausiertem oder durch Limits beendetem Crawl.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Spalten `Address`, `Crawl Depth` (geplante Tiefe), `Found On` (erste Quell-URL), `Link Type`.
- **Akzeptanzkriterien:**
  - [ ] Crawl mit Limit „max. 5 URLs“ auf einer Seite mit 20 Links → Export enthält die übrigen entdeckten, nicht gecrawlten internen URLs, keine bereits gecrawlte URL.
  - [ ] Nach vollständigem Crawl ist der Export leer (nur Kopfzeile).
  - [ ] URLs, die durch Exclude/robots.txt nie eingereiht werden, erscheinen nicht.
- **Testfixture:** `out-queue`: Startseite mit 20 internen Links, Crawl-Limit 5.

### OUT-031 Links: All Inlinks (Standard-Linkformat)
- **Priorität:** P0
- **Beschreibung:** Jeder Verweis im Crawl (Hyperlinks und Ressourcen-Referenzen, intern und extern) als eine Zeile. Das Spaltenformat ist das Standard-Linkformat aller Inlink-/Outlink-Exporte dieses Dokuments.
- **Eingaben/Konfiguration:** Welche Linktypen gespeichert werden, steuert die Crawl-Konfiguration (Bilder, CSS, JS, Canonicals usw. „speichern“).
- **Ausgabe (Standard-Linkformat):**
  `Type`, `Source`, `Destination`, `Size (Bytes)`, `Alt Text`, `Anchor`, `Status Code`, `Status`, `Follow`, `Target`, `Rel`, `Path Type`, `Link Path`, `Link Position`, `Link Origin`.
  - `Type` ∈ {`Hyperlink`, `Image`, `CSS`, `JavaScript`, `Media`, `Iframe`, `Canonical`, `HTTP Canonical`, `Rel Next`, `Rel Prev`, `Hreflang`, `HTTP Hreflang`, `AMP`, `Mobile Alternate`, `Meta Refresh`, `HTTP Refresh`, `HTTP Redirect`, `HSTS Policy`, `JavaScript Redirect`, `XML Sitemap`, `PDF Link`}.
  - `Status` = Statustext bzw. Ergebnis (`OK`, `Not Found`, `Moved Permanently`, `HSTS Policy`, `DNS Lookup Failed`, `Connection Timeout`, `Blocked by Robots.txt`, `Not Crawled` …).
  - `Rel` = alle `rel`-Werte des Elements in Originalreihenfolge, kleingeschrieben, mit einem Leerzeichen getrennt; `Target` = Wert des `target`-Attributs oder leer.
  - `Alt Text` nur bei `Type`=Image (bzw. verlinktem Bild bei Hyperlink); sonst leer.
  - `Follow` = `True`/`False` (False bei `rel=nofollow`/`ugc`/`sponsored` am Link oder Meta/X-Robots nofollow der Quelle).
  - `Path Type` ∈ {`Absolute`, `Protocol-Relative`, `Root-Relative`, `Path-Relative`}.
  - `Link Path` = XPath des Link-Elements im (gerenderten bzw. rohen) DOM.
  - `Link Position` ∈ {`Navigation`, `Header`, `Content`, `Sidebar`, `Aside`, `Footer`} oder benutzerdefinierte Position.
  - `Link Origin` ∈ {`HTML`, `Rendered HTML`, `HTML & Rendered HTML`}.
- **Akzeptanzkriterien:**
  - [ ] Seite mit `<a href="/x" rel="nofollow" target="_blank">Text</a>` → Zeile mit `Type`=Hyperlink, `Anchor`=Text, `Follow`=False, `Target`=_blank, `Rel`=nofollow, `Path Type`=Root-Relative.
  - [ ] `<img src="pic.jpg" alt="Hund">` → `Type`=Image, `Alt Text`=Hund, `Path Type`=Path-Relative, `Size (Bytes)` = Größe des Bildes (wenn gecrawlt).
  - [ ] Zwei identische Links auf derselben Seite → zwei Zeilen (keine Deduplizierung).
  - [ ] `Status Code` ist leer, wenn das Ziel nicht gecrawlt wurde (z. B. externe Links bei deaktivierter Prüfung); `Status` dann `Not Crawled` **(OS-Festlegung)**.
  - [ ] Bei JS-Rendering: Link nur im gerenderten DOM → `Link Origin`=Rendered HTML; in beiden → `HTML & Rendered HTML`.
  - [ ] Link im `<nav>` → `Link Position`=Navigation; im `<footer>` → Footer; im `<main>` → Content.
  - [ ] 1 Mio. Links werden in < 60 s als CSV geschrieben (Referenz-Hardware: Apple M1, SSD).
  - [ ] Kopfzeile ist exakt `Type,Source,Destination,Size (Bytes),Alt Text,Anchor,Status Code,Status,Follow,Target,Rel,Path Type,Link Path,Link Position,Link Origin` (Golden-Vergleich).
  - [ ] Typzuordnung je Element (je eine Zeile, Golden): `<link rel="canonical">` → Canonical; `Link: <…>; rel="canonical"` (Header) → HTTP Canonical; `<link rel="next">`/`prev` → Rel Next/Rel Prev; `<link rel="alternate" hreflang>` → Hreflang; hreflang im Header → HTTP Hreflang; `<link rel="amphtml">` → AMP; `<link rel="alternate" media="only screen…">` → Mobile Alternate; `<iframe src>` → Iframe; `<video>/<audio>/<source>` → Media; `<link rel="stylesheet">` → CSS; `<script src>` → JavaScript; `<meta http-equiv="refresh">` → Meta Refresh; Antwortheader `Refresh: 0; url=/x` → HTTP Refresh; 3xx-`Location` → HTTP Redirect; Link in PDF-Datei → PDF Link.
  - [ ] `<a href="/x" rel="Nofollow  UGC">` → `Rel` = `nofollow ugc`, `Follow`=False.
  - [ ] `Link Path` ist ein absoluter XPath mit Positionsindizes, z. B. `/html/body/main/p[2]/a[1]`; derselbe Link liefert bei wiederholtem Crawl denselben Pfad.
  - [ ] Sortierung: nach `Destination`, dann `Source`, dann Dokumentreihenfolge **(OS-Festlegung)**.
- **Testfixture:** `out-links`: Seiten mit allen Linktypen (inkl. Canonical-Header, hreflang-Header, `Refresh`-Header, iframe, video, PDF mit Link), Pfadtypen, nofollow/ugc/sponsored (auch gemischte Groß-/Kleinschreibung), `target`, Links in nav/header/main/aside/footer, ein nur per JS eingefügter Link.

### OUT-032 Links: All Outlinks
- **Priorität:** P0
- **Beschreibung:** Inhaltlich gleiche Menge wie OUT-031, aber aus Sicht der Quellseiten und mit Fokus auf Quellen innerhalb des Crawls; sortiert nach `Source`, dann Linkreihenfolge im Dokument.
- **Eingaben/Konfiguration:** wie OUT-031.
- **Ausgabe:** Standard-Linkformat (OUT-031).
- **Akzeptanzkriterien:**
  - [ ] Für jede gecrawlte interne HTML-Seite stimmen die Zeilen mit `Source` = Seite mit dem Outlinks-Detailtab dieser Seite überein.
  - [ ] Reihenfolge innerhalb einer Quelle = Reihenfolge im Quelltext.
- **Testfixture:** `out-links`.

### OUT-033 Links: All Anchor Text
- **Priorität:** P1
- **Beschreibung:** Alle Hyperlinks mit ihrem Ankertext (bzw. Alt-Text verlinkter Bilder) für Ankertext-Analysen.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** `Source`, `Destination`, `Anchor`, `Alt Text`, `Link Position`, `Follow`, `Status Code` **(OS-Festlegung** der Spaltenauswahl).
- **Akzeptanzkriterien:**
  - [ ] Nur `Type`=Hyperlink (keine Ressourcen).
  - [ ] Verlinktes Bild ohne Text → `Anchor` leer, `Alt Text` gefüllt.
  - [ ] Ankertext ist whitespace-normalisiert (mehrfache Leerzeichen/Zeilenumbrüche → ein Leerzeichen, getrimmt).
- **Testfixture:** `out-links` (Link mit `<a>\n  Mehr   lesen\n</a>`, verlinktes Bild).

### OUT-034 Links: External Links
- **Priorität:** P1
- **Beschreibung:** Alle Verweise von internen Seiten auf externe Ziele.
- **Ausgabe:** Standard-Linkformat.
- **Eingaben/Konfiguration:** Definition „intern“ gemäß Crawl-Konfiguration (Subdomains, Startordner).
- **Akzeptanzkriterien:**
  - [ ] Bei deaktivierter Option „alle Subdomains crawlen“ gilt `blog.example.test` als extern und erscheint hier.
  - [ ] Externe Bilder/Skripte (CDN) erscheinen mit ihrem `Type`.
- **Testfixture:** `out-links` mit Hosts `example.test`, `blog.example.test`, `cdn.other.test` (über Host-Mapping im Fixture-Server).

### OUT-035 Links: Internal Nofollow Outlinks
- **Priorität:** P1
- **Beschreibung:** Interne Hyperlinks mit `Follow`=False.
- **Ausgabe:** Standard-Linkformat.
- **Akzeptanzkriterien:**
  - [ ] Enthält Links mit `rel` `nofollow`, `ugc`, `sponsored` (einzeln oder kombiniert) sowie Links von Seiten mit Meta-Robots `nofollow`.
  - [ ] Externe nofollow-Links sind nicht enthalten.
- **Testfixture:** `out-links`.

### OUT-036 Links: Internal Outlinks With No Anchor Text
- **Priorität:** P1
- **Beschreibung:** Interne Hyperlinks ohne zugänglichen Ankertext.
- **Ausgabe:** Standard-Linkformat.
- **Akzeptanzkriterien:**
  - [ ] Enthalten: `<a href="/x"></a>`, `<a href="/x">   </a>`, `<a href="/x"><img src="i.png"></a>` (Bild ohne alt).
  - [ ] Nicht enthalten: `<a href="/x"><img src="i.png" alt="Logo"></a>` und `<a href="/x" aria-label="Start"></a>` **(OS-Festlegung:** aria-label zählt als Text).
- **Testfixture:** `out-links-anchor`.

### OUT-037 Links: Non-Descriptive Anchor Text In Internal Outlinks
- **Priorität:** P1
- **Beschreibung:** Interne Hyperlinks, deren Ankertext (case-insensitiv, getrimmt, Satzzeichen am Ende ignoriert) in einer konfigurierbaren Liste nichtssagender Phrasen steht.
- **Eingaben/Konfiguration:** Liste in Konfiguration → Links (Standard, englisch + deutsch **(OS-Festlegung)**: „click here“, „here“, „read more“, „more“, „learn more“, „this“, „link“, „continue“, „go“, „start“, „hier“, „hier klicken“, „mehr“, „weiterlesen“, „mehr erfahren“, „weiter“).
- **Ausgabe:** Standard-Linkformat.
- **Akzeptanzkriterien:**
  - [ ] `Click Here!` → enthalten; `Click here for pricing` → nicht enthalten (exakter Phrasenvergleich, nicht Teilstring).
  - [ ] Nach Entfernen von „more“ aus der Liste ist `More` nicht mehr enthalten.
- **Testfixture:** `out-links-anchor`.

### OUT-038 Links: Follow & Nofollow Internal Inlinks To Page
- **Priorität:** P1
- **Beschreibung:** Alle internen Inlinks zu Seiten, die sowohl follow- als auch nofollow-Inlinks erhalten (inkonsistente Verlinkung).
- **Ausgabe:** Standard-Linkformat (alle Inlinks dieser Zielseiten, beide Arten).
- **Akzeptanzkriterien:**
  - [ ] Ziel `/p` mit 2 follow- und 1 nofollow-Link → 3 Zeilen; Ziel `/q` nur mit follow-Links → keine Zeile.
- **Testfixture:** `out-links-follow`.

### OUT-039 Links: Internal Nofollow Inlinks Only
- **Priorität:** P1
- **Beschreibung:** Inlinks zu Seiten, die ausschließlich über nofollow-Links intern verlinkt sind.
- **Ausgabe:** Standard-Linkformat.
- **Akzeptanzkriterien:**
  - [ ] Ziel `/r` mit nur 2 nofollow-Inlinks → 2 Zeilen; `/p` aus OUT-038 → keine Zeile.
  - [ ] Canonical/hreflang-Verweise zählen nicht als follow-Inlink.
- **Testfixture:** `out-links-follow`.

### OUT-040 Links: Outlinks to Localhost
- **Priorität:** P1
- **Beschreibung:** Verweise auf lokale Adressen, die auf Live-Seiten nicht vorkommen sollten.
- **Ausgabe:** Standard-Linkformat.
- **Akzeptanzkriterien:**
  - [ ] Enthalten: Ziele mit Host `localhost`, `127.0.0.0/8`, `[::1]`, `0.0.0.0`, Hosts mit Endung `.localhost`.
  - [ ] Nicht enthalten: private Netze `192.168.x.x`/`10.x.x.x` **(OS-Festlegung;** eigener Filter wäre P2).
  - [ ] Wenn die Fixture selbst auf 127.0.0.1 läuft, werden Links auf den **eigenen** Crawl-Host nicht gemeldet (sonst wäre jede Fixture positiv).
- **Testfixture:** `out-links` gemappt auf Host `site.test`, mit Links auf `http://localhost:3000/x` und `http://127.0.0.1/y`.

### OUT-041 Links: Pages With Uncrawlable Internal Outlinks
- **Priorität:** P1
- **Beschreibung:** Exportiert „Links“, die für Suchmaschinen nicht als normale Links crawlbar sind (seit Referenz 24.0), sofern das Speichern solcher Links aktiviert ist.
- **Eingaben/Konfiguration:** Konfiguration → Crawl → „Nicht crawlbare Links speichern“.
- **Ausgabe:** Standard-Linkformat + Spalte `Link Crawlability` mit Grund: `href on non-anchor element`, `onclick navigation`, `javascript: URL`, `missing href`, `empty href`, `fragment only` **(OS-Festlegung** der Werte).
- **Akzeptanzkriterien:**
  - [ ] `<span href="/a">`, `<div onclick="location.href='/b'">`, `<a href="javascript:go('/c')">`, `<a onclick="...">` ohne href werden je eine Zeile.
  - [ ] Normale `<a href="/d">` erscheinen nicht.
  - [ ] Ohne aktivierte Option ist der Export leer und im Menü mit Hinweis versehen.
- **Testfixture:** `out-uncrawlable-links`.

### OUT-042 Path Type: Absolute / Protocol-Relative / Root-Relative / Path-Relative Links
- **Priorität:** P2
- **Beschreibung:** Vier Exporte, je einer pro Pfadtyp der `href`-Notation, mit Quellseiten.
- **Ausgabe:** Standard-Linkformat, gefiltert nach `Path Type`.
- **Akzeptanzkriterien:**
  - [ ] `https://x.test/a` → Absolute; `//x.test/a` → Protocol-Relative; `/a` → Root-Relative; `a`, `./a`, `../a`, `?q=1` → Path-Relative.
  - [ ] Summe der vier Exporte = Anzahl aller Zeilen in All Outlinks mit nicht-leerem `href`-Attribut.
- **Testfixture:** `out-links`.

### OUT-043 Web: Screenshots
- **Priorität:** P1
- **Beschreibung:** Speichert alle beim JS-Rendering erzeugten Seiten-Screenshots als Dateien.
- **Eingaben/Konfiguration:** Rendering = JavaScript, „Screenshots speichern“ aktiv; Fenstergröße gemäß Rendering-Konfiguration.
- **Ausgabe:** Ordner `screenshots/` mit PNG je URL; Dateiname = bereinigte URL (OUT-008-Regeln) + `_<8-stelliger Hash>.png`; zusätzlich `screenshots/index.csv` mit `Address`, `File`.
- **Akzeptanzkriterien:**
  - [ ] Anzahl PNG-Dateien = Anzahl gerenderter Seiten mit gespeichertem Screenshot.
  - [ ] Ohne Rendering ist der Eintrag deaktiviert mit Hinweis.
  - [ ] Nur lokal speicherbar (kein Drive).
- **Testfixture:** `out-render`: 3 Seiten mit JS-generiertem Inhalt.

### OUT-044 Web: Archived Website
- **Priorität:** P2
- **Beschreibung:** Speichert die gerenderte Website als Offline-Kopie (Ordnerstruktur, umgeschriebene Links) und optional im WARC-Format für gängige WARC-Betrachter.
- **Eingaben/Konfiguration:** Rendering-Konfiguration „Website archivieren“; Format: Ordner / WARC / beides.
- **Ausgabe:** Ordner `archive/<host>/<pfad>/index.html` + Ressourcen; bzw. `archive.warc.gz` (WARC 1.1).
- **Akzeptanzkriterien:**
  - [ ] Offline-Kopie lässt sich per `file://` öffnen; interne Links zeigen auf lokale Dateien.
  - [ ] WARC enthält je URL einen `request`- und einen `response`-Record; Validierung mit einem WARC-Validator (z. B. `warcio check`) ohne Fehler.
- **Testfixture:** `out-render`.

### OUT-045 Web: All Page Source
- **Priorität:** P1
- **Beschreibung:** Speichert den HTML-Quelltext aller internen HTML-Seiten – wahlweise roh (wie empfangen) oder gerendert.
- **Eingaben/Konfiguration:** „HTML speichern“ bzw. „gerendertes HTML speichern“ in der Extraktions-Konfiguration; Auswahl Original/Rendered beim Export.
- **Ausgabe:** Ordner `page_source/` mit einer `.html` pro URL + `index.csv` (`Address`, `File`, `Source Type`).
- **Akzeptanzkriterien:**
  - [ ] Rohes HTML ist byte-identisch zum (dekomprimierten) Antwortkörper.
  - [ ] Ist nur rohes HTML gespeichert, ist die Option „Rendered“ deaktiviert.
- **Testfixture:** `out-render`.

### OUT-046 Web: All Page Text
- **Priorität:** P1
- **Beschreibung:** Extrahierter sichtbarer Text aller internen HTML-Seiten mit Status 200 als Textdateien.
- **Eingaben/Konfiguration:** „HTML speichern“ aktiv; Inhaltsbereich-Konfiguration (Ein-/Ausschluss von Elementen) wird beachtet.
- **Ausgabe:** Ordner `page_text/` mit `.txt` (UTF-8) pro URL + `index.csv`.
- **Akzeptanzkriterien:**
  - [ ] Seiten mit Status ≠ 200 und Nicht-HTML werden nicht exportiert.
  - [ ] Text enthält keinen Inhalt aus `<script>`, `<style>`, `<noscript>`-Tags **(OS-Festlegung** für noscript) und keine HTML-Tags.
  - [ ] Ist `<nav>` per Inhaltsbereich ausgeschlossen, fehlt der Navigationstext.
- **Testfixture:** `out-render` + Seite mit nav/footer.

### OUT-047 Web: All PDF Documents / All PDF Content
- **Priorität:** P2
- **Beschreibung:** Speichert alle gecrawlten PDF-Dateien bzw. deren extrahierten Text.
- **Eingaben/Konfiguration:** PDFs speichern / PDF-Inhalt extrahieren.
- **Ausgabe:** `pdf_documents/*.pdf` bzw. `pdf_content/*.txt`, jeweils mit `index.csv`.
- **Akzeptanzkriterien:**
  - [ ] PDF-Dateien sind byte-identisch zum Download.
  - [ ] Passwortgeschützte/defekte PDFs werden übersprungen und im `index.csv` mit `Error` vermerkt.
- **Testfixture:** `out-pdf`: 2 gültige PDFs, 1 defektes PDF.

### OUT-048 Web: All HTTP Request Headers / All HTTP Response Headers
- **Priorität:** P1
- **Beschreibung:** Alle gesendeten Request- bzw. empfangenen Response-Header je URL.
- **Eingaben/Konfiguration:** Extraktion „HTTP-Header speichern“.
- **Ausgabe:** Langformat `Address`, `Header Name`, `Header Value` (eine Zeile je Header; mehrfach vorkommende Header wie `Set-Cookie` als mehrere Zeilen).
- **Akzeptanzkriterien:**
  - [ ] Header-Namen in Originalschreibweise (HTTP/1.1) bzw. Kleinbuchstaben (HTTP/2) wie empfangen.
  - [ ] Request-Header mit Zugangsdaten (`Authorization`, `Proxy-Authorization`, `Cookie`) werden als `<redacted>` ausgegeben, außer Option „Zugangsdaten in Exporten zeigen“ ist aktiv **(OS-Festlegung)**.
- **Testfixture:** `out-headers`: Seiten mit mehreren `Set-Cookie`, `Link`-Header, Basic-Auth-Bereich.

### OUT-049 Web: All Cookies
- **Priorität:** P1
- **Beschreibung:** Alle gesetzten Cookies je URL (aus HTTP-Headern und – bei Rendering – per JavaScript gesetzt).
- **Eingaben/Konfiguration:** Extraktion „Cookies speichern“.
- **Ausgabe:** `Address`, `Cookie Name`, `Cookie Value`, `Domain`, `Path`, `Expires`, `Max-Age`, `Secure`, `HttpOnly`, `SameSite`, `Type` (`HTTP`/`JavaScript`), `Session` (`True` wenn weder Expires noch Max-Age).
- **Akzeptanzkriterien:**
  - [ ] `Set-Cookie: a=1; Secure; HttpOnly; SameSite=Lax` → eine Zeile mit `Secure`=True, `HttpOnly`=True, `SameSite`=Lax, `Session`=True.
  - [ ] Per `document.cookie` gesetztes Cookie erscheint nur mit Rendering, `Type`=JavaScript.
- **Testfixture:** `out-headers`.

### OUT-050 Response Codes: Inlinks-Exporte je Filter
- **Priorität:** P0
- **Beschreibung:** Für die Bereiche **Internal & External**, **Internal** und **External** je ein Export pro Response-Code-Filter mit allen Verweisen auf URLs dieses Filters.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Standard-Linkformat. Einträge je Bereich:
  - `All` (alle Inlinks), `Blocked by Robots.txt Inlinks`, `Blocked Resource Inlinks`, `No Response Inlinks`, `Success (2xx) Inlinks`, `Redirection (3xx) Inlinks`, `Redirection (JavaScript) Inlinks`, `Redirection (Meta Refresh) Inlinks`, `Redirection (HTTP Refresh) Inlinks` (Filter seit Referenz 23.0), `Client Error (4xx) Inlinks`, `Server Error (5xx) Inlinks`, `All Error Inlinks` (seit Referenz 23.0, Details OUT-067).
- **Akzeptanzkriterien:**
  - [ ] `Internal:Client Error (4xx) Inlinks` enthält genau alle Verweise mit internem Ziel, dessen Status 400–499 ist; Anzahl Ziele = Zähler „Client Error (4xx)“ im Tab Internal-Bereich.
  - [ ] `No Response Inlinks` umfasst Ziele mit DNS-Fehler, Verbindungsabbau, Timeout, ungültiger URL.
  - [ ] `Redirection (HTTP Refresh) Inlinks` enthält Verweise auf Ziele, die mit Status 200 und Antwortheader `Refresh: <n>; url=<ziel>` antworten; diese Ziele erscheinen **nicht** in `Redirection (Meta Refresh) Inlinks` (und umgekehrt: `<meta http-equiv="refresh">` nur in Meta Refresh).
  - [ ] `Success (2xx) Inlinks` enthält die HTTP-Refresh-Ziele **nicht**, obwohl ihr Statuscode 200 ist (Refresh-Klassifikation hat Vorrang, analog Meta Refresh) **(OS-Festlegung)**.
  - [ ] Quelle eines Verweises kann selbst eine Weiterleitung sein (`Type`=HTTP Redirect), wenn die 404 nur über eine Weiterleitung erreicht wurde.
  - [ ] Jeder der 12 Einträge × 3 Bereiche ist per CLI `--bulk-export` ansprechbar (z. B. `"Response Codes:Internal:Redirection (HTTP Refresh) Inlinks"`).
- **Testfixture:** `out-response-codes`: Ziele mit 200, 301→404, 302, Meta-Refresh, `Refresh`-Header (`Refresh: 0; url=/refresh-target`), JS-Redirect, 403, 404, 410, 500, 503, Timeout (Fixture verzögert 60 s), DNS-Fehler (`nonexistent.invalid`), robots-gesperrte URL, extern 404, extern 500.

### OUT-051 Security: Exporte je Filter
- **Priorität:** P1
- **Beschreibung:** Für jeden Security-Filter, bei dem ein Verweis die Ursache ist, ein Export der betroffenen Verweise.
- **Ausgabe:** Standard-Linkformat. Einträge: `HTTP URLs Inlinks`, `Mixed Content` (Quelle = HTTPS-Seite, Ziel = HTTP-Ressource), `Form URL Insecure` (Formular-`action` auf HTTP), `Form On HTTP URL`, `Unsafe Cross-Origin Links` (externe Links mit `target=_blank` ohne `noopener`/`noreferrer`), `Protocol-Relative Resource Links`.
- **Akzeptanzkriterien:**
  - [ ] HTTPS-Seite mit `<img src="http://…">` → Zeile in Mixed Content, `Type`=Image.
  - [ ] `<a href="https://ext.test" target="_blank">` ohne rel → Unsafe Cross-Origin Links; mit `rel="noopener"` nicht.
  - [ ] `<form action="http://…/login">` auf HTTPS-Seite → Form URL Insecure.
- **Testfixture:** `out-security` (Fixture-Server mit TLS über selbstsigniertes Zertifikat, Vertrauen per Test-Konfiguration).

### OUT-052 Content: Duplikat-, Ähnlichkeits- und Rechtschreib-Exporte
- **Priorität:** P1
- **Beschreibung:** Paarweise bzw. detaillierte Exporte der Content-Filter.
- **Ausgabe:**
  - `Exact Duplicates`: `Address`, `Exact Duplicate Address`, `Hash`.
  - `Near Duplicates`: `Address`, `Near Duplicate Address`, `Similarity` (%), nur Paare ≥ eingestellter Schwelle (Standard 90 %).
  - `Semantically Similar`: `Address`, `Semantically Similar Address`, `Similarity Score` (0–1), nur Paare ≥ Schwelle.
  - `Low Relevance Content`: `Address`, `Relevance Score`.
  - `Spelling Errors`: `Address`, `Error`, `Suggestion`, `Section` (Title/Meta Description/H1/Content …), `Context`, `Language`.
  - `Grammar Errors`: `Address`, `Error`, `Rule`, `Detail` (eigene Kurzbeschreibung der Regel), `Suggestion`, `Section`, `Context`, `Language`.
  - `Spelling & Grammar Errors`: Vereinigung beider mit zusätzlicher Spalte `Error Type` (Spelling/Grammar) an zweiter Stelle.
  - Hinweis zur Benennung: Die öffentliche Tutorial-Doku der Referenz beschreibt einen kombinierten Export; der Spec-Review gegen Version 24.3 nennt getrennte Einträge für Rechtschreib- und Grammatikfehler (vor Umsetzung per Golden-Export der Free-Version bestätigen, Anhang B). OpenSpider registriert alle drei kanonischen Pfade `Content:Spelling Errors`, `Content:Grammar Errors`, `Content:Spelling & Grammar Errors`, damit Skripte beider Varianten funktionieren.
- **Akzeptanzkriterien:**
  - [ ] Zwei Seiten mit identischem Inhaltsbereich → je Seite eine Zeile (Paar symmetrisch in beide Richtungen).
  - [ ] Bei Schwelle 90 % erscheinen Paare mit 89,9 % nicht, mit 90,0 % schon.
  - [ ] Exporte erfordern abgeschlossene Crawl-Analyse; sonst Hinweis „Crawl-Analyse ausführen“ und leerer Export.
  - [ ] Seite mit 2 Rechtschreib- und 1 Grammatikfehler → `Spelling Errors` 2 Zeilen, `Grammar Errors` 1 Zeile, `Spelling & Grammar Errors` 3 Zeilen; Zeilenzahl kombiniert = Summe der beiden Einzelexporte (Invariante, Registry-Test).
  - [ ] `--bulk-export "Content:Spelling Errors,Content:Grammar Errors"` erzeugt `content_spelling_errors.csv` und `content_grammar_errors.csv`.
  - [ ] Fehler im Title werden mit `Section`=Title ausgegeben, Fehler im Fließtext mit `Section`=Content.
- **Testfixture:** `out-content-dupes`: 2 exakte Duplikate, 2 Seiten mit ~92 % Überschneidung, 1 Seite mit ~85 %; `out-detail-tabs`: Seite mit „Beispeil“ im Title und „Fehlerr“ im Text sowie einem Grammatikfehler („die Haus“).

### OUT-053 Images: Exporte je Filter
- **Priorität:** P1
- **Beschreibung:** Verweise auf Bilder der jeweiligen Image-Filter (Quelle = einbindende Seite).
- **Ausgabe:** Standard-Linkformat. Einträge: `All Image Inlinks`, `Images Missing Alt Text Inlinks`, `Images Missing Alt Attribute Inlinks`, `Images With Alt Text Over X Characters Inlinks` (X aus Präferenzen, Standard 100), `Images Over X KB Inlinks` (X Standard 100), `Images Missing Size Attributes Inlinks`, `Incorrectly Sized Images Inlinks`, `Background Images`.
- **Akzeptanzkriterien:**
  - [ ] `<img src="a.png" alt="">` → in „Missing Alt Text“, nicht in „Missing Alt Attribute“; `<img src="b.png">` → nur in „Missing Alt Attribute“ (beide Filter sind disjunkt).
  - [ ] Alt-Text mit 101 Zeichen → enthalten; mit 100 Zeichen → nicht.
  - [ ] Bild 100 KB (102 400 Bytes) → nicht enthalten; 102 401 Bytes → enthalten (Grenze: „über“).
  - [ ] Der Name im Menü zeigt den aktuell konfigurierten Wert statt „X“; CLI akzeptiert beide Schreibweisen.
- **Testfixture:** `out-images`: Bilder mit/ohne alt, langer alt, 100 KB und 100 KB + 1 Byte, ohne width/height, CSS-Hintergrundbild.

### OUT-054 Canonicals: Exporte je Filter
- **Priorität:** P1
- **Beschreibung:** Je Canonicals-Filter ein Inlinks-Export auf die betroffenen URLs.
- **Ausgabe:** Standard-Linkformat. Einträge: `Contains Canonical Inlinks`, `Self Referencing Inlinks`, `Canonicalised Inlinks`, `Missing Inlinks`, `Multiple Inlinks`, `Multiple Conflicting Inlinks`, `Non-Indexable Canonical Inlinks`, `Canonical Is Relative Inlinks`, `Unlinked Inlinks`, `Invalid Attribute In Annotation Inlinks`, `Outside <head> Inlinks`.
- **Akzeptanzkriterien:**
  - [ ] Seite `/a` mit Canonical auf `/b`, verlinkt von `/home` und `/x` → `Canonicalised Inlinks` enthält 2 Zeilen mit `Destination`=`/a`.
  - [ ] Für jeden Filter des Canonicals-Tabs (Spec-Bereich Indexierung) existiert genau ein Export; Registry-Test schlägt fehl, wenn ein Filter ohne Export hinzukommt.
- **Testfixture:** `out-canonicals`: je eine Seite pro Filterfall.

### OUT-055 Directives: Exporte je Filter
- **Priorität:** P1
- **Beschreibung:** Je Directives-Filter ein Inlinks-Export.
- **Ausgabe:** Standard-Linkformat. Einträge (je `… Inlinks`): `Index`, `Noindex`, `Follow`, `Nofollow`, `None`, `NoArchive`, `NoSnippet`, `Max-Snippet`, `Max-Image-Preview`, `Max-Video-Preview`, `NoODP`, `NoYDIR`, `NoImageIndex`, `NoTranslate`, `Unavailable_After`, `Refresh`, `Outside <head>`.
- **Akzeptanzkriterien:**
  - [ ] Seite mit `X-Robots-Tag: noindex` (Header) erscheint im Noindex-Ziel genauso wie mit Meta-Tag.
  - [ ] Registry-Test: jeder Directives-Filter hat einen Export.
- **Testfixture:** `out-directives`.

### OUT-056 Structured Data: Exporte (RDF-Tripel)
- **Priorität:** P1
- **Beschreibung:** Strukturierte Daten aller URLs eines Filters als Tripel (Subjekt–Prädikat–Objekt), formatübergreifend einheitlich (JSON-LD, Microdata, RDFa).
- **Ausgabe:** `Address`, `Format`, `Subject`, `Predicate`, `Object`, `Validation Type`, `Severity`, `Issue` (die letzten drei nur bei Validierungsfunden). Einträge: `Contains Structured Data`, `Validation Errors`, `Validation Warnings`, `Parse Errors`, `Microdata URLs`, `JSON-LD URLs`, `RDFa URLs`, `Rich Result Feature Detected` **(OS-Festlegung** zur Benennung des letzten).
- **Akzeptanzkriterien:**
  - [ ] JSON-LD `{"@type":"Product","name":"X","offers":{"@type":"Offer","price":"5"}}` ergibt mindestens die Tripel `_:b0 rdf:type schema:Product`, `_:b0 schema:name "X"`, `_:b0 schema:offers _:b1`, `_:b1 schema:price "5"`. Blank-Node-IDs sind je URL deterministisch.
  - [ ] Parse-Error-URL (ungültiges JSON) erscheint im Export `Parse Errors` mit `Issue` = Parser-Meldung und Zeile/Spalte.
- **Testfixture:** `out-structured-data`: gültiges Product, Product ohne Pflichtfeld, kaputtes JSON-LD, Microdata-Breadcrumb, RDFa-Organisation.

### OUT-057 Sitemaps: Exporte je Filter
- **Priorität:** P1
- **Beschreibung:** Für Filter des Sitemaps-Tabs: welche URL steht in welcher Sitemap.
- **Ausgabe:** `Sitemap`, `Address`, `Status Code`, `Indexability`, `Indexability Status`. Einträge: `URLs In Sitemap`, `URLs Not In Sitemap` (nur `Address`), `Non-Indexable URLs In Sitemap`, `URLs In Multiple Sitemaps`, `XML Sitemap With Over 50k URLs`, `XML Sitemap Over 50MB`, `Orphan URLs`.
- **Akzeptanzkriterien:**
  - [ ] URL in `sitemap-a.xml` und `sitemap-b.xml` → 2 Zeilen in „URLs In Multiple Sitemaps“.
  - [ ] Noindex-URL in Sitemap → „Non-Indexable URLs In Sitemap“.
  - [ ] Erfordert, dass Sitemaps gecrawlt wurden; sonst Hinweis.
- **Testfixture:** `out-sitemaps-in`: Sitemap-Index mit 2 Sitemaps, Überschneidung, noindex-URL, 404-URL, verwaiste URL.

### OUT-058 JavaScript: Exporte je Filter
- **Priorität:** P2
- **Beschreibung:** Inlinks-Exporte je JavaScript-Filter sowie Konsolen-Log aller Seiten.
- **Ausgabe:** Standard-Linkformat je Filter (`Blocked Resource`, `Contains JavaScript Links`, `Contains JavaScript Content`, `Noindex Only In Original HTML`, `Nofollow Only In Original HTML`, `Canonical Only In Rendered HTML`, `Canonical Mismatch`, `Page Title Only In Rendered HTML`, `Pages With JavaScript Errors`, …); `All Console Messages`: `Address`, `Level` (error/warning/info/log), `Message`, `Source`, `Line`.
- **Akzeptanzkriterien:**
  - [ ] `Contains JavaScript Links` liefert die Links, die nur im gerenderten DOM vorhanden sind (nicht leer, bekannter Fehler der Referenz).
  - [ ] `console.error("x")` auf einer Seite → eine Zeile `Level`=error.
- **Testfixture:** `out-render` + Seite mit Konsolenfehler.

### OUT-059 AMP: Exporte je Filter
- **Priorität:** P2
- **Beschreibung:** Je AMP-Filter ein Inlinks-Export auf die betroffenen AMP-URLs (Quelle = Seite, die per `rel="amphtml"` oder Hyperlink auf die AMP-Seite verweist).
- **Ausgabe:** Standard-Linkformat. Einträge (je `… Inlinks`): `Non-200 Response`, `Missing Non-AMP Return Link`, `Missing Canonical to Non-AMP`, `Non-Indexable Canonical`, `Indexable`, `Non-Indexable`, `Missing <html amp> Tag`, `Missing/Invalid <!doctype html> Tag`, `Missing <head> Tag`, `Missing <body> Tag`, `Missing Canonical`, `Missing/Invalid <meta charset> Tag`, `Missing/Invalid <meta viewport> Tag`, `Missing/Invalid AMP Script`, `Missing/Invalid AMP Boilerplate`, `Contains Disallowed HTML`, `Other Validation Errors`. Die maßgebliche Filterliste definiert der Spec-Bereich des AMP-Tabs; Registry-Test erzwingt 1:1-Abdeckung.
- **Akzeptanzkriterien:**
  - [ ] `amphtml`-Link auf 404 → Eintrag in „Non-200 Response“ mit `Type`=AMP, `Status Code`=404.
  - [ ] AMP-Seite `/p/amp` ohne `<link rel="canonical">` → „Missing Canonical“; mit Canonical auf `/p`, aber `/p` verweist nicht per `amphtml` zurück → „Missing Non-AMP Return Link“.
  - [ ] AMP-Seite mit Canonical auf eine noindex-Seite → „Non-Indexable Canonical“.
  - [ ] AMP-Seite ohne `⚡`/`amp`-Attribut an `<html>` → „Missing <html amp> Tag“; `<html amp>` und `<html ⚡>` gelten beide als vorhanden.
  - [ ] AMP-Seite mit `<img>` statt `<amp-img>` → „Contains Disallowed HTML“.
  - [ ] Ohne aktivierte AMP-Extraktion sind alle AMP-Exporte im Menü ausgegraut mit Hinweis.
  - [ ] Registry-Test: Anzahl AMP-Exporte = Anzahl AMP-Filter.
- **Testfixture:** `out-amp`: Normalseite mit `amphtml`-Link auf gültige AMP-Seite, auf 404-AMP-Seite, AMP-Seite ohne Canonical, ohne Rückverweis, mit noindex-Canonical, ohne `amp`-Attribut, mit `<img>`, ohne Boilerplate.

### OUT-060 Custom Search: Exporte je Suchfilter
- **Priorität:** P1
- **Beschreibung:** Für jeden benutzerdefinierten Suchfilter ein Export der URLs mit Treffern (bzw. ohne Treffer bei „Does Not Contain“), inklusive Trefferanzahl.
- **Ausgabe:** `Address`, `Filter`, `Occurrences`, `Status Code`, `Indexability` **(OS-Festlegung;** Referenz liefert Inlinks – zusätzlich `… Inlinks`-Variante im Standard-Linkformat).
- **Akzeptanzkriterien:**
  - [ ] Filter „Contains `Ausverkauft`“ auf 2 Seiten (eine mit 3 Treffern) → 2 Zeilen, Occurrences 3 und 1.
  - [ ] Pro Filter eine Datei, Dateiname enthält Filternamen.
- **Testfixture:** `out-custom-search`.

### OUT-061 Custom Extraction: Exporte
- **Priorität:** P1
- **Beschreibung:** Alle extrahierten Werte im Langformat sowie Inlinks-Exporte je Extraktor.
- **Ausgabe:** `All Extractions`: `Address`, `Extractor`, `Index` (1-basiert bei mehreren Treffern), `Value`. Je Extraktor `… Inlinks` im Standard-Linkformat.
- **Akzeptanzkriterien:**
  - [ ] XPath-Extraktor `//span[@class='price']` auf Seite mit 2 Preisen → 2 Zeilen mit Index 1 und 2.
  - [ ] Langformat hat keine Begrenzung auf die in der Tabelle angezeigte Anzahl Spalten.
- **Testfixture:** `out-custom-extraction`.

### OUT-062 Accessibility: All Violations und je WCAG-Stufe
- **Priorität:** P1
- **Beschreibung:** Alle Barrierefreiheitsverstöße mit Fundstelle; zusätzlich gefiltert nach WCAG-Stufe.
- **Eingaben/Konfiguration:** Accessibility-Prüfung + JS-Rendering aktiv.
- **Ausgabe:** `Address`, `Issue`, `Rule ID`, `Guidelines` (z. B. `WCAG 2.0 A, 1.1.1`), `User Impact` (Critical/Serious/Moderate/Minor), `Priority` (High/Medium/Low), `Location` (CSS-Selektor), `HTML Snippet`, `Help URL`. Einträge: `All Violations`, `WCAG 2.0 A`, `WCAG 2.0 AA`, `WCAG 2.0 AAA`, `WCAG 2.1 AA`, `WCAG 2.2 AA`, `Best Practice`.
- **Akzeptanzkriterien:**
  - [ ] `<img src="x.png">` ohne alt → Zeile mit Regel „image-alt“, Stufe WCAG 2.0 A, Impact Critical.
  - [ ] Summe der Stufen-Exporte = All Violations (jede Verletzung genau einer Stufe zugeordnet; maßgeblich ist die niedrigste zutreffende Stufe).
- **Testfixture:** `out-a11y`: fehlender alt, zu geringer Kontrast, fehlendes Label, doppelte IDs.

### OUT-063 Issues: Alle Issues als Dateisammlung
- **Priorität:** P0
- **Beschreibung:** Exportiert jede Zeile des Issues-Panels als eigene Datei mit den betroffenen URLs; bei linkbezogenen Issues zusätzlich die Inlinks-Variante.
- **Eingaben/Konfiguration:** Zielordner; Format.
- **Ausgabe:** Ordner `issues/` mit Dateien `<issue_type>_<issue_name>.csv` (z. B. `issue_response_codes_internal_client_error_4xx.csv`, `issue_..._inlinks.csv`) und einer `issues_overview.csv` (Format wie OUT-025 Issues; die Texte in `Description`/`How To Fix` sind eigenständig verfasste OpenSpider-Texte, keine Übernahme oder Nachbildung der Issue-Beschreibungen der Referenz).
- **Akzeptanzkriterien:**
  - [ ] Anzahl Dateien (ohne Inlinks-Varianten und Übersicht) = Anzahl Zeilen im Issues-Panel.
  - [ ] Jede Datei enthält genau die URLs, die der Issue-Zähler nennt.
  - [ ] Die Option „Nicht indexierbare URLs bei Issues ignorieren“ wirkt identisch auf Panel und Export.
  - [ ] Bei Google Sheets: eine Tabelle mit einem Blatt je Issue (Konsolidierung erzwungen), da sonst Hunderte Dateien entstünden **(OS-Festlegung)**.
- **Testfixture:** `out-issues`: Seite ohne Title, doppelte Titles, 404-Link, fehlendes H1, langer Title.

### OUT-064 URL Inspection: Rich Results / Referring Pages / Sitemaps
- **Priorität:** P2
- **Beschreibung:** Detaildaten aus der Search-Console-URL-Inspection-Schnittstelle.
- **Eingaben/Konfiguration:** GSC-Verbindung mit aktivierter URL-Inspektion (Tageslimit 2 000 je Property beachten).
- **Ausgabe:**
  - `Rich Results`: `Address`, `Rich Result Type`, `Validity`, `Severity`, `Issue Type`, `Item Name`.
  - `Referring Pages`: `Address`, `Referring Page` (max. 5 je URL laut Schnittstelle).
  - `Sitemaps`: `Address`, `Sitemap`.
- **Akzeptanzkriterien:**
  - [ ] Mit Mock-GSC-Antwort mit 7 Referring Pages werden nur die gelieferten (≤ 5) ausgegeben.
  - [ ] Auch im APIs-Modus (ohne Crawl) verfügbar.
- **Testfixture:** Mock-GSC-Endpunkt im `os-testsite`.

### OUT-065 AI: generierte Bilder und Audiodateien
- **Priorität:** P2
- **Beschreibung:** Speichert durch KI-Integrationen (Prompts mit Ausgabetyp Bild bzw. Sprache) erzeugte Bilder bzw. Audiodateien als Dateien. Zwei Einträge: `AI:All Generated Images`, `AI:All Generated Audio`.
- **Eingaben/Konfiguration:** KI-Prompt mit Ausgabetyp Bild/Audio; Zielordner.
- **Ausgabe:** Ordner `ai_images/<prompt_name>/` bzw. `ai_audio/<prompt_name>/` mit einer Datei je URL; Dateiname = bereinigte URL (OUT-008) + `_<8-stelliger Hash>.<ext>`; je Ordner `index.csv` mit `Address`, `Prompt`, `File`, `MIME Type`, `Bytes`, `Error`.
- **Akzeptanzkriterien:**
  - [ ] Nur lokal speicherbar (Drive-Ziel deaktiviert); Erweiterung aus dem MIME-Typ der Anbieterantwort: `image/png`→`.png`, `image/jpeg`→`.jpg`, `image/webp`→`.webp`, `audio/mpeg`→`.mp3`, `audio/wav`→`.wav`; unbekannter Typ → `.bin` und Warnung.
  - [ ] Zwei Bild-Prompts über 3 URLs → 2 Unterordner mit je 3 Dateien und je einem `index.csv` mit 3 Zeilen.
  - [ ] URL, für die der Anbieter einen Fehler lieferte → keine Datei, aber Zeile in `index.csv` mit leerem `File` und gefülltem `Error`.
  - [ ] Dateien sind byte-identisch mit der (ggf. base64-dekodierten) Anbieterantwort.
  - [ ] Ohne Prompt mit Ausgabetyp Bild/Audio sind die Einträge ausgegraut.
- **Testfixture:** Mock-KI-Endpunkt mit festen PNG-/MP3-Antworten und einer Fehlerantwort (HTTP 500) für eine URL; `out-basic-site` (3 Seiten).

### OUT-066 Weitere Tab-Bulk-Exporte (generische Regel)
- **Priorität:** P1
- **Beschreibung:** Damit kein Filter ohne Massenexport bleibt, gilt: Für jeden Filter jedes Haupt-Tabs, dessen Befund an der Ziel-URL hängt (Page Titles, Meta Description, H1, H2, Hreflang, Pagination, Mobile, Validation, Link Metrics, Analytics, Search Console, PageSpeed), steht ein Export `<Tab>:<Filter> Inlinks` im Standard-Linkformat bereit, sofern der Bereich „Inlinks-Exporte für alle Filter“ aktiviert ist (Standard: an) **(OS-Festlegung,** Erweiterung gegenüber Referenz).
- **Ausgabe:** Standard-Linkformat.
- **Akzeptanzkriterien:**
  - [ ] Registry-Test: Anzahl generierter Inlinks-Exporte = Anzahl registrierter Filter dieser Tabs.
  - [ ] `Page Titles:Missing Inlinks` enthält alle Inlinks auf Seiten ohne Title.
- **Testfixture:** `out-issues`.

### OUT-067 Response Codes: All Error Inlinks
- **Priorität:** P1
- **Beschreibung:** Ein gebündelter Export aller Verweise auf fehlerhafte Ziele – Ziele ohne Antwort, mit Client-Fehler (4xx) und mit Server-Fehler (5xx) – in einer Datei (in der Referenz seit 23.0 unter Bulk Export → Response Codes). Erspart das Zusammenführen dreier Einzelexporte.
- **Eingaben/Konfiguration:** keine; verfügbar in den Bereichen `Internal & External`, `Internal`, `External` (kanonische Pfade `Response Codes:All Error Inlinks`, `Response Codes:Internal:All Error Inlinks`, `Response Codes:External:All Error Inlinks`) **(OS-Festlegung** für die Verfügbarkeit in allen drei Bereichen).
- **Ausgabe:** Standard-Linkformat (OUT-031); `Status Code` leer und `Status` mit Fehlergrund (z. B. `DNS Lookup Failed`, `Connection Timeout`, `Connection Refused`) bei Zielen ohne Antwort.
- **Akzeptanzkriterien:**
  - [ ] Mengengleichheit: Zeilen von `All Error Inlinks` = Vereinigung der Zeilen von `No Response Inlinks`, `Client Error (4xx) Inlinks` und `Server Error (5xx) Inlinks` desselben Bereichs (gleiche Zeilenzahl, keine Duplikate, Test über Multimengen-Vergleich).
  - [ ] Nicht enthalten: Verweise auf 2xx, 3xx (auch wenn die Kette auf 404 endet – die 404 selbst ist enthalten, die Weiterleitungs-URL nicht), robots-gesperrte Ziele, Blocked Resource.
  - [ ] Ein externes Ziel mit 500 erscheint in `Internal & External` und `External`, nicht in `Internal`.
  - [ ] Sortierung: `Status Code` aufsteigend (leere Werte = No Response zuerst), dann `Destination`, dann `Source` **(OS-Festlegung)**.
  - [ ] Erscheint im Crawl Overview (OUT-080) und im Multi-Export-Baum (OUT-075).
- **Testfixture:** `out-response-codes` (enthält No Response, 403, 404, 410, 500, 503, extern 404/500, 301→404).

### OUT-075 Multi-Export (Mehrfachexport mit Vorlagen)
- **Priorität:** P1
- **Beschreibung:** Ein Dialog (Bulk Export → Multi Export) zeigt einen Baum aller Tabs/Filter, Bulk-Exporte und Reports mit Checkboxen. Mit einem Klick werden alle gewählten Exporte erzeugt. Auswahlen lassen sich als benannte Vorlagen (Presets) speichern und in UI, Scheduling und CLI wiederverwenden. Optional wird zusätzlich der Looker-Studio-Export (OUT-210) für einen manuellen Crawl ausgeführt.
- **Eingaben/Konfiguration:** Auswahl, Format, Zielordner/Drive, Optionen Konsolidierung (OUT-007), Leere überspringen (OUT-012), Segment (OUT-011), Überschreiben/Zeitstempel (OUT-013).
- **Ausgabe:** Dateien bzw. Arbeitsmappe; Preset-Datei `<name>.ospreset` (TOML, Liste kanonischer Exportpfade + Optionen) im Einstellungsordner **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Auswahl „Internal:All“, „Links:All Inlinks“, „Reports:Crawl Overview“ → drei Dateien in einem Lauf.
  - [ ] Suche im Baum filtert Einträge nach Teilstring.
  - [ ] Preset speichern → nach Neustart wieder auswählbar; Preset per CLI `--export-preset <name|pfad>` nutzbar.
  - [ ] Preset mit unbekanntem Exportnamen (z. B. aus neuerer Version) → Warnung, übrige Exporte laufen.
  - [ ] Einträge, deren Voraussetzung fehlt (z. B. Screenshots ohne Rendering), sind markiert und werden übersprungen mit Logeintrag.
- **Testfixture:** `out-basic-site`.

---

## 4 Reports

Menü „Reports“. Reports sind aufbereitete Auswertungen, die nicht 1:1 einem Tab entsprechen. Reports, deren Daten erst in der Nachlauf-Analyse („Crawl Analysis“) entstehen, sind vor deren Abschluss mit Hinweis gekennzeichnet; der Export liefert dann nur die Kopfzeile und meldet „Crawl-Analyse erforderlich“. Kanonische CLI-Pfade: `"<Untermenü>:<Report>"`, z. B. `"Redirects:Redirect Chains"`; Reports ohne Untermenü nur `"Crawl Overview"`.

### OUT-080 Crawl Overview
- **Priorität:** P0
- **Beschreibung:** Gesamtübersicht des Crawls in einer Tabelle: Eckdaten, Inhaltstypen, Antwortcodes und für jeden Tab jeden Filter mit Anzahl und Anteil – identisch mit den Zählern im rechten „Overview“-Panel.
- **Eingaben/Konfiguration:** optional Segment.
- **Ausgabe:** Langformat mit Spalten `Section`, `Item`, `URLs`, `% of Total`. Abschnitte in dieser Reihenfolge:
  1. `Summary`: `Start URL`/`List File`, `Mode`, `Crawl Start`, `Crawl End`, `Elapsed Time`, `Total URLs Encountered`, `Total URLs Crawled`, `Total Internal Blocked by robots.txt`, `Total External Blocked by robots.txt`, `URLs Remaining In Queue`, `Average Response Time (ms)`, `Average URLs/s`, `OpenSpider Version`.
  2. `Crawl Data – Internal`: `All`, `HTML`, `JavaScript`, `CSS`, `Images`, `PDF`, `Plugins`, `Media`, `Fonts`, `XML`, `Other`, `Unknown`.
  3. `Crawl Data – External`: wie 2.
  4. `Response Codes` (intern+extern getrennt): `Blocked by Robots.txt`, `Blocked Resource`, `No Response`, `Success (2xx)`, `Redirection (3xx)`, `Redirection (JavaScript)`, `Redirection (Meta Refresh)`, `Redirection (HTTP Refresh)`, `Redirect Chain`, `Redirect Loop`, `Client Error (4xx)`, `Server Error (5xx)`, `All Errors` (Summe No Response + 4xx + 5xx, Gegenstück zu OUT-067) **(OS-Festlegung** für die Zeile `All Errors`).
  5. Je Tab (Reihenfolge wie UI) alle Filter.
  6. `Crawl Depth`: `0` … `10`, `10+` (Zeile `0` = Start-URL(s); die Referenz beginnt bei 1, siehe OUT-025 **(OS-Festlegung)**).
  7. `Issues`: Anzahl Issues/Warnings/Opportunities nach Priorität.
- **Akzeptanzkriterien:**
  - [ ] Jeder Zähler stimmt mit dem entsprechenden Zähler im UI-Overview überein (automatisierter Abgleich über alle Filter).
  - [ ] `% of Total` bezieht sich auf die Grundgesamtheit des Abschnitts (z. B. Page-Titles-Filter auf „interne HTML-URLs“); Rundung auf 2 Nachkommastellen; Division durch 0 → leer.
  - [ ] Filter, die mangels Konfiguration nicht befüllt werden (z. B. PageSpeed ohne API), erscheinen mit leerem `URLs` statt 0.
  - [ ] Im Vergleichsmodus zusätzliche Spalten (siehe OUT-147).
  - [ ] `Total URLs Encountered` ≥ `Total URLs Crawled`; Differenz = blockierte + nicht gecrawlte + Queue.
  - [ ] `Redirection (HTTP Refresh)` zählt im Fixture genau die Ziel-URL mit `Refresh`-Header (1), `Redirection (Meta Refresh)` genau die Meta-Refresh-Seite (1).
  - [ ] `All Errors` = `No Response` + `Client Error (4xx)` + `Server Error (5xx)` je Bereich (Invariante).
- **Testfixture:** `out-basic-site` + `out-response-codes`; Golden `crawl_overview.csv`.

### OUT-081 Redirects: All Redirects
- **Priorität:** P0
- **Beschreibung:** Alle internen (und, falls externe gefolgt werden, externen) Weiterleitungen – Einzelsprünge und Ketten – je Start-URL eine Zeile, mit allen Sprüngen bis zum Endziel.
- **Eingaben/Konfiguration:** Max. Anzahl zu folgender Weiterleitungen (Crawl-Limits); Weiterleitungsarten: HTTP 3xx, HSTS-Policy (interne 307-Umleitung auf HTTPS, wenn „HSTS respektieren“ aktiv), Meta Refresh, HTTP-`Refresh`-Header, JavaScript-Redirect (bei Rendering).
- **Ausgabe:** `Chain Type` (`HTTP Redirect`, `HSTS Policy`, `JavaScript Redirect`, `Meta Refresh Redirect`, `HTTP Refresh`, `Mixed` – `Mixed` sobald eine Kette Sprünge unterschiedlicher Art enthält **(OS-Festlegung)**), `Number of Redirects`, `Redirect Loop` (True/False), `Temp Redirect in Chain` (True/False), `Address` (Start-URL), `Final Address`, `Final Status Code`, `Final Status`, `Final Content Type`, `Final Indexability`, `Final Indexability Status`, `Inlinks` (Anzahl Inlinks auf die Start-URL), `Sample Source` (eine verlinkende Seite) **(OS-Festlegung** für die letzten beiden), danach je Sprung n (1…max): `Redirect Type n` (∈ {`HTTP Redirect`, `HSTS Policy`, `JavaScript Redirect`, `Meta Refresh Redirect`, `HTTP Refresh`}), `Redirect URL n`, `Status Code n`, `Status n`.
- **Akzeptanzkriterien:**
  - [ ] HSTS: `http://site.test/h` auf einem Host, der zuvor `Strict-Transport-Security` geliefert hat, und aktivem „HSTS respektieren“ → erster Sprung `Redirect Type 1`=`HSTS Policy`, `Status Code 1`=307, `Status 1`=`HSTS Policy`; ohne die Option erscheint stattdessen der echte Server-Sprung (z. B. 301, `HTTP Redirect`).
  - [ ] HTTP Refresh: Antwort 200 mit Header `Refresh: 0; url=/r2` → `Redirect Type 1`=`HTTP Refresh`, `Status Code 1`=200, `Chain Type`=`HTTP Refresh`.
  - [ ] Meta Refresh: `<meta http-equiv="refresh" content="0;url=/m2">` → `Redirect Type 1`=`Meta Refresh Redirect`.
  - [ ] Kette 301 → Meta Refresh → 200 → `Chain Type`=`Mixed`, `Redirect Type 1`=`HTTP Redirect`, `Redirect Type 2`=`Meta Refresh Redirect`.
  - [ ] `/a` 301→ `/b` 200 → eine Zeile, `Number of Redirects`=1, `Final Status Code`=200, `Temp Redirect in Chain`=False.
  - [ ] `/c` 302→ `/d` 301→ `/e` 200 → `Number of Redirects`=2, `Temp Redirect in Chain`=True (302, 303, 307 sowie Meta Refresh und HTTP Refresh mit Verzögerung > 0 gelten als temporär **(OS-Festlegung** für Meta/HTTP Refresh); die interne 307 der HSTS Policy zählt **nicht** als temporär, da sie keine Server-Weiterleitung ist **(OS-Festlegung)**).
  - [ ] Zwischenglieder einer Kette (`/d`) erzeugen keine eigene Zeile, wenn sie nur über die Kette erreicht wurden; sind sie zusätzlich direkt verlinkt, erhalten sie eine eigene Zeile **(OS-Festlegung)**.
  - [ ] Schleife `/f`→`/g`→`/f` → `Redirect Loop`=True, `Final Address` leer, `Final Status` = `Redirect Loop`.
  - [ ] Abbruch bei Erreichen des Weiterleitungslimits → `Final Status` = `Max Redirects Reached`.
  - [ ] Spaltenzahl richtet sich nach der längsten Kette im Crawl.
- **Testfixture:** `out-redirects`: einfache 301, 302-301-Kette, 5er-Kette, Schleife, Kette auf 404, Meta Refresh, `Refresh`-Header, JS-Redirect, 308, gemischte Kette 301→Meta Refresh, HSTS-Host (TLS-Fixture liefert `Strict-Transport-Security: max-age=31536000`, HTTP-Variante 301 auf HTTPS), Kette über Host-Wechsel http→https→www.

### OUT-082 Redirects: Redirect Chains
- **Priorität:** P0
- **Beschreibung:** Wie OUT-081, aber nur Ketten mit **mindestens 2** Weiterleitungen.
- **Ausgabe:** Spalten wie OUT-081.
- **Akzeptanzkriterien:**
  - [ ] `/a` (1 Sprung) fehlt; `/c` (2 Sprünge) und die 5er-Kette sind enthalten; die Schleife ist enthalten.
  - [ ] Filterbar über die `Redirect Loop`-Spalte (Excel-Autofilter vorhanden, OUT-003).
- **Testfixture:** `out-redirects`.

### OUT-083 Redirects: Redirect & Canonical Chains
- **Priorität:** P1
- **Beschreibung:** Ketten aus Weiterleitungen **und/oder** Canonicals mit mindestens 2 Gliedern (z. B. 301 auf eine Seite, deren Canonical woandershin zeigt).
- **Ausgabe:** wie OUT-081; `Redirect Type n` kann zusätzlich `Canonical`/`HTTP Canonical` sein; `Chain Type` dann `Redirect & Canonical` bzw. `Canonical`.
- **Akzeptanzkriterien:**
  - [ ] `/h` 301→ `/i` (Canonical → `/j`, 200, self-canonical) → 2 Glieder, `Final Address`=`/j`.
  - [ ] Seite mit selbstreferenzierendem Canonical bildet kein Kettenglied.
  - [ ] Canonical-Schleife `/k`↔`/l` → `Redirect Loop`=True.
- **Testfixture:** `out-redirects` + `out-canonicals`.

### OUT-084 Redirects: Redirects to Errors
- **Priorität:** P1
- **Beschreibung:** Weiterleitungen/Ketten, deren Endziel ein Fehler ist: robots.txt-gesperrt, keine Antwort, 4xx oder 5xx.
- **Ausgabe:** wie OUT-081.
- **Akzeptanzkriterien:**
  - [ ] Kette `/m` 301→ `/n` 404 enthalten; `/a`→200 nicht.
  - [ ] Weiterleitung auf robots-gesperrte URL enthalten mit `Final Status` = `Blocked by Robots.txt`.
  - [ ] Weiterleitung auf DNS-Fehler-Host enthalten mit `Final Status` = `No Response` / `DNS Lookup Failed`.
- **Testfixture:** `out-redirects`.

### OUT-085 Ketten-Reports im Listenmodus
- **Priorität:** P1
- **Beschreibung:** Im Listenmodus werden für Ketten-Reports alle Sprünge nur dann vollständig verfolgt, wenn „Weiterleitungen immer folgen“ bzw. „Canonicals immer folgen“ aktiviert sind; jede Listen-URL ist Startpunkt einer Zeile.
- **Eingaben/Konfiguration:** Erweiterte Optionen „Always Follow Redirects“, „Always Follow Canonicals“.
- **Ausgabe:** wie OUT-081, Reihenfolge = Upload-Reihenfolge.
- **Akzeptanzkriterien:**
  - [ ] Liste mit `/c` und aktivem „immer folgen“ → vollständige Kette `/c`→`/d`→`/e` im Report, obwohl Crawl-Tiefe 0 ist.
  - [ ] Ohne die Option → nur erster Sprung, `Final Status` = `Not Followed` **(OS-Festlegung)**.
  - [ ] Gleichzeitig mehrere Listen-URLs, die in dieselbe Kette münden → jede eigene Zeile.
- **Testfixture:** `out-redirects` + `out-list-order/redirects.txt`.

### OUT-086 Canonicals: Canonical Chains
- **Priorität:** P1
- **Beschreibung:** Canonical-Ketten mit mindestens 2 Gliedern (A → B, B → C).
- **Ausgabe:** wie OUT-081 mit `Redirect Type n` = `Canonical`/`HTTP Canonical`.
- **Akzeptanzkriterien:**
  - [ ] A (canonical B), B (canonical C), C (self) → eine Zeile für A mit 2 Gliedern; B erscheint nicht als eigene Startzeile, sofern nicht unabhängig verlinkt.
  - [ ] Canonical per HTTP-`Link`-Header wird gleich behandelt.
- **Testfixture:** `out-canonicals`.

### OUT-087 Canonicals: Non-Indexable Canonicals
- **Priorität:** P1
- **Beschreibung:** Canonical-Ziele, die nicht indexierbar sind (Status ≠ 200, noindex, selbst kanonisiert, blockiert), sowie Canonical-Ziele, die ausschließlich über Canonicals gefunden wurden („unlinked“).
- **Ausgabe:** `Source` (Seite mit Canonical), `Canonical`, `Status Code`, `Status`, `Indexability`, `Indexability Status`, `Unlinked` (True/False).
- **Akzeptanzkriterien:**
  - [ ] Canonical auf 404 → enthalten, `Indexability Status`=Client Error.
  - [ ] Canonical auf noindex-Seite → enthalten.
  - [ ] Canonical auf 200-Seite, die nirgends verlinkt ist → enthalten mit `Unlinked`=True, obwohl indexierbar.
- **Testfixture:** `out-canonicals`.

### OUT-088 Pagination: Non-200 Pagination URLs
- **Priorität:** P2
- **Beschreibung:** `rel="next"`/`rel="prev"`-Ziele, die nicht mit 200 antworten.
- **Ausgabe:** `Source`, `Pagination Type` (Next/Prev), `Pagination URL`, `Status Code`, `Status`.
- **Akzeptanzkriterien:**
  - [ ] `rel=next` auf `/page/3` (404) → enthalten.
- **Testfixture:** `out-pagination`.

### OUT-089 Pagination: Unlinked Pagination URLs
- **Priorität:** P2
- **Beschreibung:** Paginierungs-URLs, die nur über `rel=next/prev` und nicht über Hyperlinks entdeckt wurden.
- **Ausgabe:** wie OUT-088 + `Hyperlink Inlinks` (=0).
- **Akzeptanzkriterien:**
  - [ ] `/page/2` nur per `<link rel=next>` erreichbar → enthalten; per `<a>` verlinkt → nicht enthalten.
- **Testfixture:** `out-pagination`.

### OUT-090 Hreflang: All Hreflang URLs
- **Priorität:** P1
- **Beschreibung:** Alle hreflang-Annotationen als 1:1-Zuordnung (Seite → Sprachcode → Ziel-URL), aus HTML, HTTP-Header und XML-Sitemap.
- **Eingaben/Konfiguration:** hreflang speichern/crawlen; Crawl-Analyse.
- **Ausgabe:** `Address`, `Hreflang Language`, `Hreflang URL`, `Annotation Source` (`HTML`/`HTTP Header`/`XML Sitemap`), `Status Code`, `Indexability`, `Is Self Reference` (True/False).
- **Akzeptanzkriterien:**
  - [ ] Seite mit 3 hreflang-Links (de, en, x-default) → 3 Zeilen.
  - [ ] Dieselbe Annotation in HTML und Sitemap → 2 Zeilen mit unterschiedlicher `Annotation Source`.
- **Testfixture:** `out-hreflang`: Seiten de/en/fr mit korrekten Rücklinks, eine ohne Rücklink, eine mit falschem Regionscode, eine mit Rücklink auf kanonisierte Variante, eine mit Rücklink auf noindex-Seite, ein hreflang-Ziel 404, ein unverlinktes Ziel.

### OUT-091 Hreflang: Non-200 Hreflang URLs
- **Priorität:** P1
- **Beschreibung:** hreflang-Ziele, die nicht mit 200 antworten (inkl. Weiterleitung, Blockierung, keine Antwort).
- **Ausgabe:** wie OUT-090 + `Status`.
- **Akzeptanzkriterien:**
  - [ ] Ziel 404 → enthalten; Ziel 301 → enthalten; Ziel 200 → nicht.
- **Testfixture:** `out-hreflang`.

### OUT-092 Hreflang: Unlinked Hreflang URLs
- **Priorität:** P1
- **Beschreibung:** hreflang-Ziele, die nicht über Hyperlinks im Crawl gefunden wurden.
- **Ausgabe:** wie OUT-090.
- **Akzeptanzkriterien:**
  - [ ] `/fr/` nur per hreflang referenziert → enthalten (eine Zeile pro referenzierender Seite).
- **Testfixture:** `out-hreflang`.

### OUT-093 Hreflang: Missing Return Links
- **Priorität:** P1
- **Beschreibung:** Seite A verweist per hreflang auf B, aber B verweist nicht per hreflang zurück auf A.
- **Ausgabe:** `Address` (A), `Hreflang Language`, `Hreflang URL` (B), `Hreflang URL Status Code`, `Hreflang URL Indexability`, `Missing Return Link To` (A).
- **Akzeptanzkriterien:**
  - [ ] A→B ohne B→A → eine Zeile.
  - [ ] Ist B nicht 200 oder nicht gecrawlt, wird die Zeile trotzdem ausgegeben, mit Statuscode von B (Referenz ergänzte Statuscodes in 22.1).
  - [ ] Rücklink über eine andere Quelle (z. B. Sitemap statt HTML) zählt als vorhanden.
- **Testfixture:** `out-hreflang`.

### OUT-094 Hreflang: Inconsistent Language & Region Return Links
- **Priorität:** P1
- **Beschreibung:** Rücklinks sind vorhanden, aber der Sprach-/Regionscode, mit dem A die Seite B annotiert, unterscheidet sich vom Code, mit dem B sich selbst annotiert.
- **Ausgabe:** `Address` (A), `Hreflang URL` (B), `Expected Language` (Code laut A), `Actual Language` (Selbstannotation von B).
- **Akzeptanzkriterien:**
  - [ ] A annotiert B als `de-AT`, B annotiert sich selbst als `de-DE` → enthalten.
  - [ ] Vergleich ist case-insensitiv (`de-at` = `de-AT`).
- **Testfixture:** `out-hreflang`.

### OUT-095 Hreflang: Non Canonical Return Links
- **Priorität:** P1
- **Beschreibung:** Rücklinks zeigen auf eine kanonisierte (nicht-kanonische) URL statt auf die kanonische.
- **Ausgabe:** `Address`, `Hreflang Language`, `Hreflang URL`, `Canonical Of Hreflang URL`.
- **Akzeptanzkriterien:**
  - [ ] B verweist zurück auf `/de/?ref=x`, die per Canonical auf `/de/` zeigt → enthalten.
- **Testfixture:** `out-hreflang`.

### OUT-096 Hreflang: Noindex Return Links
- **Priorität:** P1
- **Beschreibung:** Rücklinks zeigen auf Seiten mit noindex.
- **Ausgabe:** `Address`, `Hreflang Language`, `Hreflang URL`, `Indexability Status`.
- **Akzeptanzkriterien:**
  - [ ] Rücklink auf Seite mit `<meta name="robots" content="noindex">` → enthalten.
- **Testfixture:** `out-hreflang`.

### OUT-097 Insecure Content
- **Priorität:** P1
- **Beschreibung:** Alle unsicheren (HTTP-)Verweise auf HTTPS-Seiten, wie in der Referenz: Hyperlinks auf HTTP-URLs – intern **und extern** (z. B. Links auf Social-Media-Profile per `http://`) –, HTTP-Canonicals, HTTP-Paginierungs- und hreflang-Verweise sowie alle per HTTP eingebundenen Ressourcen (Bilder, JS, CSS, Medien, iframes), intern wie extern (z. B. CDN-Bilder).
- **Ausgabe:** Standard-Linkformat (OUT-031), nur Zeilen mit HTTPS-`Source` und HTTP-`Destination`. Zusätzliche Spalte `Internal` (True/False) zur Filterung **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] HTTPS-Seite mit `<a href="http://site.test/x">` (intern) → enthalten, `Internal`=True.
  - [ ] HTTPS-Seite mit `<a href="http://social.test/profil">` (externer Hyperlink) → enthalten, `Internal`=False.
  - [ ] HTTPS-Seite mit `<img src="http://cdn.test/p.jpg">` (externe Ressource) → enthalten, `Type`=Image.
  - [ ] HTTPS-Seite mit `<script src="http://cdn.test/a.js">` → enthalten (`Type`=JavaScript).
  - [ ] HTTPS-Seite mit `<link rel="canonical" href="http://site.test/x">` → enthalten (`Type`=Canonical); mit `rel="next"` auf HTTP → enthalten (`Type`=Rel Next).
  - [ ] HTTPS-Seite mit protokollrelativem Verweis `//cdn.test/a.js` → nicht enthalten (wird über HTTPS aufgelöst).
  - [ ] HTTP-Seite mit HTTP-Ressourcen → nicht enthalten.
  - [ ] Externe HTTP-Ziele werden aufgenommen, auch wenn „externe Links prüfen“ deaktiviert ist (Bewertung allein anhand des Schemas; `Status Code` dann leer).
- **Testfixture:** `out-security`: HTTPS-Seiten mit internem HTTP-Link, externem HTTP-Social-Link, HTTP-CDN-Bild, HTTP-Skript, HTTP-Canonical, HTTP-`rel=next`, protokollrelativem Skript; HTTP-Seite mit HTTP-Bild.

### OUT-098 SERP Summary
- **Priorität:** P1
- **Beschreibung:** Titles und Meta Descriptions aller internen HTML-Seiten mit Zeichenlänge und Pixelbreite; die Datei kann im SERP-Modus wieder geladen, bearbeitet und erneut berechnet werden.
- **Ausgabe:** `Address`, `Title 1`, `Title 1 Length`, `Title 1 Pixel Width`, `Meta Description 1`, `Meta Description 1 Length`, `Meta Description 1 Pixel Width`.
- **Akzeptanzkriterien:**
  - [ ] Längen und Pixelbreiten sind identisch mit den Werten in Page Titles/Meta Description (gleiche Berechnung, Schrift und Pixelregeln aus dem On-Page-Bereich).
  - [ ] Der SERP-Modus akzeptiert diese Datei direkt (Spaltenzuordnung `Address`→URL, `Title 1`→Title, `Meta Description 1`→Description) sowie Dateien mit Spalten `URL`, `Title`, `Description`.
  - [ ] Nicht-ASCII-Titel bleiben nach Export/Import unverändert (Rundreise-Test).
- **Testfixture:** `out-serp`: Titles mit 30, 60, 61 Zeichen, mit breiten Zeichen (`WWWW`), Umlauten, CJK.

### OUT-099 Orphan Pages
- **Priorität:** P1
- **Beschreibung:** URLs, die aus Google Analytics, der Search Console oder XML-Sitemaps bekannt sind, aber im Crawl nicht über interne Links gefunden wurden.
- **Eingaben/Konfiguration:** Mindestens eine Quelle: GA4-Verbindung, GSC-Verbindung, Sitemap-Crawl; Crawl-Analyse abgeschlossen.
- **Ausgabe:** `Address`, `Source` ∈ {`GA`, `GSC`, `Sitemap`, `GA & GSC`, `GA & Sitemap`, `GSC & Sitemap`, `GA & GSC & Sitemap`}, `Status Code` (falls die URL über die Quelle gecrawlt wurde), `Indexability`.
- **Akzeptanzkriterien:**
  - [ ] URL nur in Sitemap, nicht verlinkt → `Source`=Sitemap.
  - [ ] URL in GA4 (Mock) und Sitemap → `Source`=`GA & Sitemap`.
  - [ ] Verlinkte URLs erscheinen nie, auch wenn sie in allen Quellen stehen.
  - [ ] Ohne jede Quelle ist der Report deaktiviert mit Hinweis auf die nötigen Integrationen.
  - [ ] URL-Abgleich nach Normalisierung (Schema/Host-Kleinschreibung, Standardport, Fragment entfernt); Trailing-Slash-Unterschiede gelten als verschiedene URLs.
- **Testfixture:** `out-sitemaps-in` + Mock-GA4/GSC mit Orphan-Pfaden.

### OUT-100 Structured Data: Validation Errors & Warnings Summary
- **Priorität:** P1
- **Beschreibung:** Aggregierte Validierungsfunde über alle URLs mit Beispiel-URL.
- **Ausgabe:** `Validation Type` (`Schema.org`/`Google Rich Results`), `Severity` (`Error`/`Warning`), `Type` (Schema-Typ), `Feature` (Rich-Result-Feature), `Issue`, `URLs`, `% of Total`, `Sample URL`.
- **Akzeptanzkriterien:**
  - [ ] Drei Produktseiten ohne `offers` → eine Zeile mit `URLs`=3.
  - [ ] Sortierung: Errors vor Warnings, dann `URLs` absteigend.
- **Testfixture:** `out-structured-data`.

### OUT-101 Structured Data: Validation Errors & Warnings
- **Priorität:** P1
- **Beschreibung:** Dieselben Funde je URL.
- **Ausgabe:** `Address`, `Validation Type`, `Severity`, `Type`, `Feature`, `Issue`, `Property Path`.
- **Akzeptanzkriterien:**
  - [ ] Summe der Zeilen je Issue = `URLs` × Vorkommen im Summary (bei mehrfachem Vorkommen je Seite mehrere Zeilen).
- **Testfixture:** `out-structured-data`.

### OUT-102 Structured Data: Google Rich Results Features Summary
- **Priorität:** P1
- **Beschreibung:** Je erkanntem Rich-Result-Feature Anzahl URLs, mit/ohne Fehler.
- **Ausgabe:** `Feature`, `URLs`, `% of Total`, `Valid`, `With Warnings`, `With Errors`, `Sample URL`.
- **Akzeptanzkriterien:**
  - [ ] Die Liste gültiger Features ist versioniert in `os-analyze` hinterlegt (`rich_result_features.toml` mit Feld `deprecated_since`); ein dort als abgekündigt markiertes Feature wird nicht mehr als Feature gezählt (Test: Fixture-Seite mit Markup eines als abgekündigt markierten Typs → keine Zeile für dieses Feature, Schema.org-Validierung des Markups bleibt aber bestehen). Welche Features abgekündigt sind, richtet sich nach Googles öffentlicher Dokumentation, nicht nach einer Referenzversion.
  - [ ] Seite mit gültigem Breadcrumb und Product mit Warnung → `Breadcrumb`: `Valid`=1; `Product`: `With Warnings`=1; `Valid` + `With Warnings` + `With Errors` = `URLs` je Feature.
- **Testfixture:** `out-structured-data`.

### OUT-103 Structured Data: Google Rich Results Features
- **Priorität:** P1
- **Beschreibung:** Matrix URL × Feature.
- **Ausgabe:** `Address`, danach eine Spalte je Feature mit Wert `Valid`/`Warnings`/`Errors`/leer.
- **Akzeptanzkriterien:**
  - [ ] Seite mit gültigem Breadcrumb und fehlerhaftem Product → `Breadcrumb`=Valid, `Product`=Errors.
- **Testfixture:** `out-structured-data`.

### OUT-104 PageSpeed: Opportunities Summary
- **Priorität:** P2
- **Beschreibung:** Aggregierte Lighthouse/PSI-Verbesserungsvorschläge (Insight-Audits) über alle URLs.
- **Eingaben/Konfiguration:** PSI-API oder lokale Lighthouse-Integration.
- **Ausgabe:** `Opportunity`, `URLs`, `% of Total`, `Total Savings (ms)`, `Average Savings (ms)`, `Total Savings (Bytes)`, `Average Savings (Bytes)`.
- **Akzeptanzkriterien:**
  - [ ] Werte = Summe/Mittel der Einzelwerte aus dem PageSpeed-Tab (Golden aus Mock-PSI-Antworten).
- **Testfixture:** Mock-PSI-Endpunkt mit 3 festen Antworten.

### OUT-105 PageSpeed: CSS Coverage Summary
- **Priorität:** P2
- **Beschreibung:** Je CSS-Datei ungenutzte Bytes über alle Seiten.
- **Ausgabe:** `Resource`, `Pages`, `Total Bytes`, `Average Unused Bytes`, `Average Unused %`.
- **Akzeptanzkriterien:**
  - [ ] Eine CSS-Datei auf 3 Seiten mit 40/50/60 % ungenutzt → `Pages`=3, `Average Unused %`=50.
- **Testfixture:** Mock-PSI.

### OUT-106 PageSpeed: JavaScript Coverage Summary
- **Priorität:** P2
- **Beschreibung:** Wie OUT-105 für JavaScript-Dateien (ungenutzter JS-Code je Datei über alle Seiten, auf denen sie geladen wird).
- **Ausgabe:** `Resource`, `Pages`, `Total Bytes` (übertragene Größe der Datei), `Average Unused Bytes`, `Average Unused %`, `Sample URL`.
- **Akzeptanzkriterien:**
  - [ ] `app.js` (100 000 Bytes) auf 4 Seiten mit 20/40/60/80 % ungenutzt → `Pages`=4, `Average Unused %`=50, `Average Unused Bytes`=50000.
  - [ ] Inline-Skripte (ohne eigene URL) werden nicht aufgeführt; Skripte mit Query-String-Varianten (`app.js?v=1`, `app.js?v=2`) sind zwei getrennte Zeilen **(OS-Festlegung)**.
  - [ ] Seiten ohne Coverage-Daten (PSI-Fehler) fließen nicht in `Pages` und Mittelwerte ein.
  - [ ] Sortierung: `Average Unused Bytes` × `Pages` absteigend (größtes Einsparpotenzial zuerst) **(OS-Festlegung)**.
- **Testfixture:** Mock-PSI mit 4 Antworten für `app.js` und einer Fehlerantwort; zusätzlich `vendor.js?v=1`/`?v=2`.

### OUT-107 JavaScript: Chrome Console Log Summary
- **Priorität:** P2
- **Beschreibung:** Aggregation der Browser-Konsolenmeldungen beim Rendern.
- **Eingaben/Konfiguration:** JS-Rendering + JS-Fehlerberichte aktiv.
- **Ausgabe:** `Level`, `Message` (URL-spezifische Teile wie Query-Strings nicht normalisiert), `Pages`, `% of Total`, `Sample URL`.
- **Akzeptanzkriterien:**
  - [ ] Dieselbe Fehlermeldung auf 4 Seiten → eine Zeile `Pages`=4.
- **Testfixture:** `out-render`.

### OUT-108 Mobile
- **Priorität:** P2
- **Beschreibung:** Gesammelter Export aller Mobile-Filter und der Lighthouse-Detaildaten.
- **Ausgabe:** Ordner/Arbeitsmappe mit je einer Tabelle pro Mobile-Filter (Spalten wie Tab) sowie `lighthouse_details` im Langformat `Address`, `Audit`, `Score`, `Display Value`, `Details`.
- **Akzeptanzkriterien:**
  - [ ] Anzahl Tabellen = Anzahl Mobile-Filter + 1.
- **Testfixture:** `out-mobile` mit lokalem Lighthouse-Mock.

### OUT-109 Accessibility: Violations Summary
- **Priorität:** P1
- **Beschreibung:** Aggregierte Barrierefreiheitsverstöße.
- **Ausgabe:** `Issue`, `Rule ID`, `Guidelines`, `User Impact`, `Priority`, `URLs`, `% of Total`, `Sample URL`.
- **Akzeptanzkriterien:**
  - [ ] `URLs` zählt Seiten, nicht Vorkommen (Seite mit 5 Bildern ohne alt zählt 1).
  - [ ] Sortierung: Priority (High→Low), dann `URLs` absteigend.
- **Testfixture:** `out-a11y`.

### OUT-110 HTTP Header Summary
- **Priorität:** P1
- **Beschreibung:** Alle vorkommenden Response-Header-Namen mit Anzahl der URLs, die sie liefern.
- **Eingaben/Konfiguration:** HTTP-Header-Extraktion aktiv.
- **Ausgabe:** `Header Name` (Kleinbuchstaben, normalisiert), `URLs`, `% of Total`, `Distinct Values`, `Most Common Value`.
- **Akzeptanzkriterien:**
  - [ ] Header `Cache-Control` auf 10 von 12 URLs → `URLs`=10, `% of Total`=83.33.
  - [ ] `Content-Type` und `content-type` werden zusammengeführt.
- **Testfixture:** `out-headers`.

### OUT-111 Cookie Summary
- **Priorität:** P1
- **Beschreibung:** Eindeutige Cookies über den Crawl – nützlich für Datenschutz-/Consent-Prüfungen.
- **Eingaben/Konfiguration:** Cookie-Extraktion aktiv (mit Rendering für JS-Cookies).
- **Ausgabe:** `Cookie Name`, `Domain`, `Path`, `Persistence` (`Session` oder Laufzeit in Tagen), `Secure`, `HttpOnly`, `SameSite`, `Type` (`HTTP`/`JavaScript`), `First-Party` (True/False), `URLs`, `Sample URL`.
- **Akzeptanzkriterien:**
  - [ ] Eindeutigkeit über (Name, Domain, Path, Secure, HttpOnly, SameSite, Type): derselbe Cookie auf 12 Seiten → eine Zeile `URLs`=12.
  - [ ] Cookie mit `Max-Age=86400` → `Persistence`=1.
  - [ ] Drittanbieter-Cookie aus iframe/Skript einer anderen Domain → `First-Party`=False.
- **Testfixture:** `out-headers`.

### OUT-112 Report-Verfügbarkeit und Voraussetzungen
- **Priorität:** P0
- **Beschreibung:** Das Reports-Menü zeigt jeden Report; nicht erfüllte Voraussetzungen (Integration fehlt, Extraktion nicht aktiviert, Crawl-Analyse fehlt, kein Rendering) werden im Menü und im Export-Log benannt, statt kommentarlos leere Dateien zu erzeugen.
- **Eingaben/Konfiguration:** Crawl-Konfiguration.
- **Ausgabe:** Tooltip/Hinweistext; CLI: Warnung je betroffenem Report.
- **Akzeptanzkriterien:**
  - [ ] Cookie Summary ohne Cookie-Extraktion → Menüeintrag mit Hinweis „Cookies-Extraktion aktivieren“; CLI-Warnung `report 'Cookie Summary' requires cookie extraction`.
  - [ ] Hreflang-Reports ohne abgeschlossene Crawl-Analyse → Hinweis + Angebot „Analyse jetzt starten“.
- **Testfixture:** `out-basic-site` mit minimaler Konfiguration.

### OUT-113 Reports im Vergleichs- und Segmentkontext
- **Priorität:** P1
- **Beschreibung:** Reports berücksichtigen das aktive Segment (OUT-011). Im Vergleichsmodus beziehen sich Reports auf den aktuellen Crawl; der Crawl Overview zeigt zusätzlich die Vergleichsspalten (OUT-147).
- **Akzeptanzkriterien:**
  - [ ] Redirect Chains mit Segment „Shop“ → nur Ketten, deren Start-URL im Segment liegt.
  - [ ] Im Vergleichsmodus ist das Reports-Menü bedienbar.
- **Testfixture:** `out-segments`.

---

## 5 XML- und Bild-Sitemaps

Menü „Sitemaps → XML-Sitemap erstellen / Bild-Sitemap erstellen“. Ein Assistent mit Reitern (Seiten, Last Modified, Priority, Change Frequency, Images, Hreflang) erzeugt die Dateien aus dem aktuellen Crawl. Die Einstellungen werden mit dem Konfigurationsprofil gespeichert (OUT-220), damit CLI/Scheduling sie verwenden.

### OUT-120 XML-Sitemap-Erzeugung (Grundfunktion)
- **Priorität:** P0
- **Beschreibung:** Erzeugt aus den gecrawlten internen URLs eine XML-Sitemap nach sitemaps.org-Protokoll 0.9.
- **Eingaben/Konfiguration:** Zielordner, Dateiname (Standard `sitemap.xml`), Optionen OUT-121 – OUT-130.
- **Ausgabe:** `<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">` mit je `<url><loc>…</loc>[<lastmod>][<changefreq>][<priority>]</url>`.
- **Akzeptanzkriterien:**
  - [ ] Standardinhalt: nur interne URLs mit Inhaltstyp HTML, Status 200, indexierbar (kein noindex, nicht kanonisiert auf andere URL, nicht paginiert mit `rel=prev`).
  - [ ] Jede URL erscheint höchstens einmal; Reihenfolge = Crawl-Reihenfolge (Start-URL zuerst).
  - [ ] Robots-gesperrte und nicht gecrawlte URLs (Queue) sind nie enthalten.
  - [ ] Die generierte Datei validiert gegen das offizielle XSD `sitemap.xsd`.
  - [ ] Externe URLs sind nie enthalten.
- **Testfixture:** `out-sitemap-gen`: 20 Seiten inkl. noindex, kanonisierter Seite, paginierter Seite `/list?page=2` mit `rel=prev`, PDF, 301, 404, 500, robots-gesperrtem Pfad, Seite mit Umlaut-Pfad `/über-uns`, Seite mit `&` im Query.

### OUT-121 Sitemap: Einschlussoptionen für Seiten
- **Priorität:** P1
- **Beschreibung:** Optional können zusätzliche Kategorien aufgenommen werden.
- **Eingaben/Konfiguration:** Checkboxen (Standard jeweils aus): „Noindex-Seiten einschließen“, „Kanonisierte Seiten einschließen“, „Paginierte URLs (rel=prev) einschließen“, „PDFs einschließen“, „3xx einschließen“, „4xx einschließen“, „5xx einschließen“, „Keine Antwort einschließen“.
- **Ausgabe:** erweiterte `<url>`-Liste.
- **Akzeptanzkriterien:**
  - [ ] Je Checkbox einzeln aktiviert → genau die betreffenden URLs kommen hinzu (6 Einzeltests + 1 Test „alle an“).
  - [ ] „PDFs einschließen“ nimmt interne `application/pdf`-URLs mit Status 200 auf (sofern nicht zugleich eine andere Ausschlussregel greift).
  - [ ] Eine Seite, die sowohl noindex als auch kanonisiert ist, erscheint nur, wenn beide Optionen aktiv sind.
- **Testfixture:** `out-sitemap-gen`.

### OUT-122 Sitemap: Ausschluss einzelner URLs
- **Priorität:** P1
- **Beschreibung:** Vor der Erzeugung entfernte URLs (Rechtsklick → Entfernen im Internal-Tab) sowie per Exclude/robots.txt nie gecrawlte URLs erscheinen nicht. Zusätzlich: Regex-Ausschlussliste im Assistenten **(OS-Festlegung)**.
- **Eingaben/Konfiguration:** Regex-Liste (eine pro Zeile).
- **Akzeptanzkriterien:**
  - [ ] Nach Entfernen von `/impressum` fehlt sie in der Sitemap.
  - [ ] Regex `\?sessionid=` schließt URLs mit Session-Parameter aus.
- **Testfixture:** `out-sitemap-gen`.

### OUT-123 Sitemap: lastmod
- **Priorität:** P1
- **Beschreibung:** Optionales `<lastmod>`-Element.
- **Eingaben/Konfiguration:** „lastmod aufnehmen“ (Standard an); Quelle: „Server-Antwort (Last-Modified)“ (Standard) oder „Benutzerdefiniertes Datum“ (Datumsauswahl, gilt für alle URLs).
- **Ausgabe:** W3C-Datetime, Format `YYYY-MM-DD` bei benutzerdefiniertem Datum, `YYYY-MM-DDThh:mm:ss+00:00` bei Server-Datum **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Server-Quelle: URL mit `Last-Modified: Tue, 01 Oct 2026 10:00:00 GMT` → `<lastmod>2026-10-01T10:00:00+00:00</lastmod>`.
  - [ ] Server-Quelle, kein/ungültiger Header → kein `<lastmod>` für diese URL (nicht das Crawldatum).
  - [ ] Benutzerdefiniert `2026-10-13` → alle URLs `<lastmod>2026-10-13</lastmod>`.
  - [ ] Abgeschaltet → kein `<lastmod>` in der Datei.
- **Testfixture:** `out-sitemap-gen` (Fixture-Server setzt `Last-Modified` für ausgewählte Seiten, einmal ungültig).

### OUT-124 Sitemap: priority
- **Priorität:** P1
- **Beschreibung:** Optionales `<priority>` abhängig von der Crawl-Tiefe.
- **Eingaben/Konfiguration:** „priority aufnehmen“ (Standard an); Tabelle Tiefe → Wert, Standard: Tiefe 0 = 1.0, 1 = 0.9, 2 = 0.8, 3 = 0.7, 4 = 0.6, ≥ 5 = 0.5; alle Werte editierbar (0.0–1.0, Schritt 0.1).
- **Ausgabe:** `<priority>` mit einer Nachkommastelle.
- **Akzeptanzkriterien:**
  - [ ] Start-URL → `1.0`; Seite in Tiefe 7 → `0.5`.
  - [ ] Eingabe 1.5 wird abgelehnt (Validierungsfehler im Dialog).
  - [ ] Im Listenmodus (alle Tiefe 0) → alle `1.0`, mit Hinweis im Dialog.
  - [ ] Abgeschaltet → kein `<priority>`.
- **Testfixture:** `out-sitemap-gen` (lineare Kette bis Tiefe 7).

### OUT-125 Sitemap: changefreq
- **Priorität:** P1
- **Beschreibung:** Optionales `<changefreq>`.
- **Eingaben/Konfiguration:** „changefreq aufnehmen“ (Standard an); Basis: „Last-Modified-Header“ oder „Crawl-Tiefe“. Bei Tiefe: Tabelle Tiefe → Wert (`always`, `hourly`, `daily`, `weekly`, `monthly`, `yearly`, `never`), Standard **(OS-Festlegung)**: 0 = daily, 1 = daily, 2 = weekly, 3 = weekly, ≥ 4 = monthly.
- **Ausgabe:** `<changefreq>`.
- **Akzeptanzkriterien:**
  - [ ] Basis Last-Modified: Header jünger als 24 h relativ zum Crawlzeitpunkt der URL → `daily`; älter oder fehlend → `monthly`.
  - [ ] Grenzfall: genau 24 h → `monthly` (strikt „innerhalb der letzten 24 h“).
  - [ ] Basis Tiefe: Werte gemäß Tabelle.
- **Testfixture:** `out-sitemap-gen` (Fixture-Server setzt `Last-Modified` dynamisch auf jetzt − 1 h bzw. jetzt − 48 h).

### OUT-126 Sitemap: Bilder in der XML-Sitemap
- **Priorität:** P1
- **Beschreibung:** Optional werden Bilder je Seite als `<image:image><image:loc>` aufgenommen (Namespace `http://www.google.com/schemas/sitemap-image/1.1`).
- **Eingaben/Konfiguration:** „Bilder aufnehmen“ (Standard aus); „Externe Bilder (CDN) einschließen per Regex“ (leer = keine externen); „Nur relevante Bilder mit bis zu N IMG-Inlinks aufnehmen“ (Standard deaktiviert; dient dem Ausschluss seitenweiter Logos/Icons). Die Schwelle ist – wie in der Referenz – eine inklusive Obergrenze der Anzahl einbindender Seiten (`IMG Inlinks`); OpenSpider übernimmt das Referenzverhalten unverändert. Option „noindex-Bilder ausschließen“ (P2).
- **Ausgabe:** `<url>` mit 0..n `<image:image>`.
- **Akzeptanzkriterien:**
  - [ ] Interne Bilder mit Status 200 werden der einbindenden Seite zugeordnet; Bilder mit Status ≠ 200 nie.
  - [ ] Externes Bild `https://cdn.test/p.jpg` erscheint nur, wenn es auf die Regex `cdn\.test` passt.
  - [ ] Schwelle N = 5: Logo auf 20 Seiten (20 IMG-Inlinks) fehlt, Produktbild mit 1 Inlink ist enthalten; Grenzfälle: Bild mit genau 5 IMG-Inlinks enthalten, mit 6 nicht.
  - [ ] Max. 1 000 Bilder je `<url>`; weitere werden ausgelassen und gewarnt.
- **Testfixture:** `out-sitemap-images`: Logo auf allen 20 Seiten, je Produktseite ein Bild, ein Banner auf genau 5 und eines auf genau 6 Seiten, ein CDN-Bild, ein 404-Bild.

### OUT-127 Bild-Sitemap (separat)
- **Priorität:** P1
- **Beschreibung:** Eigene Sitemap, die nur Seiten enthält, die mindestens ein aufzunehmendes Bild einbinden, mit den Bildern.
- **Eingaben/Konfiguration:** wie OUT-126 (Regex, Inlink-Schwelle); Seitenregeln wie OUT-120/121.
- **Ausgabe:** `image-sitemap.xml` **(OS-Festlegung** Standardname) im gleichen Format wie OUT-126.
- **Akzeptanzkriterien:**
  - [ ] Seiten ohne (verbleibende) Bilder fehlen.
  - [ ] Nur Bilder mit Status 200.
  - [ ] Aufteilung wie OUT-129.
- **Testfixture:** `out-sitemap-images`.

### OUT-128 Sitemap: hreflang
- **Priorität:** P1
- **Beschreibung:** Optional werden die gecrawlten hreflang-Annotationen jeder URL als `<xhtml:link rel="alternate" hreflang="…" href="…"/>` (Namespace `http://www.w3.org/1999/xhtml`) ausgegeben. OpenSpider erzeugt keine hreflang-Beziehungen, sondern übernimmt vorhandene aus HTML/HTTP-Header.
- **Eingaben/Konfiguration:** „hreflang aufnehmen“ (Standard aus); Option „nur hreflang-Ziele mit Status 200 und indexierbar“ (Standard an) **(OS-Festlegung)**.
- **Ausgabe:** `<xhtml:link>`-Elemente je `<url>`.
- **Akzeptanzkriterien:**
  - [ ] Seite `/de/` mit Annotationen de, en, x-default → 3 `xhtml:link`-Elemente inkl. Selbstverweis.
  - [ ] hreflang-Ziel 404 wird bei aktiver Filteroption nicht aufgenommen.
  - [ ] Ohne hreflang-Extraktion im Crawl ist die Option deaktiviert.
- **Testfixture:** `out-hreflang`.

### OUT-129 Sitemap: Aufteilung und Sitemap-Index
- **Priorität:** P0
- **Beschreibung:** Überschreitet die Sitemap die Protokollgrenzen, werden mehrere Dateien plus ein Sitemap-Index erzeugt.
- **Eingaben/Konfiguration:** max. URLs je Datei (Standard 49 999 wie Referenz, max. 50 000); Basis-URL, unter der die Dateien später liegen (für `<loc>` im Index; Standard = Start-URL-Origin + `/`).
- **Ausgabe:** `sitemap_1.xml`, `sitemap_2.xml`, … und `sitemap_index.xml` mit `<sitemapindex>`/`<sitemap><loc>` (+ `<lastmod>` = Erzeugungsdatum) **(OS-Festlegung** Dateinamen).
- **Akzeptanzkriterien:**
  - [ ] 49 999 URLs → genau eine Datei `sitemap.xml`, kein Index.
  - [ ] 50 000 URLs → zwei Dateien (49 999 + 1) und Index.
  - [ ] Zusätzlich wird geteilt, wenn eine Datei unkomprimiert 50 MiB (52 428 800 Bytes) überschreiten würde.
  - [ ] Index validiert gegen `siteindex.xsd`; `<loc>` sind absolute URLs.
  - [ ] Bild-Sitemaps zählen `<url>`-Einträge, nicht Bilder, für die Grenze.
- **Testfixture:** synthetischer Crawl mit 50 000 bzw. 120 000 URLs (generiert in `os-store`), plus Crawl mit sehr langen URLs für die Bytegrenze.

### OUT-130 Sitemap je Segment
- **Priorität:** P2
- **Beschreibung:** Bei konfigurierten Segmenten kann je Segment eine eigene Sitemap erzeugt werden, plus ein Index über alle.
- **Eingaben/Konfiguration:** „Nach Segment erstellen“ (nur sichtbar, wenn Segmente existieren).
- **Ausgabe:** `sitemap_<segment>.xml` (+ ggf. `_1`, `_2` bei Teilung), `sitemap_index.xml`.
- **Akzeptanzkriterien:**
  - [ ] Segmente Blog/Shop → zwei Sitemaps + Index; URLs ohne Segment landen in `sitemap_unsegmented.xml` **(OS-Festlegung)**.
  - [ ] Segmentnamen werden für Dateinamen bereinigt (OUT-008); Bindestriche bleiben erlaubt.
- **Testfixture:** `out-segments`.

### OUT-131 Sitemap: XML-Korrektheit und Kodierung
- **Priorität:** P0
- **Beschreibung:** Korrekte Kodierung von URLs und Sonderzeichen.
- **Akzeptanzkriterien:**
  - [ ] Datei ist UTF-8 mit Deklaration `<?xml version="1.0" encoding="UTF-8"?>`, ohne BOM.
  - [ ] `&` in URLs wird als `&amp;` maskiert; `'`, `"`, `<`, `>` ebenfalls entitätskodiert.
  - [ ] Nicht-ASCII-Pfade werden prozentkodiert ausgegeben (`/über-uns` → `/%C3%BCber-uns`), IDN-Hosts in Punycode.
  - [ ] URLs über 2 048 Zeichen werden ausgelassen und im Erzeugungsbericht aufgeführt.
  - [ ] Erzeugungsbericht nach Abschluss: Anzahl URLs, Dateien, ausgelassene URLs mit Grund.
- **Testfixture:** `out-sitemap-gen`.

### OUT-132 Sitemap: gzip-Ausgabe
- **Priorität:** P2
- **Beschreibung:** Optional werden Sitemaps zusätzlich bzw. ausschließlich als `.xml.gz` geschrieben **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Entpackte `.gz`-Datei ist byte-identisch zur unkomprimierten Variante.
  - [ ] Im Index verweisen `<loc>` auf `.xml.gz`, wenn nur komprimiert geschrieben wird.
- **Testfixture:** `out-sitemap-gen`.

### OUT-133 Sitemaps in CLI und Scheduling
- **Priorität:** P1
- **Beschreibung:** Sitemap- und Bild-Sitemap-Erzeugung nach Crawl-Ende per CLI (`--create-sitemap`, `--create-images-sitemap`) und als „lokale Datei“ in geplanten Aufgaben; Einstellungen kommen aus der Konfigurationsdatei.
- **Akzeptanzkriterien:**
  - [ ] `openspider --crawl {BASE} --headless --output-folder out --create-sitemap` erzeugt `out/sitemap.xml`, identisch zur UI-Erzeugung mit derselben Konfiguration.
  - [ ] Ohne Sitemap-Einstellungen in der Konfigurationsdatei gelten die Standardwerte aus OUT-120–OUT-129.
- **Testfixture:** `out-sitemap-gen`.

---

## 6 Crawl-Vergleich

Voraussetzung: beide Crawls liegen als Crawl-Dateien (`*.ospider`) in der Crawl-Verwaltung vor (OUT-228). Da OpenSpider immer auf Disk speichert, entfällt die Einschränkung der Referenz auf einen bestimmten Speichermodus.

### OUT-140 Compare-Modus: Crawls auswählen
- **Priorität:** P1
- **Beschreibung:** Zwei Wege: (a) in der Crawl-Verwaltung zwei Crawls markieren → „Zum Vergleich wählen“; (b) Modus → Compare, dann je eine Auswahl für „Current“ und „Previous“. Standardmäßig ist der neuere Crawl „Current“.
- **Eingaben/Konfiguration:** zwei Crawl-IDs.
- **Ausgabe:** Vergleichsansicht (alle Tabs/Filter mit Vergleichsspalten).
- **Akzeptanzkriterien:**
  - [ ] Auswahl in beliebiger Reihenfolge → der Crawl mit späterem Startzeitpunkt ist Current.
  - [ ] Mehr oder weniger als zwei markierte Crawls → Schaltfläche deaktiviert.
  - [ ] Vergleich von Crawls unterschiedlicher Modi (Spider vs. Liste) ist erlaubt; ein Hinweis weist darauf hin.
  - [ ] Crawls aus älteren Dateiversionen werden vor dem Vergleich migriert (Schema-Migration aus `os-store`), ohne das Original zu verändern.
- **Testfixture:** `out-compare`: Fixture-Server mit Umschalter `?version=1|2` (Version 2: 3 Seiten neu, 2 gelöscht, 1 Title geändert, 1 Seite neu noindex, 1 Link entfernt → Tiefe ändert sich, 1 404 behoben).

### OUT-141 Current/Previous tauschen
- **Priorität:** P1
- **Beschreibung:** Eine Schaltfläche vertauscht die Rollen der beiden Crawls; alle Zähler werden neu berechnet.
- **Akzeptanzkriterien:**
  - [ ] Nach Tausch gilt: neues Added = altes Removed, neues New = altes Missing (und umgekehrt).
- **Testfixture:** `out-compare`.

### OUT-142 Vergleichsklassifikation je Filter: Added / New / Removed / Missing
- **Priorität:** P1
- **Beschreibung:** Für jeden Filter F jedes Tabs wird jede URL u klassifiziert (nach URL-Mapping, OUT-146):
  - **Added:** u existiert in beiden Crawls, war in Previous **nicht** in F, ist in Current in F.
  - **New:** u existiert nur in Current und ist dort in F.
  - **Removed:** u existiert in beiden Crawls, war in Previous in F, ist in Current nicht mehr in F.
  - **Missing:** u existiert nur in Previous und war dort in F.
- **Ausgabe:** Im Overview je Filter die Spalten `Current`, `Previous`, `Change`, `Added`, `New`, `Removed`, `Missing`; in der Haupttabelle Unterfilter mit diesen vier Kategorien.
- **Akzeptanzkriterien:**
  - [ ] Invariante für jeden Filter: `Current − Previous = Added + New − Removed − Missing` (automatisierter Test über alle Filter).
  - [ ] Im Fixture: Title-geänderte Seite, die vorher „Duplicate“ war und jetzt nicht mehr → Removed in „Page Titles:Duplicate“.
  - [ ] Behobene 404 → Removed in „Response Codes:Client Error (4xx)“ und Added in „Success (2xx)“.
  - [ ] Gelöschte Seite (nicht mehr verlinkt, nicht gecrawlt) → Missing in „Internal:HTML“.
  - [ ] Neue Seite → New in „Internal:HTML“.
  - [ ] „Existiert in Crawl“ heißt: URL wurde gecrawlt (nicht nur entdeckt/blockiert) **(OS-Festlegung)**.
- **Testfixture:** `out-compare`.

### OUT-143 Vergleich im Issues-Panel
- **Priorität:** P1
- **Beschreibung:** Das Issues-Panel zeigt je Issue Current, Previous, Change sowie die vier Kategorien; Diagramme reagieren auf Klicks auf Tabs/Filter/Zahlen.
- **Akzeptanzkriterien:**
  - [ ] Klick auf die Zahl in „Added“ eines Issues filtert die Haupttabelle auf genau diese URLs.
  - [ ] Issues, die nur in einem Crawl existieren, erscheinen mit 0 im anderen.
- **Testfixture:** `out-compare`.

### OUT-144 Change Detection (Elementänderungen)
- **Priorität:** P1
- **Beschreibung:** Für URLs, die in beiden Crawls existieren, wird ermittelt, ob sich bestimmte Elemente geändert haben. Ein eigener Tab „Change Detection“ (oben und im rechten Overview als letzter Eintrag) zeigt je Element die Anzahl geänderter URLs.
- **Eingaben/Konfiguration:** Konfiguration → Compare → Change Detection: Elemente einzeln an/aus (Standard: alle aus außer Indexability, Page Titles, Meta Description, H1 **(OS-Festlegung)**). Elemente in dieser Reihenfolge (entspricht der Filterreihenfolge der Referenz):
  1. `Indexability` (Vergleich von `Indexability` **und** `Indexability Status`),
  2. `Page Titles`, 3. `Meta Description`, 4. `H1`, 5. `Word Count`, 6. `Crawl Depth`,
  7. `Inlinks` (Anzahl aller internen Inlinks, inkl. Mehrfachlinks derselben Quelle), 8. `Unique Inlinks` (Anzahl eindeutiger Quellseiten),
  9. `Internal Outlinks` (alle), 10. `Unique Internal Outlinks` (eindeutige Ziele),
  11. `External Outlinks` (alle), 12. `Unique External Outlinks` (eindeutige Ziele),
  13. `Structured Data Unique Types`, 14. `Content` (OUT-145).
- **Ausgabe:** Overview-Zeile je Element mit `Current`, `Previous`, `Change` (alle drei = Anzahl geänderter URLs, wie in der Referenz) und Filter `All` (= Anzahl in beiden Crawls vorhandener, analysierter URLs). Haupttabelle je Element: `Address`, `Current <Element>`, `Previous <Element>`, bei Zahlen zusätzlich `Change` (absolut) und `Change %`; Filterumschalter `Changed`/`Not Changed`.
- **Akzeptanzkriterien:**
  - [ ] Textelemente werden nach Whitespace-Normalisierung (trim, Mehrfach-Leerzeichen → eines) verglichen; Groß-/Kleinschreibung zählt als Änderung.
  - [ ] Element vorher fehlend, jetzt vorhanden (oder umgekehrt) → geändert.
  - [ ] H1: verglichen wird die geordnete Liste aller H1; Reihenfolgeänderung = geändert.
  - [ ] Indexability: Seite vorher `Indexable`, jetzt `Non-Indexable` (noindex) → geändert; Seite in beiden Crawls `Non-Indexable`, aber Grund wechselt von `Noindex` zu `Canonicalised` → geändert (Statusvergleich); Seite in beiden `Indexable` → nicht geändert. Tabelle zeigt `Current Indexability`, `Previous Indexability`, `Current Indexability Status`, `Previous Indexability Status`.
  - [ ] Word Count / alle sechs Link-Elemente / Crawl Depth: jede Abweichung ≠ 0 = geändert; `Change %` = (Current − Previous) / Previous × 100, bei Previous = 0 leer.
  - [ ] Inlinks vs. Unique Inlinks: Startseite verlinkt `/x` jetzt zweimal statt einmal (sonst unverändert) → `Inlinks` geändert (1 → 2), `Unique Inlinks` nicht geändert (1 → 1). Analog für Internal/External Outlinks vs. Unique-Varianten.
  - [ ] Structured Data Unique Types: Vergleich der Mengen der `@type`-Werte (Reihenfolge egal).
  - [ ] URLs nur in einem Crawl sind ausgeschlossen (sie stehen unter New/Missing).
  - [ ] Ausgeschaltete Elemente erscheinen nicht im Tab; ist kein Element aktiv, fehlt der Tab.
  - [ ] Sortierung nach `Change` und `Change %` möglich.
  - [ ] Registry-Test: genau 14 Elemente in der oben genannten Reihenfolge; jedes ist einzeln abschaltbar.
- **Testfixture:** `out-compare` (Indexability, Title, Description, H1-Reihenfolge, Wortanzahl, Tiefe, doppelter Link auf `/x`, neuer externer Link, Schema-Typen ändern sich gezielt auf je einer Seite; eine Seite wechselt von noindex zu kanonisiert).

### OUT-145 Content Change Detection
- **Priorität:** P2
- **Beschreibung:** Inhaltliche Änderung des Hauptinhalts als Prozentwert über eine MinHash-Ähnlichkeitsschätzung. Erfordert in beiden Crawls gespeichertes HTML bzw. gespeicherte Inhalts-Signaturen.
- **Eingaben/Konfiguration:** Schwelle in Konfiguration → Compare: als geändert gilt eine Seite, deren Inhalt sich um **mehr als** X % geändert hat; Standard X = 10 % wie in der Referenz (dort als Ähnlichkeitsschwelle unter Config → Compare einstellbar). Wertebereich 0–100, ganzzahlig.
- **Ausgabe:** `Content Change %` (= 100 − geschätzte Ähnlichkeit in %, 2 Nachkommastellen), Filter Changed/Not Changed.
- **Akzeptanzkriterien:**
  - [ ] Identischer Inhalt → 0 %, Not Changed.
  - [ ] Standardschwelle 10 %: berechnete Änderung 10,00 % → Not Changed; 10,01 % → Changed (strikt „größer als“). Grenzfalltest über injizierte Ähnlichkeitswerte (Unit-Test der Klassifikation, unabhängig von der MinHash-Schätzung).
  - [ ] Seite mit ca. 30 % ersetztem Text → Changed bei Standardschwelle; nach Änderung der Schwelle auf 40 % → Not Changed, ohne erneutes Crawlen (Neuklassifikation aus gespeicherten Signaturen).
  - [ ] Hälfte des Textes ersetzt → zwischen 35 % und 65 % (Toleranz wegen Approximation; fester Seed → deterministisch).
  - [ ] Fehlen Inhalts-Signaturen in einem Crawl → Element deaktiviert mit Hinweis.
- **Testfixture:** `out-compare` (Seite mit 50 % neuem Text, Seite mit ca. 30 % neuem Text, Seite mit nur geändertem Footer → bei Inhaltsbereich ohne Footer 0 %).

### OUT-146 URL-Mapping zwischen Crawls
- **Priorität:** P1
- **Beschreibung:** Regex-Ersetzungsregeln bilden URLs des Previous-Crawls auf URLs des Current-Crawls ab, damit z. B. Staging gegen Live, Mobil-Unterordner gegen Desktop oder URLs mit/ohne Trailing Slash verglichen werden können. Gilt für Overview, Issues, Site Structure und Change Detection.
- **Eingaben/Konfiguration:** Liste von Regeln `Suchmuster (Regex)` → `Ersetzung` (mit `$1`-Gruppen), beliebig viele, nacheinander angewendet; Reiter „Test“ mit Eingabe-URL und Ergebnisanzeige.
- **Akzeptanzkriterien:**
  - [ ] Regel `^https://staging\.site\.test` → `https://www.site.test`: Staging-URL `/a` wird als dieselbe URL wie Live-`/a` behandelt (keine New/Missing-Einträge für gemappte Paare).
  - [ ] Regel `/$` → `` (Trailing Slash entfernen) mappt `/a/` auf `/a`.
  - [ ] Mehrere Regeln werden in Listenreihenfolge auf das Ergebnis der vorigen angewendet.
  - [ ] Bilden zwei Previous-URLs auf dieselbe Current-URL ab, wird die erste verwendet und eine Warnung angezeigt **(OS-Festlegung)**.
  - [ ] Ungültige Regex → Fehlermeldung im Dialog, Speichern nicht möglich.
  - [ ] Test-Reiter zeigt für eine Eingabe die gemappte URL sofort an.
- **Testfixture:** `out-compare-mapping`: zwei Hosts (`staging.site.test`, `www.site.test`) mit gleicher Struktur, einer mit Trailing Slashes.

### OUT-147 Crawl-Overview-Export im Vergleichsmodus
- **Priorität:** P1
- **Beschreibung:** Reports → Crawl Overview im Vergleichsmodus enthält je Zeile die Vergleichswerte.
- **Ausgabe:** `Section`, `Item`, `Current`, `Previous`, `Change`, `Change %`, `Added`, `New`, `Removed`, `Missing`, zusätzlich Kopfblock mit Name/Startzeit beider Crawls.
- **Akzeptanzkriterien:**
  - [ ] Werte identisch mit dem Overview-Panel im Vergleichsmodus.
  - [ ] Change-Detection-Elemente erscheinen als eigener Abschnitt.
- **Testfixture:** `out-compare`.

### OUT-148 Site Structure im Vergleich
- **Priorität:** P2
- **Beschreibung:** Verzeichnisbaum mit Änderungen je Verzeichnis: Anzahl hinzugekommener (grüner Pfeil nach oben) und weggefallener URLs (roter Pfeil nach unten), aufklappbar bis zur URL; Darstellung der Crawl-Tiefen-Verschiebung.
- **Ausgabe:** Export: `Path`, `Current URLs`, `Previous URLs`, `New`, `Missing`, `Average Depth Current`, `Average Depth Previous`.
- **Akzeptanzkriterien:**
  - [ ] Verzeichnis `/blog/` mit 2 neuen und 1 fehlenden URL → `New`=2, `Missing`=1.
- **Testfixture:** `out-compare`.

### OUT-149 Inlinks/Outlinks-Vergleich im unteren Fenster
- **Priorität:** P2
- **Beschreibung:** Detail-Tabs Inlinks/Outlinks haben im Vergleichsmodus einen Umschalter `Auto` (folgt dem oberen Fenster), `Current`, `Previous` und Filter `Added`/`Removed` für Links, die zwischen den Crawls hinzugekommen/weggefallen sind.
- **Akzeptanzkriterien:**
  - [ ] Seite, deren Tiefe von 1 auf 2 stieg, zeigt unter Inlinks → Removed den entfernten Link von der Startseite.
  - [ ] Link-Identität: (Source, Destination, Type, Anchor) nach URL-Mapping **(OS-Festlegung)**.
- **Testfixture:** `out-compare`.

### OUT-150 Gespeicherte Vergleiche
- **Priorität:** P2
- **Beschreibung:** Ein abgeschlossener Vergleich wird als kleiner Eintrag (Modus „Compare“) in der Crawl-Verwaltung gespeichert und kann ohne Neuberechnung geöffnet werden. Gespeichert werden: IDs von Current/Previous, die beim Vergleich wirksame Compare-Konfiguration (Change-Detection-Elemente, Content-Schwelle, URL-Mapping) und die berechneten Klassifikationen (Added/New/Removed/Missing je Filter, Change-Detection-Ergebnisse) **(OS-Festlegung** des Umfangs).
- **Eingaben/Konfiguration:** automatisch nach jedem Vergleich (UI, Auto-Compare, CLI); Name Standard `<Projekt> – <Name Current> vs <Name Previous>`, umbenennbar.
- **Ausgabe:** Zeile in der Crawl-Verwaltung mit `Mode`=Compare, `Name`, `Current`, `Previous`, `Created`, `Size on Disk`, `Status` (`Available`/`Unavailable`).
- **Akzeptanzkriterien:**
  - [ ] Öffnen eines gespeicherten Vergleichs dauert < 2 s bei 100 000 URLs je Crawl (keine Neuberechnung; Messung: es wird keine Klassifikationsabfrage über beide Crawls ausgeführt).
  - [ ] Zähler des geöffneten gespeicherten Vergleichs sind identisch mit denen unmittelbar nach der Berechnung (Golden-Vergleich des Crawl Overview, OUT-147).
  - [ ] Nach Änderung der Compare-Konfiguration (z. B. URL-Mapping) bleibt der gespeicherte Vergleich unverändert; „Neu berechnen“ erzeugt einen neuen Eintrag, der alte bleibt erhalten.
  - [ ] Wird einer der beiden Crawls gelöscht, ist der Vergleich als `Unavailable` markiert und nicht öffenbar; der Löschdialog des Crawls nennt die betroffenen Vergleiche namentlich.
  - [ ] Filter „Anzeigen: Vergleiche“ in der Crawl-Verwaltung listet nur Vergleiche; „Anzeigen: Crawls“ keine.
  - [ ] Speicherbedarf eines gespeicherten Vergleichs ≤ 10 % der Summe beider Crawl-Dateien (Fixture mit 100 000 URLs).
- **Testfixture:** `out-compare`; synthetisches Crawlpaar mit je 100 000 URLs für Lade- und Größenmessung.

### OUT-151 Tab-Exporte im Vergleichsmodus
- **Priorität:** P1
- **Beschreibung:** Exporte aus der Vergleichsansicht enthalten die Vergleichsinformation.
- **Ausgabe:** Zusätzlich zu den Spalten des Current-Crawls die Spalte `Comparison Status` (`Added`/`New`/`Removed`/`Missing`/`Unchanged`) und für Change-Detection-Ansichten die `Previous …`-Spalten **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Export „Response Codes:Client Error (4xx)“ mit Unterfilter „Removed“ enthält die behobene 404 mit `Comparison Status`=Removed.
  - [ ] Bulk-Exporte und Reports beziehen sich auf den Current-Crawl.
- **Testfixture:** `out-compare`.

### OUT-152 Automatischer Vergleich (Auto Compare) für Scheduling und CLI
- **Priorität:** P1
- **Beschreibung:** Nach Abschluss eines geplanten oder CLI-Crawls wird automatisch mit dem vorherigen Crawl desselben Projekts verglichen; der Vergleich wird gespeichert (OUT-150) und kann in die Abschlussmail (OUT-282) einfließen.
- **Eingaben/Konfiguration:** Aufgaben-Option „Mit vorherigem Crawl vergleichen“; CLI `--project-crawl-comparison ["true"|"false"]` (vergleicht die letzten beiden Crawls eines Projekts; das Argument ist optional, die Referenz-Syntax mit `"true"` wird ebenfalls akzeptiert, siehe OUT-202) bzw. `--crawl-comparison <idA> <idB>`.
- **Akzeptanzkriterien:**
  - [ ] Erster Crawl eines Projekts → kein Vergleich, Hinweis im Log, kein Fehler.
  - [ ] Zweiter Crawl → gespeicherter Vergleich mit den beiden letzten Crawls des Projekts (nach Startzeit).
  - [ ] Change-Detection-Einstellungen und URL-Mapping kommen aus der Konfigurationsdatei.
  - [ ] Exporte (`--export-tabs` usw.) können im Vergleichskontext ausgeführt werden und tragen dann die Vergleichsspalten (OUT-151).
- **Testfixture:** `out-compare` via CLI-Integrationstest (zwei Läufe mit Projektname).

### OUT-153 Paritätsprüfungen über Vergleich
- **Priorität:** P2
- **Beschreibung:** Der Vergleich dient auch für Paritätschecks zwischen Konfigurationen: Mobil- vs. Desktop-User-Agent, rohes vs. gerendertes HTML, verschiedene User-Agents. Im Detail-Tab „Rendered Page“ lassen sich die Screenshots beider Crawls nebeneinander zeigen.
- **Akzeptanzkriterien:**
  - [ ] Zwei Crawls derselben Fixture mit unterschiedlichem User-Agent (Fixture liefert für Mobil-UA einen anderen Title) → Change Detection meldet die Title-Änderung.
  - [ ] Nebeneinander-Ansicht nur verfügbar, wenn beide Crawls Screenshots enthalten.
- **Testfixture:** `out-compare-ua`: Fixture-Server variiert Title nach User-Agent.

---

## 7 Geplante Crawls (Scheduling)

Menü „Datei → Zeitplanung“. Eine geplante Aufgabe ist eine Beschreibung von Crawl + Exporten, die zu festgelegten Zeiten über das CLI-Binary (Abschnitt 8) ausgeführt wird. Aufgaben werden als Dateien im Einstellungsordner gespeichert (`schedules/<task-id>.ostask`, TOML) **(OS-Festlegung)**.

### OUT-160 Aufgabenverwaltung
- **Priorität:** P1
- **Beschreibung:** Liste aller Aufgaben mit Spalten `Name`, `Project`, `Next Run`, `Interval`, `Mode`, `Start URL / Source`, `Status` (Valid/Invalid), `Last Run Result`. Aktionen: Hinzufügen, Bearbeiten, Duplizieren, Löschen, Jetzt ausführen, Historie.
- **Eingaben/Konfiguration:** –
- **Ausgabe:** Aufgabendateien + Registrierung beim Betriebssystem (OUT-166).
- **Akzeptanzkriterien:**
  - [ ] Hinzufügen → Aufgabe erscheint in der Liste und ist im OS-Planer registriert (macOS: LaunchAgent-plist vorhanden; Windows: Eintrag in der Aufgabenplanung unter Ordner `\OpenSpider\`).
  - [ ] Löschen → Aufgabendatei und OS-Registrierung entfernt; bereits erzeugte Crawls und Exporte bleiben erhalten.
  - [ ] „Jetzt ausführen“ startet sofort einen Lauf, unabhängig vom Zeitplan, und verschiebt den nächsten geplanten Lauf nicht.
  - [ ] Bearbeiten einer Aufgabe während sie läuft wirkt erst auf den nächsten Lauf.
- **Testfixture:** Integrationstest mit gemocktem OS-Planer-Adapter (Trait in `os-cli`), plus manueller Test je OS pro Release.

### OUT-161 Aufgabe: Allgemein (Name, Projekt, Zeitplan)
- **Priorität:** P1
- **Beschreibung:** Grunddaten einer Aufgabe.
- **Eingaben/Konfiguration:** `Task Name` (Pflicht, eindeutig), `Project Name` (Projektordner in der Crawl-Verwaltung und Drive, Standard = Taskname), `Description`, `Start Date/Time`, `Interval` ∈ {Einmalig, Täglich, Wöchentlich, Monatlich} (P1); erweitert **(OS-Festlegung, P2)**: Stündlich, alle N Tage, Wochentage-Auswahl.
- **Ausgabe:** Zeitplan im OS-Planer.
- **Akzeptanzkriterien:**
  - [ ] Startzeitpunkt in der Vergangenheit → Validierungsfehler bei „Einmalig“; bei wiederkehrenden Intervallen wird der nächste zukünftige Termin berechnet und angezeigt.
  - [ ] Monatlich mit Starttag 31 → in Monaten mit weniger Tagen am letzten Tag des Monats.
  - [ ] Wöchentlich → gleicher Wochentag und gleiche Uhrzeit wie Startzeitpunkt.
  - [ ] Zeitangaben gelten in lokaler Zeit; bei Sommerzeitumstellung läuft ein täglicher 02:30-Lauf am Umstellungstag genau einmal (in der fehlenden Stunde: 03:30; in der doppelten: beim ersten Auftreten).
  - [ ] Taskname mit für Dateinamen unzulässigen Zeichen wird für Ordnernamen bereinigt (OUT-008), Anzeige bleibt unverändert.
- **Testfixture:** Unit-Tests der Terminberechnung (`next_run(start, interval, now, tz)`) mit Zeitzone `Europe/Berlin` inkl. DST-Daten 2026-03-29 und 2026-10-25.

### OUT-162 Aufgabe: Crawl-Einstellungen
- **Priorität:** P1
- **Beschreibung:** Was und wie gecrawlt wird.
- **Eingaben/Konfiguration:** `Mode` ∈ {Spider, List, APIs (P2)}; Spider: Start-URL; List: Quelle = lokale Datei (`.txt`, `.csv`, `.xlsx`), XML-Sitemap-URL (auch Sitemap-Index; mehrere Sitemap-URLs zulässig) oder Google-Sheet-URL (mit Drive-Konto für private Tabellen); `Config File` (`.osconfig`, optional – sonst Standardkonfiguration); `Auth Profile` (`.osauth`, optional).
- **Akzeptanzkriterien:**
  - [ ] List-Quelle Sitemap-URL → alle `<loc>` der Sitemap (rekursiv über Index) werden gecrawlt.
  - [ ] List-Quelle Google-Sheet → URLs der ersten Spalte des ersten Blatts werden gecrawlt (Kopfzeile wird erkannt, wenn die erste Zelle keine URL ist).
  - [ ] Fehlt die Konfigurationsdatei zur Laufzeit → Lauf schlägt mit Status „Failed: config not found“ fehl, Historie-Eintrag, keine Teil-Exporte.
  - [ ] Auth-Profil mit Formular-Login → der geplante Crawl erreicht die geschützte Seite (Status 200 statt Weiterleitung zum Login).
- **Testfixture:** `out-schedule`: Fixture mit Sitemap-Index, Login-Formular-Bereich (Cookie-Session) und Basic-Auth-Bereich.

### OUT-163 Aufgabe: Ausführungsart (headless)
- **Priorität:** P1
- **Beschreibung:** Geplante Läufe laufen immer ohne Benutzeroberfläche über das CLI-Binary **(OS-Festlegung;** in der Referenz muss „Headless“ für Exporte aktiviert sein – bei OpenSpider entfällt die Wahl). Optional „Nach Abschluss in der App öffnen“ (P2): Ist die Desktop-App geöffnet, wird der Crawl dort als Benachrichtigung angeboten.
- **Akzeptanzkriterien:**
  - [ ] Ein geplanter Lauf öffnet kein Fenster und stiehlt keinen Fokus.
  - [ ] Läuft die Desktop-App, beeinflusst der geplante Lauf deren Crawl nicht (getrennte Prozesse, getrennte Crawl-Dateien).
- **Testfixture:** manueller Test je OS.

### OUT-164 Aufgabe: API-Datenquellen
- **Priorität:** P2
- **Beschreibung:** Pro Aufgabe wählbar, welche Integrationen während des Crawls abgefragt werden: GA4, Search Console, PageSpeed Insights, Majestic, Ahrefs, Moz. Zugangsdaten stammen aus den globalen Integrationseinstellungen (Schlüsselbund).
- **Akzeptanzkriterien:**
  - [ ] Aktivierte Integration ohne gültige Zugangsdaten → Aufgabe „Invalid“ mit Grund.
- **Testfixture:** Mock-Endpunkte.

### OUT-165 Aufgabe: Exporte
- **Priorität:** P1
- **Beschreibung:** Exporte nach Crawl-Ende.
- **Eingaben/Konfiguration:**
  - *Ausgabe:* Ziel lokal (Ordner) und/oder Google Drive (Konto); Überschreiben vs. Zeitstempel-Ordner (OUT-013); Format CSV/XLSX/XLS/Google Sheets; Konsolidierung in eine Arbeitsmappe (OUT-007); Leere überspringen (OUT-012); Segment (OUT-011).
  - *Tabellen:* Auswahlbaum Tabs/Filter, Bulk-Exporte, Reports (wie Multi-Export, OUT-075) oder Auswahl eines gespeicherten Presets.
  - *Lokale Dateien:* Crawl-Datei zusätzlich als `.ospider` in den Exportordner kopieren (die Crawl-Verwaltung speichert ohnehin, OUT-228), XML-Sitemap, Bild-Sitemap.
  - *Looker Studio:* OUT-210.
- **Ausgabe:** Ordner `<Output>/<Project>/<Task>/[<Zeitstempel>/]` bzw. Drive `OpenSpider/<Project>/<Task>/[<Zeitstempel>/]`.
- **Akzeptanzkriterien:**
  - [ ] Ein Lauf mit Internal:All (CSV), Redirect Chains und XML-Sitemap erzeugt drei Dateien im Zielordner.
  - [ ] Drive-Fehler (z. B. Quota) lässt lokale Exporte unberührt; Lauf-Status „Completed with warnings“.
  - [ ] Ist eine Auswahl-Position nicht verfügbar (z. B. PSI-Report ohne PSI), wird sie übersprungen und in der Historie aufgeführt.
- **Testfixture:** `out-basic-site` + Drive-Mock.

### OUT-166 Ausführung über den Betriebssystem-Planer
- **Priorität:** P1
- **Beschreibung:** Damit geplante Crawls ohne geöffnete App laufen, registriert OpenSpider jede Aufgabe beim OS-Planer, der `openspider --run-task <task-id>` aufruft **(OS-Festlegung)**.
  - macOS: LaunchAgent (`~/Library/LaunchAgents/de.openspider.task.<id>.plist`) mit `StartCalendarInterval`; läuft im Benutzerkontext, wenn der Benutzer angemeldet ist.
  - Windows: Aufgabe in der Aufgabenplanung (Ordner `\OpenSpider\`), Trigger gemäß Intervall, „Nur ausführen, wenn Benutzer angemeldet ist“ (Standard); Option „Unabhängig von der Anmeldung ausführen“ (P2, verlangt Kennworteingabe im Windows-Dialog – OpenSpider speichert kein Windows-Kennwort).
- **Eingaben/Konfiguration:** Optionen: „Auch im Akkubetrieb ausführen“ (Standard an, wie Referenz seit 18.2), „Verpasste Läufe nach dem Aufwachen nachholen“ (Standard an: höchstens **ein** Nachhol-Lauf), „Rechner zum Ausführen aufwecken“ (Windows, P2).
- **Akzeptanzkriterien:**
  - [ ] Bei geschlossener App startet der Lauf zur geplanten Zeit (± 60 s).
  - [ ] Nach Ruhezustand über 3 verpasste tägliche Termine → genau ein Nachhol-Lauf.
  - [ ] Keine Begrenzung der Laufzeit durch den Planer (Windows: Option „Aufgabe beenden, falls länger als“ deaktiviert) – lange Crawls werden nicht abgebrochen.
  - [ ] Nach einem App-Update zeigen registrierte Aufgaben auf das neue Binary (stabiler Pfad oder Neuregistrierung beim ersten Start nach Update).
  - [ ] Deinstallation entfernt alle OS-Registrierungen.
- **Testfixture:** Adapter-Unit-Tests (erzeugtes plist/Task-XML gegen Golden), manueller Release-Test.

### OUT-167 Parallele und überlappende Läufe
- **Priorität:** P1
- **Beschreibung:** Jeder Lauf ist ein eigener Prozess. Verschiedene Aufgaben dürfen gleichzeitig laufen. Für dieselbe Aufgabe gilt die Option „Überlappung“: `Parallel starten` (Referenzverhalten, Standard) oder `Überspringen, wenn noch aktiv` **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Zwei Aufgaben zur selben Minute → zwei Prozesse, zwei Crawl-Dateien, keine Sperrkonflikte.
  - [ ] „Überspringen“ aktiv und Vorlauf läuft noch → Historie-Eintrag „Skipped: previous run still active“.
  - [ ] Gleichzeitige Läufe schreiben nie in dieselbe Exportdatei (Zeitstempel-Ordner bzw. Dateisperre mit Wartezeit ≤ 60 s, danach Fehler).
- **Testfixture:** Integrationstest mit zwei CLI-Prozessen auf `out-basic-site` (verlangsamt).

### OUT-168 Gültigkeitsprüfung von Aufgaben
- **Priorität:** P1
- **Beschreibung:** Jede Aufgabe zeigt den Status `Valid` oder `Invalid` mit Grund, geprüft beim Speichern, beim App-Start und vor jedem Lauf.
- **Akzeptanzkriterien:**
  - [ ] Gründe mindestens: Konfigurationsdatei fehlt, Auth-Profil fehlt, Listen-Datei fehlt, ungültige Start-URL, Ausgabeordner nicht beschreibbar, Google-Konto nicht verbunden/abgelaufen, E-Mail-Konto fehlt für Benachrichtigung, OS-Registrierung fehlt.
  - [ ] Ungültige Aufgabe wird vom Planer trotzdem aufgerufen, beendet sich aber sofort mit Historie-Eintrag und (falls möglich) Fehlermail.
- **Testfixture:** Unit-Tests je Grund.

### OUT-169 Laufhistorie
- **Priorität:** P1
- **Beschreibung:** Je Aufgabe eine Historie der Läufe.
- **Ausgabe:** Spalten `Started`, `Finished`, `Duration`, `Result` (`Completed`, `Completed with warnings`, `Failed`, `Skipped`, `Aborted`), `URLs Crawled`, `Crawl ID`, `Output Folder`, `Message`; Aktion „Log öffnen“ (Lauf-Logdatei, OUT-241).
- **Akzeptanzkriterien:**
  - [ ] Abgestürzter Lauf (Prozess beendet ohne Abschluss) erscheint beim nächsten App-Start als `Failed` mit „unexpected termination“.
  - [ ] Historie hält mindestens die letzten 100 Läufe je Aufgabe; ältere werden verworfen.
  - [ ] Listenmodus-Läufe erscheinen ebenso in der Historie (Referenzfehler in 21.4 behoben).
- **Testfixture:** CLI-Integrationstest mit absichtlich abgebrochenem Prozess.

### OUT-170 Aufgaben exportieren/importieren
- **Priorität:** P2
- **Beschreibung:** Aufgaben können als Datei exportiert und auf einem anderen Rechner importiert werden.
- **Ausgabe:** `.ostask`-Datei (TOML) ohne Geheimnisse; referenzierte Konfigurations-/Auth-Dateien optional als `.zip` mitgepackt.
- **Akzeptanzkriterien:**
  - [ ] Import auf anderem Rechner → Aufgabe erscheint; fehlende Abhängigkeiten (Google-Konto, E-Mail-Konto, Pfade) führen zu `Invalid` mit konkretem Grund.
  - [ ] Exportdatei enthält keine Tokens/Kennwörter (Test sucht nach bekannten Testgeheimnissen).
- **Testfixture:** Roundtrip-Test.

### OUT-171 Laufende geplante Crawls abbrechen
- **Priorität:** P2
- **Beschreibung:** Aus der Aufgabenliste kann ein laufender Lauf abgebrochen werden; optional maximale Laufzeit je Aufgabe **(OS-Festlegung;** die Referenz unterstützt das laut Hersteller noch nicht).
- **Eingaben/Konfiguration:** „Max. Laufzeit (h)“, Standard leer.
- **Akzeptanzkriterien:**
  - [ ] Abbruch → Prozess beendet sich innerhalb 10 s, Crawl wird gespeichert, Status `Aborted`, Exporte werden nicht erzeugt.
  - [ ] Max. Laufzeit erreicht → Crawl wird gestoppt, Exporte des bisherigen Stands werden erzeugt, Status `Completed with warnings`.
- **Testfixture:** `out-basic-site` mit großer Verzögerung.

---

## 8 Kommandozeile / Headless

Ein eigenes Konsolen-Binary `openspider` (Crate `os-cli`) bietet alle Funktionen ohne Benutzeroberfläche. Optionsnamen orientieren sich an der Referenz, damit vorhandene Skripte leicht übertragbar sind; abweichende/zusätzliche Optionen sind markiert. Alle Exporte nutzen die kanonischen Namen aus Abschnitt 0.

### OUT-180 CLI-Binary und Aufruf je Plattform
- **Priorität:** P0
- **Beschreibung:** Bereitstellung eines echten Konsolenprogramms je Plattform.
  - macOS: `OpenSpider.app/Contents/MacOS/openspider` (Konsolenprogramm, blockiert bis zum Ende, Ausgabe im Terminal); Menüpunkt „Kommandozeilenwerkzeug installieren“ legt einen Symlink in `/usr/local/bin` (mit Admin-Rückfrage) oder `~/.local/bin` an.
  - Windows: `openspider.exe` (Subsystem Console) im Installationsordner neben der GUI; Installer bietet „Zu PATH hinzufügen“ an.
- **Akzeptanzkriterien:**
  - [ ] Aufruf aus Terminal/PowerShell blockiert bis zum Ende und liefert einen Exit-Code (OUT-204).
  - [ ] Das Binary startet ohne grafische Sitzung (SSH auf macOS; Windows-Dienstkonto) für Crawls ohne JS-Rendering.
  - [ ] Keine Abhängigkeit von installierter Java- oder Node-Laufzeit.
- **Testfixture:** CI-Smoke-Test `openspider --version` auf allen drei Zielplattformen.

### OUT-181 Hilfe und Version
- **Priorität:** P0
- **Beschreibung:** Selbstdokumentation.
- **Eingaben/Konfiguration:** `--help`; `--help export-tabs`, `--help bulk-export`, `--help save-report`, `--help export-custom-summary` listen alle gültigen Namen der jeweiligen Kategorie (aus derselben Registry wie die UI); `--version`.
- **Akzeptanzkriterien:**
  - [ ] `--help export-tabs` listet jeden `Tab:Filter`, eine Zeile je Eintrag, maschinenlesbar mit `--json`.
  - [ ] `--version` gibt `openspider <semver> (<git-hash>, <os>-<arch>)` aus.
  - [ ] Liste ist vollständig: Registry-Test vergleicht Anzahl mit UI-Filterregistry.
- **Testfixture:** Golden-Ausgabe `tests/golden/out-cli/help_export_tabs.txt`.

### OUT-182 `--crawl <url>`
- **Priorität:** P0
- **Beschreibung:** Startet einen Crawl im Spider-Modus ab der URL.
- **Akzeptanzkriterien:**
  - [ ] URL ohne Schema → `http://` wird ergänzt (wie UI), Hinweis auf stderr.
  - [ ] Ungültige URL → Exit-Code 2 vor Crawlbeginn.
  - [ ] Start-URL nicht erreichbar → Crawl endet regulär mit 1 URL (No Response), Exit-Code 0, Warnung **(OS-Festlegung:** kein Fehler, weil das ein gültiges Crawlergebnis ist); mit `--fail-on-start-error` Exit-Code 3.
- **Testfixture:** `out-basic-site`.

### OUT-183 `--crawl-list <quelle>`
- **Priorität:** P0
- **Beschreibung:** Crawl im Listenmodus. Quelle: lokale Datei (`.txt`, `.csv`, `.xlsx`; URLs werden aus allen Zellen extrahiert), XML-Sitemap-URL oder -Datei (inkl. Index; mehrfach angebbar), Google-Sheet-URL (mit `--google-drive-account`).
- **Akzeptanzkriterien:**
  - [ ] Datei mit 10 URLs, davon 2 Duplikate → 8 gecrawlte URLs; Export in Upload-Reihenfolge (OUT-015) mit 10 Zeilen.
  - [ ] `--crawl-list https://site.test/sitemap_index.xml` crawlt alle URLs aller untergeordneten Sitemaps.
  - [ ] Mehrere `--crawl-list`-Angaben werden zusammengeführt (P2).
  - [ ] Leere Liste → Exit-Code 2 mit Meldung.
- **Testfixture:** `out-list-order`, `out-sitemaps-in`.

### OUT-184 `--load-crawl <datei|id>`
- **Priorität:** P1
- **Beschreibung:** Öffnet einen gespeicherten Crawl (Pfad zu `.ospider` oder Crawl-ID aus der Crawl-Verwaltung), um ohne erneutes Crawlen zu exportieren.
- **Akzeptanzkriterien:**
  - [ ] `--load-crawl <id> --export-tabs "Internal:All" --output-folder out` erzeugt den Export ohne Netzwerkzugriff.
  - [ ] Unbekannte ID → Exit-Code 2 mit Hinweis auf `--list-crawls`.
  - [ ] Ein noch laufender oder unvollständiger Crawl kann geladen werden; Exporte zeigen den gespeicherten Stand; Warnung „crawl incomplete“.
  - [ ] Crawl-Dateien älterer Schemaversion werden migriert (Kopie, Original bleibt unverändert) **(OS-Festlegung)**.
- **Testfixture:** gespeicherter Crawl `tests/fixtures/out-basic-site/crawl.ospider`.

### OUT-185 `--list-crawls`
- **Priorität:** P1
- **Beschreibung:** Listet alle Crawls der Crawl-Verwaltung.
- **Ausgabe:** Tabelle `ID`, `Project`, `Name`, `Mode`, `Start URL`, `Started`, `URLs`, `Status`, `Locked`; mit `--json` als JSON-Array.
- **Akzeptanzkriterien:**
  - [ ] Filter `--project-name X` beschränkt auf ein Projekt.
  - [ ] IDs sind stabil und identisch mit „ID kopieren“ in der UI.
- **Testfixture:** Crawl-Verwaltung mit 3 Crawls in 2 Projekten.

### OUT-186 `--config <datei>`
- **Priorität:** P0
- **Beschreibung:** Verwendet eine gespeicherte Crawl-Konfiguration (OUT-221). Alle Optionen ohne eigene CLI-Flags (Rendering, Include/Exclude, Custom Extraction, Spalten, Sitemap-Optionen, Change Detection, Segmente …) werden so gesetzt.
- **Akzeptanzkriterien:**
  - [ ] Konfiguration mit Exclude `/privat/` → keine URL unter `/privat/` im Crawl.
  - [ ] Nicht lesbare/ungültige Datei → Exit-Code 2 mit Zeilen-/Feldangabe des Fehlers.
  - [ ] Konfiguration einer neueren Schemaversion mit unbekannten Feldern → Warnung, unbekannte Felder werden ignoriert.
- **Testfixture:** `tests/fixtures/out-basic-site/exclude.osconfig`.

### OUT-187 `--auth-config <datei>`
- **Priorität:** P1
- **Beschreibung:** Verwendet ein Authentifizierungsprofil (Basic/Digest/NTLM-Zugangsdaten und/oder gespeicherte Formular-Login-Cookies), damit automatisierte Crawls geschützte Bereiche erreichen.
- **Akzeptanzkriterien:**
  - [ ] Basic-Auth-Bereich der Fixture liefert mit Profil 200, ohne 401.
  - [ ] Formular-Login: gespeicherte Session-Cookies werden gesendet; abgelaufene Session → Warnung „authentication cookies expired“ im Log.
  - [ ] Das Profil ist verschlüsselt (OUT-223); falsches/fehlendes Schlüsselmaterial → Exit-Code 2.
- **Testfixture:** `out-schedule` (Login-Bereich).

### OUT-188 `--headless`
- **Priorität:** P0
- **Beschreibung:** Das CLI-Binary läuft immer ohne Oberfläche; `--headless` wird aus Kompatibilitätsgründen akzeptiert und ist wirkungslos **(OS-Festlegung)**. Mit `--gui` (P2) wird stattdessen die Desktop-App mit dem gestarteten Crawl geöffnet.
- **Akzeptanzkriterien:**
  - [ ] Mit und ohne `--headless` identisches Verhalten des CLI-Binaries.
- **Testfixture:** CLI-Test.

### OUT-189 `--save-crawl`
- **Priorität:** P0
- **Beschreibung:** Legt den abgeschlossenen Crawl zusätzlich als `.ospider`-Datei im Ausgabeordner ab (in der Crawl-Verwaltung wird ohnehin gespeichert).
- **Akzeptanzkriterien:**
  - [ ] Datei `crawl.ospider` im Ausgabeordner; per `--load-crawl <pfad>` wieder ladbar mit identischen Exporten.
  - [ ] Ohne `--save-crawl` und mit `--no-store` **(OS-Festlegung)** wird der Crawl nach Ende verworfen (temporäre Datei gelöscht).
- **Testfixture:** `out-basic-site`.

### OUT-190 `--output-folder`, `--overwrite`, `--timestamped-output`
- **Priorität:** P0
- **Beschreibung:** Ziel der Ausgaben; Standard = aktuelles Arbeitsverzeichnis. Verhalten gemäß OUT-013.
- **Akzeptanzkriterien:**
  - [ ] `--timestamped-output` legt `<output>/<YYYY-MM-DD_HH-MM-SS>/` an, auch in Google Drive.
  - [ ] `--overwrite` ersetzt gleichnamige Dateien.
  - [ ] Beide angegeben → `--timestamped-output` hat Vorrang, Warnung.
  - [ ] Pfade mit Leerzeichen und Nicht-ASCII-Zeichen funktionieren (Windows: `C:\Users\Jürgen\Desktop\SEO Reports`).
  - [ ] Ein Pfad, der mit `\` endet und in Anführungszeichen steht (Windows-Fallstrick `"C:\out\"`), wird erkannt und korrigiert statt die Folgeargumente zu verschlucken **(OS-Festlegung)**.
- **Testfixture:** CLI-Integrationstests mit temporären Ordnern.

### OUT-191 `--project-name`, `--task-name`
- **Priorität:** P1
- **Beschreibung:** Ordnet den Crawl einem Projekt und Namen in der Crawl-Verwaltung zu; bestimmt auch die Ordnerstruktur in Google Drive und lokal (OUT-165).
- **Akzeptanzkriterien:**
  - [ ] Crawl erscheint in der UI-Crawl-Verwaltung unter dem Projekt mit dem Namen.
  - [ ] Ohne Angabe: Projekt „Default“, Name = Host der Start-URL bzw. Listen-Dateiname.
- **Testfixture:** CLI-Test + Abfrage via `--list-crawls`.

### OUT-192 `--export-tabs "<Tab>:<Filter>,…"`
- **Priorität:** P0
- **Beschreibung:** Exportiert Tabs mit Filtern nach Crawl-Ende bzw. aus dem geladenen Crawl. Namen entsprechen den kanonischen UI-Namen; konfigurierbare Schwellen im Namen dürfen als `X` oder als aktueller Wert geschrieben werden (z. B. `Page Titles:Over X Characters` oder `Page Titles:Over 60 Characters`).
- **Akzeptanzkriterien:**
  - [ ] `--export-tabs "Internal:All,Response Codes:Client Error (4xx)"` erzeugt `internal_all.csv` und `response_codes_client_error_4xx.csv`.
  - [ ] Namensvergleich ist case-insensitiv und toleriert zusätzliche Leerzeichen um `:` und `,`.
  - [ ] Kommas innerhalb eines Namens sind per `\,` maskierbar.
  - [ ] Unbekannter Name → Abbruch vor dem Crawl (OUT-203).
- **Testfixture:** `out-basic-site`.

### OUT-193 `--bulk-export "<Menü>:<Eintrag>,…"`
- **Priorität:** P0
- **Beschreibung:** Führt Bulk-Exporte aus Abschnitt 3 aus; verschachtelte Menüs mit weiterem `:`.
- **Akzeptanzkriterien:**
  - [ ] `--bulk-export "Links:All Inlinks,Response Codes:Internal:Client Error (4xx) Inlinks"` erzeugt beide Dateien.
  - [ ] Nicht-tabellarische Bulk-Exporte (Screenshots, Page Source …) landen in Unterordnern des Ausgabeordners.
- **Testfixture:** `out-links`, `out-response-codes`.

### OUT-194 `--save-report "<Menü>:<Report>,…"`
- **Priorität:** P0
- **Beschreibung:** Speichert Reports aus Abschnitt 4.
- **Akzeptanzkriterien:**
  - [ ] `--save-report "Crawl Overview,Redirects:Redirect Chains"` erzeugt `crawl_overview.csv` und `redirect_chains.csv`.
  - [ ] Report mit unerfüllter Voraussetzung → Warnung, kein Abbruch, Exit-Code 0 (bzw. 4 mit `--strict`).
- **Testfixture:** `out-redirects`.

### OUT-195 `--export-format csv|xls|xlsx|gsheet`
- **Priorität:** P0
- **Beschreibung:** Format für alle Exporte des Aufrufs; Standard `csv`. `gsheet` erfordert `--google-drive-account`.
- **Akzeptanzkriterien:**
  - [ ] `gsheet` ohne Konto → Exit-Code 2 vor dem Crawl.
  - [ ] Unbekanntes Format → Exit-Code 2 mit Liste gültiger Werte.
- **Testfixture:** CLI-Test.

### OUT-196 Zusätzliche Export-Optionen
- **Priorität:** P1
- **Beschreibung:** **(OS-Festlegung,** Flagnamen) `--export-preset <name|pfad>` (Multi-Export-Preset, OUT-075), `--single-workbook` (OUT-007), `--skip-empty-exports` (OUT-012), `--segment <name|all>` (OUT-011), `--column-headers ui|canonical` (Abschnitt 0).
- **Akzeptanzkriterien:**
  - [ ] Preset und `--export-tabs` kombinierbar; Duplikate werden nur einmal exportiert.
  - [ ] Jede Option hat einen Integrationstest.
- **Testfixture:** `out-basic-site`, `out-segments`.

### OUT-197 `--create-sitemap`, `--create-images-sitemap`
- **Priorität:** P1
- **Beschreibung:** Siehe OUT-133.
- **Akzeptanzkriterien:**
  - [ ] Dateien liegen im (ggf. zeitgestempelten) Ausgabeordner; Aufteilung gemäß OUT-129.
- **Testfixture:** `out-sitemap-gen`.

### OUT-198 `--google-drive-account <konto>`
- **Priorität:** P1
- **Beschreibung:** Wählt ein in der App verbundenes Google-Konto (E-Mail-Adresse) für Drive/Sheets-Exporte und Google-Sheet-Listenquellen.
- **Akzeptanzkriterien:**
  - [ ] Nicht verbundenes Konto → Exit-Code 2 mit Hinweis, das Konto in der App zu verbinden (OAuth erfordert einmalig einen Browser).
  - [ ] Auf Rechnern ohne Browser kann ein in der App exportiertes Konto-Token-Paket importiert werden: `openspider --import-google-credentials <datei>` (P2, verschlüsselt, OUT-223).
- **Testfixture:** Drive-Mock.

### OUT-199 `--email-on-complete <adresse,…>`
- **Priorität:** P2
- **Beschreibung:** Sendet nach Abschluss eine Abschlussmail (OUT-282) über das konfigurierte E-Mail-Konto.
- **Akzeptanzkriterien:**
  - [ ] Ohne konfiguriertes E-Mail-Konto → Warnung, Crawl läuft trotzdem, Exit-Code 0.
  - [ ] `--email-attach-exports` **(OS-Festlegung)** hängt die Exporte gezippt an (OUT-283).
- **Testfixture:** SMTP-Mock (z. B. lokaler Test-SMTP im `os-testsite`).

### OUT-200 `--export-custom-summary "<Metrik>,…"`
- **Priorität:** P2
- **Beschreibung:** Führt den Looker-Studio-Export (OUT-210) mit den genannten Metriken aus; erfordert Google-Konto.
- **Akzeptanzkriterien:**
  - [ ] Hängt genau eine Zeile an die Tabelle `<task>_custom_summary_report` an.
  - [ ] Unbekannte Metrik → Abbruch vor dem Crawl (OUT-203).
- **Testfixture:** Sheets-Mock.

### OUT-201 API-Optionen
- **Priorität:** P2
- **Beschreibung:** Aktiviert Integrationen für den Crawl: `--use-google-analytics-4 "<konto>" "<account>" "<property>" "<datenstrom>"`, `--use-google-search-console "<konto>" "<website>"`, `--use-pagespeed`, `--use-ahrefs`, `--use-majestic`, `--use-mozscape`. Schlüssel kommen aus dem Schlüsselbund der App oder aus Umgebungsvariablen `OPENSPIDER_PSI_KEY`, `OPENSPIDER_AHREFS_KEY`, `OPENSPIDER_MAJESTIC_KEY`, `OPENSPIDER_MOZ_KEY` **(OS-Festlegung)**. `--use-google-analytics` (Universal Analytics) wird mit Fehlermeldung „nicht mehr unterstützt“ abgelehnt.
- **Akzeptanzkriterien:**
  - [ ] Fehlender Schlüssel → Exit-Code 2 vor dem Crawl.
  - [ ] Schlüssel aus Umgebungsvariable erscheinen nie im Log.
- **Testfixture:** Mock-Endpunkte.

### OUT-202 `--crawl-comparison <id1> <id2>`, `--project-crawl-comparison ["true"|"false"]`
- **Priorität:** P1
- **Beschreibung:** Vergleich zweier Crawls bzw. der letzten beiden eines Projekts (OUT-152); Current/Previous nach Crawl-Datum. Die Referenz erwartet `--project-crawl-comparison "true"`; OpenSpider akzeptiert das Flag mit und ohne Argument, damit bestehende Skripte unverändert laufen.
- **Akzeptanzkriterien:**
  - [ ] `--crawl-comparison A B --save-report "Crawl Overview"` erzeugt den Overview mit Vergleichsspalten (OUT-147); die Reihenfolge von A und B ist egal (Current = späteres Crawl-Datum).
  - [ ] Vergleich mit sich selbst → Exit-Code 2.
  - [ ] `--project-crawl-comparison`, `--project-crawl-comparison true` und `--project-crawl-comparison "TRUE"` aktivieren den Vergleich identisch; `--project-crawl-comparison false` deaktiviert ihn (gleichbedeutend mit Weglassen).
  - [ ] Jeder andere Wert (z. B. `yes`, `1`) → Exit-Code 2 mit Meldung der gültigen Werte.
  - [ ] Ein auf das Flag folgendes Argument, das mit `--` beginnt, wird nicht als Wert verbraucht (`--project-crawl-comparison --headless` funktioniert).
  - [ ] `--project-crawl-comparison` ohne `--project-name` → Exit-Code 2.
- **Testfixture:** `out-compare`.

### OUT-203 Validierung aller Namen vor dem Crawl
- **Priorität:** P0
- **Beschreibung:** Alle Argumente (Tab-, Bulk-, Report-, Metrik-Namen, Formate, Pfade, Konten) werden geprüft, **bevor** gecrawlt wird. Bei unbekannten Namen werden ähnliche gültige Namen vorgeschlagen (Levenshtein ≤ 3) **(OS-Festlegung;** bewusst strenger als die Referenz, deren öffentlich dokumentiertes Verhalten keine Vorab-Validierung beschreibt).
- **Akzeptanzkriterien:**
  - [ ] `--export-tabs "Internal:Al"` → Exit-Code 2 in < 1 s, Meldung `unknown export 'Internal:Al' – did you mean 'Internal:All'?`.
  - [ ] Kein Netzwerkzugriff erfolgt vor erfolgreicher Validierung (Fixture-Server zählt 0 Requests).
- **Testfixture:** CLI-Test mit Request-Zähler im `os-testsite`.

### OUT-204 Exit-Codes
- **Priorität:** P0
- **Beschreibung:** Eindeutige Rückgabewerte für Automatisierung **(OS-Festlegung)**.
- **Ausgabe:** `0` Erfolg (auch mit Warnungen, außer `--strict`); `1` unerwarteter interner Fehler; `2` Aufruf-/Konfigurationsfehler (vor dem Crawl erkannt); `3` Crawl konnte nicht durchgeführt werden (z. B. Crawl-Datei nicht schreibbar, Datenträger voll; mit `--fail-on-start-error` auch Start-URL nicht erreichbar); `4` Crawl erfolgreich, aber mindestens ein Export fehlgeschlagen (bzw. Warnungen bei `--strict`); `130` durch Benutzer (SIGINT/Strg+C) abgebrochen.
- **Akzeptanzkriterien:**
  - [ ] Je Exit-Code ein Integrationstest, der ihn provoziert.
  - [ ] Die letzte Zeile auf stderr fasst Ergebnis und Exit-Code zusammen.
- **Testfixture:** CLI-Tests (Drive-Mock mit Fehler für Code 4; schreibgeschützter Ordner für Code 3).

### OUT-205 Fortschritt und maschinenlesbare Ausgabe
- **Priorität:** P1
- **Beschreibung:** stdout bleibt für Nutzdaten (z. B. `--list-crawls`, `--help`) reserviert; Fortschritt und Logs gehen nach stderr. Mit `--progress json` wird je Sekunde (max.) eine JSON-Zeile mit `{"phase","crawled","queued","errors","elapsed_s","eta_s"}` ausgegeben **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Ohne TTY keine Fortschrittsbalken/ANSI-Sequenzen.
  - [ ] Phasen: `validate`, `crawl`, `analysis`, `export`, `notify`, `done`.
  - [ ] `--quiet` unterdrückt alles außer Fehlern; `--log-level` siehe OUT-241.
- **Testfixture:** CLI-Test mit Pipe statt TTY.

### OUT-206 Abbruch per Signal
- **Priorität:** P1
- **Beschreibung:** SIGINT/SIGTERM (macOS) bzw. Strg+C/Konsolen-Schließen (Windows) beenden den Crawl geordnet.
- **Akzeptanzkriterien:**
  - [ ] Erstes Strg+C: Crawl stoppt, Crawl-Datei wird konsistent geschlossen, keine Exporte, Exit-Code 130, innerhalb 10 s.
  - [ ] Zweites Strg+C: sofortiger Abbruch; die Crawl-Datei bleibt dank WAL öffnenbar.
  - [ ] Option `--export-on-interrupt` erzeugt die Exporte des bisherigen Stands vor dem Beenden (P2).
- **Testfixture:** CLI-Test, der nach 2 s SIGINT sendet.

### OUT-207 JS-Rendering im Headless-Betrieb
- **Priorität:** P1
- **Beschreibung:** JavaScript-Rendering funktioniert im CLI ohne sichtbares Fenster (Chromium headless) auf macOS und Windows, auch bei gesperrtem Bildschirm.
- **Akzeptanzkriterien:**
  - [ ] Geplanter Lauf mit Rendering bei gesperrtem Bildschirm liefert Screenshots und gerendertes HTML.
  - [ ] Fehlt die Chromium-Komponente, meldet die CLI das vor dem Crawl (Exit-Code 2) und nennt den Befehl zum Nachinstallieren (`openspider --install-browser`, P1).
- **Testfixture:** `out-render` in CI (macOS/Windows Runner).

### OUT-208 Weitere Modi per CLI
- **Priorität:** P2
- **Beschreibung:** `--mode apis --crawl-list <datei>` ruft nur API-Daten für die Liste ab; `--mode serp --serp-file <datei>` berechnet Längen/Pixelbreiten für hochgeladene Titles/Descriptions und exportiert das Ergebnis **(OS-Festlegung,** Flagnamen).
- **Akzeptanzkriterien:**
  - [ ] SERP-Modus mit Datei aus OUT-098 → Export identisch zur Eingabe (Rundreise).
  - [ ] APIs-Modus führt keine HTTP-Requests gegen die gelisteten URLs aus.
- **Testfixture:** `out-serp`.

### OUT-209 Aufgabenausführung `--run-task <id>`
- **Priorität:** P1
- **Beschreibung:** Einstieg für den OS-Planer (OUT-166): lädt die Aufgabendatei und führt sie aus; schreibt Historie und Lauf-Log.
- **Akzeptanzkriterien:**
  - [ ] `--run-task` mit unbekannter ID → Exit-Code 2; da keine Aufgabe zuordenbar ist, erfolgt der Eintrag im globalen Log (OUT-240).
  - [ ] Ergebnis identisch mit dem gleichwertigen manuellen CLI-Aufruf (Test vergleicht Exporte).
- **Testfixture:** `out-schedule`.

---

## 9 Looker-Studio-Export (Custom Crawl Overview)

### OUT-210 Zeitreihen-Export je Crawl
- **Priorität:** P1
- **Beschreibung:** Nach jedem Crawl einer Aufgabe (oder manuell über Multi-Export) wird **eine Zeile** mit Kennzahlen des Crawls an eine Google-Tabelle angehängt. Über mehrere Läufe entsteht eine Zeitreihe, die z. B. Looker Studio als Datenquelle nutzt. URL-Einzeldaten werden nicht exportiert.
- **Eingaben/Konfiguration:** Aktivierung „Custom Crawl Overview“; Google-Konto; Auswahl der Metriken (Liste „Verfügbar“ → „Ausgewählt“, Reihenfolge = Spaltenreihenfolge).
- **Ausgabe:** Google-Tabelle `OpenSpider/<Project>/<Task>_custom_summary_report` (Blatt `data`); Spalte 1 `Date` (Datum + Uhrzeit des Crawlstarts, als Datumswert), Spalte 2 `Start URL` (bzw. Name der Listendatei im Listenmodus), danach die gewählten Metriken in Auswahlreihenfolge. Belegt durch die Referenz-Doku sind: eine Zeile je Lauf, ein Datums- und ein URL-Feld neben lauter Zahlenfeldern sowie die frei wählbare Metrikreihenfolge; die feste Position von `Date` vorn und `Start URL` an zweiter Stelle, der Blattname `data` und das Datumsformat sind **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Drei Läufe → drei Datenzeilen unter einer Kopfzeile, chronologisch.
  - [ ] Kopfzeile bei Auswahl `Internal:HTML`, `Response Codes:Client Error (4xx)` = `Date`, `Start URL`, `Internal:HTML`, `Response Codes:Client Error (4xx)` (genau diese Reihenfolge).
  - [ ] `Date` wird als Datumswert geschrieben (Sheets-Zellformat `yyyy-mm-dd hh:mm:ss`, lokale Zeit des Rechners), nicht als Text.
  - [ ] Umbenennen der Aufgabe erzeugt eine neue Tabelle (bestehende bleibt unverändert).
  - [ ] Werte sind Zahlen (nicht Text), damit Looker Studio sie als Number erkennt.
- **Testfixture:** Sheets-Mock, `out-basic-site` dreimal gecrawlt.

### OUT-211 Metrikkatalog
- **Priorität:** P1
- **Beschreibung:** Verfügbare Metriken: alle Summary-Werte aus OUT-080 Abschnitt 1, jeder Filter jedes Tabs als URL-Anzahl (`<Tab>:<Filter>`), Issues nach Typ/Priorität, Durchschnittswerte (Antwortzeit, Wortanzahl, Crawl-Tiefe), sowie URL-Inspection-Kennzahlen (z. B. `Search Console:URL Is Not on Google`, `Search Console:Indexable URL Not Indexed`, `Search Console:Rich Result Invalid`) bei aktiver Integration.
- **Akzeptanzkriterien:**
  - [ ] Katalog wird aus der Filter-Registry erzeugt; neue Filter erscheinen automatisch.
  - [ ] Nicht befüllte Metriken (Integration inaktiv) werden als leere Zelle geschrieben, nicht als 0.
  - [ ] Metriken lassen sich per Kopieren/Einfügen als Textliste übernehmen (für CLI `--export-custom-summary`).
- **Testfixture:** Registry-Unit-Test.

### OUT-212 Spaltenstabilität bei Änderungen
- **Priorität:** P1
- **Beschreibung:** Spaltenzuordnung bleibt über Läufe stabil.
- **Akzeptanzkriterien:**
  - [ ] Neu hinzugefügte Metrik wird als neue Spalte **rechts** angefügt, und die Kopfzeile wird automatisch ergänzt **(Abweichung:** in der Referenz manuell nötig).
  - [ ] Entfernte Metrik: Spalte bleibt bestehen, neue Zeilen erhalten dort leere Werte.
  - [ ] Umsortierung der Auswahl nach dem ersten Lauf ändert die Spaltenreihenfolge der bestehenden Tabelle nicht (Zuordnung über Kopfzeilennamen, nicht Position).
  - [ ] Kopfzeilen nutzen immer kanonische englische Namen (sprachunabhängige Vorlage).
- **Testfixture:** Sheets-Mock mit vorbelegter Tabelle.

### OUT-213 Segmentierte Zeitreihen
- **Priorität:** P2
- **Beschreibung:** Bei konfigurierten Segmenten wird je Segment eine eigene Tabelle `<Task>_custom_summary_report_<Segment>` geführt, zusätzlich zur Gesamt-Tabelle.
- **Akzeptanzkriterien:**
  - [ ] Zwei Segmente → drei Tabellen, je eine Zeile pro Lauf.
- **Testfixture:** `out-segments`.

### OUT-214 Looker-Studio-Vorlage und Datentypen
- **Priorität:** P2
- **Beschreibung:** OpenSpider liefert eine eigene, offen lizenzierte Looker-Studio-Berichtsvorlage (Seiten: Zusammenfassung, Antwortcodes, URL-Typen, Indexierbarkeit, Struktur, On-Page, Inhalt, PageSpeed, strukturierte Daten, Sicherheit, hreflang, Sitemaps, JavaScript, URL Inspection) und eine Datentyp-Liste (`Column`, `Type`) zur korrekten Feldtyp-Zuordnung in Looker Studio.
- **Eingaben/Konfiguration:** Option „Datentyp-Blatt in die Zeitreihen-Tabelle schreiben“ (Standard an).
- **Ausgabe:**
  - Datentyp-Liste als eigenständige Datei im Repository (`docs/looker/datatypes.csv`, generiert aus der Metrik-Registry) und in der Doku verlinkt – entspricht dem Referenzvorgehen (dort eine separat verlinkte Tabelle, nicht Teil des Exports).
  - Zusätzlich, abweichend von der Referenz, ein zweites Blatt `datatypes` in jeder Zeitreihen-Tabelle, das genau die Spalten dieser Tabelle beschreibt **(OS-Festlegung, Abweichung)**.
  - Typwerte: `Date` (nur Spalte `Date`), `Text` (nur `Start URL`), `Percent` (Metriken, die einen Anteil ausdrücken, z. B. `% of Total`-Kennzahlen), `Number` (alle übrigen).
- **Akzeptanzkriterien:**
  - [ ] `docs/looker/datatypes.csv` enthält für jede Metrik des Katalogs (OUT-211) genau eine Zeile; CI schlägt fehl, wenn die Datei nicht zur Registry passt (Generator-Diff leer).
  - [ ] Blatt `datatypes` enthält genau eine Zeile je Spalte des Blatts `data`, in derselben Reihenfolge; nach Hinzufügen einer Metrik (OUT-212) wird auch das Blatt `datatypes` um eine Zeile ergänzt.
  - [ ] Mit deaktivierter Option entsteht kein zweites Blatt; die Zeitreihe ist unverändert.
  - [ ] Typzuordnung: `Date`→Date, `Start URL`→Text, `Internal:HTML`→Number; keine Spalte hat einen leeren Typ.
  - [ ] Die Vorlage ist eigenständig erstellt (kein Kopieren fremder Vorlagen), steht unter einer offenen Lizenz (CC BY 4.0) und ist in der Doku verlinkt; jede Seite der Vorlage nutzt nur Felder, die in `datatypes.csv` vorkommen (Prüfskript über die exportierte Vorlagendefinition).
- **Testfixture:** Sheets-Mock; Registry-Snapshot mit einer hinzugefügten Metrik.

---

## 10 Konfigurationsdateien & Crawl-Dateien

### OUT-220 Konfigurationsprofile speichern, laden, als Standard setzen
- **Priorität:** P0
- **Beschreibung:** Die gesamte Crawl-Konfiguration (alle Bereiche des Konfigurationsdialogs inkl. Spaltenkonfiguration, Segmente, Sitemap-Optionen, Change Detection, URL-Mapping) kann als Datei gespeichert und geladen werden. Eine Konfiguration kann als Standard gesetzt werden; sie gilt dann für jeden neuen Crawl. „Standard zurücksetzen“ stellt die Werkseinstellungen wieder her.
- **Eingaben/Konfiguration:** Menü Konfiguration → Profile: „Speichern unter…“, „Laden…“, „Zuletzt geladen“ (letzte 10), „Als Standard speichern“, „Standard zurücksetzen“.
- **Ausgabe:** `.osconfig`-Datei (OUT-221); Standardkonfiguration als `default.osconfig` im Einstellungsordner (OUT-227).
- **Akzeptanzkriterien:**
  - [ ] Speichern und erneutes Laden ergibt eine Konfiguration, die bei Serialisierung byte-identisch ist (Rundreise).
  - [ ] Nach „Als Standard speichern“ und Neustart ist die Konfiguration aktiv.
  - [ ] „Standard zurücksetzen“ fragt nach Bestätigung und löscht `default.osconfig`.
  - [ ] Laden einer Konfiguration während eines laufenden Crawls ist gesperrt (Hinweis); im pausierten Zustand nur für Optionen, die sich zur Laufzeit ändern lassen.
  - [ ] Der Konfigurationsdialog verwirft ungespeicherte Änderungen bei „Abbrechen“.
- **Testfixture:** Unit-Rundreise über alle Konfigurationsfelder mit Zufallswerten (Property-Test).

### OUT-221 Format der Konfigurationsdatei
- **Priorität:** P0
- **Beschreibung:** Menschenlesbares, versioniertes Textformat **(OS-Festlegung)**: TOML, UTF-8, mit Pflichtfeld `schema_version` (Ganzzahl) und `generator = "openspider <version>"`. Es werden **alle** Werte geschrieben (nicht nur Abweichungen), stabil sortiert nach Abschnitten, damit Dateien diff- und versionskontrollierbar sind. Kommentare sind erlaubt und werden beim Laden ignoriert. Konfigurationsdateien der Referenz können nicht importiert werden (proprietäres Format).
- **Akzeptanzkriterien:**
  - [ ] Ein JSON-Schema (`docs/spec/schemas/osconfig.schema.json`) beschreibt das Format; CI validiert alle Fixture-Konfigurationen dagegen.
  - [ ] Fehlende Felder → Standardwert; unbekannte Felder → Warnung mit Feldpfad, Laden wird fortgesetzt.
  - [ ] Typfehler (z. B. `max_depth = "abc"`) → Fehlermeldung mit Zeile/Spalte, Datei wird nicht geladen.
  - [ ] Handbearbeitete Datei mit Kommentaren lädt korrekt.
- **Testfixture:** `tests/fixtures/config/` mit gültigen, unvollständigen, fehlerhaften und zukünftigen (`schema_version` + 1) Dateien.

### OUT-222 Trennung von Crawl-Konfiguration und App-Einstellungen
- **Priorität:** P1
- **Beschreibung:** Rechner-/benutzerbezogene Einstellungen liegen nicht in der Crawl-Konfiguration, sondern unter „Einstellungen“ (macOS: App-Menü → Einstellungen; Windows: Datei → Einstellungen): Sprache, Theme, Speicherort der Crawls, Speicher-/Ressourcenlimits, Proxy, vertrauenswürdige Zertifikate, Benachrichtigungen/E-Mail-Konten, Aufbewahrung, Updates, Export-Standards, verbundene Konten. Datei: `settings.toml`.
- **Akzeptanzkriterien:**
  - [ ] Eine `.osconfig` enthält keine Werte aus `settings.toml`; Austausch einer Konfiguration zwischen Rechnern ändert keine lokalen Einstellungen.
  - [ ] Änderungen an Einstellungen, die einen Neustart erfordern, werden als solche gekennzeichnet.
- **Testfixture:** Unit-Test der Schlüsselmengen beider Formate (disjunkt).

### OUT-223 Authentifizierungsprofile
- **Priorität:** P1
- **Beschreibung:** Zugangsdaten für standardbasierte Authentifizierung (Basic, Digest, NTLM) und beim Formular-Login gespeicherte Cookies können als Profil exportiert und in CLI/Scheduling verwendet werden.
- **Eingaben/Konfiguration:** Konfiguration → Authentifizierung → Profile → Exportieren; Passphrase (für portable Dateien) oder „an diesen Rechner binden“ (Schlüssel im OS-Schlüsselbund).
- **Ausgabe:** `.osauth`-Datei, verschlüsselt (AEAD, z. B. XChaCha20-Poly1305; Schlüsselableitung Argon2id bei Passphrase) **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Datei enthält keine Klartext-Kennwörter/Cookies (Test sucht nach Testgeheimnis).
  - [ ] Falsche Passphrase → eindeutiger Fehler, kein Absturz.
  - [ ] An Rechner gebundenes Profil lässt sich auf anderem Rechner nicht entschlüsseln (klarer Hinweis).
  - [ ] CLI: Passphrase über `OPENSPIDER_AUTH_PASSPHRASE` oder interaktiv (TTY); nie als Argument **(OS-Festlegung)**.
- **Testfixture:** `out-schedule` (Basic-Auth + Formular-Login).

### OUT-224 Konfigurations-Unterschiede anzeigen
- **Priorität:** P2
- **Beschreibung:** Ein Fenster zeigt alle Abweichungen der aktuellen Konfiguration von den Werkseinstellungen bzw. von einer gewählten Datei (Tastenkürzel Strg/Cmd+Umschalt+C).
- **Ausgabe:** Tabelle `Setting`, `Current`, `Default`; kopierbar/exportierbar.
- **Akzeptanzkriterien:**
  - [ ] Nach Änderung von 3 Werten zeigt das Fenster genau 3 Zeilen.
- **Testfixture:** Unit-Test.

### OUT-225 Geheimnisse außerhalb von Konfigurationsdateien
- **Priorität:** P0
- **Beschreibung:** API-Schlüssel, OAuth-Tokens, Kennwörter und E-Mail-Zugangsdaten werden ausschließlich im OS-Schlüsselbund (macOS Keychain, Windows Credential Manager) gespeichert; Konfigurationen verweisen nur per Kennung darauf.
- **Akzeptanzkriterien:**
  - [ ] Volltextsuche nach einem eingegebenen Test-API-Schlüssel in Einstellungsordner, Crawl-Dateien, Logs und Exporten findet nichts.
  - [ ] Fehlt der Schlüsselbund-Eintrag (anderer Rechner), meldet die Integration „nicht verbunden“ statt eines kryptischen Fehlers.
- **Testfixture:** Integrationstest mit Mock-Keychain-Backend.

### OUT-226 Versionierung und Migration
- **Priorität:** P1
- **Beschreibung:** Konfigurationen, Aufgaben, Presets und Crawl-Dateien älterer OpenSpider-Versionen werden beim Laden migriert; Dateien neuerer Versionen werden mit Warnung geladen (Konfiguration) bzw. abgewiesen, wenn das Crawl-Schema inkompatibel ist.
- **Akzeptanzkriterien:**
  - [ ] Für jede Schemaänderung existiert ein Migrationstest mit einer Fixture-Datei der Vorversion.
  - [ ] Migration einer Crawl-Datei erzeugt vorher eine Sicherung `<name>.ospider.bak-v<alt>` oder arbeitet auf einer Kopie.
  - [ ] Crawl-Datei mit höherer Hauptschemaversion → Meldung „Bitte OpenSpider aktualisieren“, keine Veränderung der Datei.
- **Testfixture:** `tests/fixtures/migrations/`.

### OUT-227 Speicherorte je Betriebssystem
- **Priorität:** P0
- **Beschreibung:** Feste, dokumentierte Pfade (ermittelt über die `dirs`-Crate) **(OS-Festlegung)**:
  - Einstellungen: macOS `~/Library/Application Support/OpenSpider/`, Windows `%APPDATA%\OpenSpider\`.
  - Crawls (Standard): macOS `~/Library/Application Support/OpenSpider/crawls/`, Windows `%LOCALAPPDATA%\OpenSpider\crawls\`.
  - Logs: macOS `~/Library/Logs/OpenSpider/`, Windows `%LOCALAPPDATA%\OpenSpider\logs\`.
  - Cache (Chromium, temporäre Exporte): macOS `~/Library/Caches/OpenSpider/`, Windows `%LOCALAPPDATA%\OpenSpider\cache\`.
  - Umgebungsvariable `OPENSPIDER_HOME` überschreibt alle Basispfade (für portable Installationen und Tests).
- **Akzeptanzkriterien:**
  - [ ] Hilfe → Debug zeigt alle effektiven Pfade.
  - [ ] Mit `OPENSPIDER_HOME=/tmp/x` schreibt die App nichts außerhalb von `/tmp/x` (außer OS-Schlüsselbund und OS-Planer).
- **Testfixture:** Integrationstest mit `OPENSPIDER_HOME`.

### OUT-228 Crawl-Verwaltung
- **Priorität:** P1
- **Beschreibung:** Jeder Crawl wird automatisch fortlaufend gespeichert (auch bei Absturz/Stromausfall bis zum letzten Commit) und erscheint in der Crawl-Verwaltung (Datei → Crawls).
- **Eingaben/Konfiguration:** Aktionen: Öffnen, Umbenennen, in Projekt verschieben, Projekt anlegen/umbenennen, Duplizieren, Exportieren (`.ospider`), Importieren, Löschen, ID kopieren, Sperren/Entsperren, Zum Vergleich wählen; Filter „Anzeigen: Crawls / Vergleiche / alle“; Suche.
- **Ausgabe:** Tabelle `Project`, `Name`, `Mode`, `Start URL`, `Started`, `Duration`, `URLs`, `Size on Disk`, `Status` (Running/Paused/Completed/Incomplete/Failed), `Locked`, `ID`.
- **Akzeptanzkriterien:**
  - [ ] Nach erzwungenem Beenden (`kill -9`) während des Crawls erscheint der Crawl als `Incomplete`, ist öffenbar und fortsetzbar; nach Fortsetzen und Abschluss ist die Menge gecrawlter URLs identisch mit einem ununterbrochenen Crawl derselben Fixture.
  - [ ] Datenverlust bei `kill -9` höchstens die URLs der letzten 5 s bzw. der letzten 1 000 URLs (Commit-Intervall, je nachdem was früher eintritt) **(OS-Festlegung)**.
  - [ ] Löschen fragt nach Bestätigung, entfernt die Datei und gibt Speicherplatz frei (`Size on Disk` der Summe sinkt um die Dateigröße); gesperrte Crawls sind nicht löschbar (Schaltfläche deaktiviert, CLI-Löschversuch → Exit-Code 2).
  - [ ] Duplizieren erzeugt eine unabhängige Kopie mit neuer ID und Name `<Name> (Kopie)`; Änderungen an der Kopie (Umbenennen, Löschen) betreffen das Original nicht.
  - [ ] Umbenennen und Verschieben in ein anderes Projekt ändern die ID nicht; `--list-crawls` zeigt danach den neuen Namen/Projekt.
  - [ ] Suche filtert über `Name`, `Project` und `Start URL` (Teilstring, case-insensitiv); „Anzeigen“-Filter und Suche wirken kombiniert.
  - [ ] `Status`: laufender Crawl `Running`, pausierter `Paused`, regulär beendeter `Completed`, abgebrochener/abgestürzter `Incomplete`, mit Fehler beendeter `Failed`.
  - [ ] Import einer `.ospider`-Datei kopiert sie in den Crawl-Ordner und vergibt eine neue ID, wenn die ID schon existiert; Import einer Nicht-Crawl-Datei mit Endung `.ospider` → Fehlermeldung, nichts wird kopiert.
  - [ ] Tabelle bleibt bei 1 000 gespeicherten Crawls flüssig (< 500 ms Öffnen, Sortieren je Spalte < 200 ms).
- **Testfixture:** Integrationstest mit 3 Projekten und 1 000 synthetischen Crawl-Metadaten; `out-basic-site` für Absturz-/Fortsetzungstest; umbenannte Textdatei `fake.ospider`.

### OUT-229 Automatische Crawl-Aufbewahrung (Retention)
- **Priorität:** P1
- **Beschreibung:** Crawls werden nach einer einstellbaren Dauer automatisch gelöscht; gesperrte Crawls und Crawls in gesperrten Projekten sind ausgenommen. Sperren eines Projekts gilt auch für künftige Crawls darin (inkl. geplanter).
- **Eingaben/Konfiguration:** Einstellungen → Crawl-Aufbewahrung: `Nie` (Standard), `1 Woche`, `1 Monat`, `3 Monate`, `6 Monate`, `1 Jahr`, `Benutzerdefiniert (Tage)` **(OS-Festlegung** der Auswahlwerte); Bezugszeitpunkt = Crawl-Ende (bei unvollständigen: letzte Änderung).
- **Akzeptanzkriterien:**
  - [ ] Einstellung „1 Woche“ → Crawl mit Ende vor 8 Tagen wird beim nächsten App-Start bzw. spätestens 1 h später gelöscht; Crawl vor 6 Tagen bleibt.
  - [ ] Gesperrter Crawl bleibt unabhängig vom Alter.
  - [ ] Neuer Crawl in gesperrtem Projekt ist automatisch geschützt.
  - [ ] Crawls, die von einem gespeicherten Vergleich referenziert werden, werden trotzdem gelöscht; der Vergleich wird als „nicht verfügbar“ markiert (OUT-150), vorher erscheint ein Hinweis bei Aktivierung der Aufbewahrung.
  - [ ] Ein gerade geöffneter oder laufender Crawl wird nie gelöscht.
  - [ ] Jede automatische Löschung wird geloggt (Name, ID, Alter).
- **Testfixture:** Unit-Test mit manipulierbarer Uhr.

### OUT-230 Crawl-Datei speichern und öffnen
- **Priorität:** P0
- **Beschreibung:** Ein Crawl kann als einzelne portable Datei `*.ospider` (SQLite, WAL zusammengeführt) gespeichert und auf jedem unterstützten System geöffnet werden; Doppelklick öffnet die App (OUT-272). Pausierte Crawls lassen sich nach dem Öffnen fortsetzen.
- **Eingaben/Konfiguration:** Datei → Crawl speichern unter… / Crawl öffnen…; CLI `--save-crawl` (OUT-189), `--load-crawl` (OUT-184).
- **Ausgabe:** `*.ospider`: SQLite-Datei mit `application_id` = feste OpenSpider-Kennung und `user_version` = Schemaversion **(OS-Festlegung)**; enthält Crawl-Daten, die beim Crawl wirksame Konfiguration (als eingebettete `.osconfig`), Queue-Zustand und Metadaten (Version, Start-URL, Zeiten). Keine Geheimnisse (OUT-225).
- **Akzeptanzkriterien:**
  - [ ] Auf macOS arm64 gespeicherte Datei öffnet auf Windows x64 (und umgekehrt) mit byte-identischen CSV-Exporten von `Internal:All` und `Links:All Inlinks`.
  - [ ] Export erzeugt eine in sich geschlossene Datei (keine `-wal`/`-shm`-Begleitdateien nötig; Test öffnet die Datei nach Kopie ohne Begleitdateien).
  - [ ] Speichern eines 1-Mio.-URL-Crawls dauert < 60 s (SSD).
  - [ ] Pausierter Crawl (Queue mit 15 URLs) → speichern, App neu starten, öffnen, fortsetzen → genau diese 15 URLs werden gecrawlt; Ergebnis entspricht einem ununterbrochenen Crawl.
  - [ ] Die eingebettete Konfiguration lässt sich aus der Crawl-Datei als `.osconfig` extrahieren („Konfiguration aus Crawl übernehmen“) und ist identisch mit der beim Crawl verwendeten.
  - [ ] Datei ohne passende `application_id` (z. B. beliebige SQLite-Datei) → Meldung „keine OpenSpider-Crawl-Datei“, kein Absturz, keine Veränderung der Datei.
  - [ ] Beschädigte Datei (zufällige Bytes in der Mitte überschrieben) → Fehlermeldung mit Option „Reparaturversuch“ (SQLite-Integrity-Check; Reparatur schreibt in eine neue Datei `<name>.repaired.ospider`), kein Absturz.
  - [ ] Volltextsuche nach Testgeheimnissen (API-Schlüssel, Auth-Kennwort) in der Datei → 0 Treffer.
- **Testfixture:** gespeicherter Crawl aus `out-basic-site`, pausierter Crawl aus `out-queue`, absichtlich beschädigte Kopie, fremde SQLite-Datei.

### OUT-231 Speicherort der Crawl-Daten
- **Priorität:** P1
- **Beschreibung:** Der Ordner für Crawl-Dateien ist einstellbar; ungeeignete Ziele werden erkannt.
- **Akzeptanzkriterien:**
  - [ ] Netzlaufwerke (SMB/NFS) und cloud-synchronisierte Ordner (iCloud Drive, OneDrive, Dropbox, Google Drive) lösen eine deutliche Warnung aus (Datenverlustrisiko durch Sync von SQLite-Dateien); Speichern ist mit Bestätigung möglich.
  - [ ] Ordnerwechsel bietet an, vorhandene Crawls zu verschieben; Fortschritt + Abbruch möglich.
  - [ ] Freier Speicherplatz < 2 GB beim Crawlstart → Warnung; < 500 MB während des Crawls → Crawl pausiert automatisch mit Meldung.
- **Testfixture:** Unit-Tests mit gemocktem Dateisystem-Typ.

### OUT-232 Einzelinstanz und Dateizuordnung beim Öffnen
- **Priorität:** P1
- **Beschreibung:** Doppelklick auf `.ospider`/`.osconfig`/`.ospreset`/`.ostask` öffnet die laufende App-Instanz (keine zweite) und lädt die Datei bzw. fragt, ob die Konfiguration geladen werden soll.
- **Akzeptanzkriterien:**
  - [ ] Zweiter Start der GUI fokussiert die vorhandene Instanz und übergibt die Datei.
  - [ ] Öffnen eines Crawls bei laufendem Crawl fragt, ob der aktuelle pausiert werden soll.
- **Testfixture:** manueller Test je OS.

---

## 11 Logging & Diagnose

### OUT-240 Logdateien
- **Priorität:** P1
- **Beschreibung:** App und CLI schreiben strukturierte Logs in den Log-Ordner (OUT-227): `openspider.log` (App), `cli-<YYYYMMDD-HHMMSS>-<pid>.log` je CLI-Lauf, `task-<id>-<zeitstempel>.log` je geplantem Lauf.
- **Ausgabe:** Zeilenformat `<ISO-Zeitstempel> <LEVEL> <modul>: <nachricht> [key=value …]`; optional JSON-Zeilen (`--log-format json`).
- **Akzeptanzkriterien:**
  - [ ] Jede Zeile hat Zeitstempel mit Millisekunden und Zeitzonenoffset.
  - [ ] Crawl-Start und -Ende werden mit Konfigurations-Zusammenfassung (Modus, Start-URL, Limits, Rendering an/aus) geloggt.
  - [ ] Panics/unerwartete Fehler werden mit Backtrace geloggt, bevor der Prozess endet.
- **Testfixture:** CLI-Lauf auf `out-basic-site`, Prüfung der Logzeilen per Regex.

### OUT-241 Log-Level
- **Priorität:** P1
- **Beschreibung:** Stufen `error`, `warn`, `info` (Standard), `debug`, `trace`; einstellbar in Hilfe → Debug (App, wirkt sofort) bzw. `--log-level` und `OPENSPIDER_LOG` (CLI, Syntax wie `RUST_LOG` mit Modulfiltern).
- **Akzeptanzkriterien:**
  - [ ] `--log-level debug` protokolliert jede angefragte URL mit Status und Dauer; `info` nicht.
  - [ ] `OPENSPIDER_LOG=os_fetch=trace` erhöht nur das Level des Fetch-Moduls.
- **Testfixture:** CLI-Test.

### OUT-242 Rotation und Größenbegrenzung
- **Priorität:** P1
- **Beschreibung:** Logs wachsen nicht unbegrenzt.
- **Akzeptanzkriterien:**
  - [ ] `openspider.log` rotiert bei 10 MB; max. 5 Archive (`openspider.1.log` …), ältere werden gelöscht.
  - [ ] Lauf-Logs älter als 30 Tage werden beim App-Start gelöscht (Einstellung, Standard 30).
  - [ ] Gesamtgröße des Log-Ordners ≤ 500 MB; bei Überschreitung werden die ältesten Dateien zuerst entfernt.
- **Testfixture:** Unit-Test mit kleinen Grenzwerten.

### OUT-243 Schutz sensibler Daten in Logs
- **Priorität:** P0
- **Beschreibung:** Logs dürfen keine Geheimnisse enthalten.
- **Akzeptanzkriterien:**
  - [ ] Werte von `Authorization`, `Proxy-Authorization`, `Cookie`, `Set-Cookie`, API-Schlüsseln, OAuth-Tokens, Kennwörtern und Passphrasen werden als `<redacted>` geloggt.
  - [ ] Zugangsdaten in URLs (`https://user:pass@host/`) werden als `https://<redacted>@host/` geloggt.
  - [ ] Query-Parameter mit Namen `key`, `api_key`, `token`, `access_token`, `password`, `secret` werden maskiert.
  - [ ] Automatisierter Test: Crawl mit allen genannten Geheimnistypen, anschließend Suche nach den Klartextwerten in allen Logs → 0 Treffer.
- **Testfixture:** `out-schedule` + Mock-APIs mit Testschlüsseln.

### OUT-244 Diagnosepaket („Logs speichern“) und Debug-Informationen
- **Priorität:** P1
- **Beschreibung:** Hilfe → Debug zeigt Version, Build, OS/Architektur, CPU-Kerne, RAM gesamt/verfügbar, eingestellte Ressourcenlimits, effektive Pfade, Status der Chromium-Komponente, verbundene Integrationen (nur ja/nein). „Logs speichern“ erstellt ein ZIP für den Support.
- **Ausgabe:** `openspider-diagnostics-<zeitstempel>.zip` mit Logs (letzte 7 Tage), `system.txt`, Konfigurations-Diff gegen Standard (OUT-224, ohne Geheimnisse), Liste der Aufgaben (ohne Geheimnisse).
- **Akzeptanzkriterien:**
  - [ ] ZIP enthält keine Crawl-Daten und keine Geheimnisse (Test wie OUT-243).
  - [ ] Vor dem Speichern zeigt ein Dialog, welche Dateien enthalten sind.
  - [ ] CLI-Äquivalent: `openspider --diagnostics <zielordner>` **(OS-Festlegung)**.
- **Testfixture:** Integrationstest.

### OUT-245 Lokale Nutzungsstatistik
- **Priorität:** P2
- **Beschreibung:** Hilfe → Nutzungsstatistik zeigt eigene Nutzungsdaten über die Zeit (Anzahl Crawls, gecrawlte URLs, Exporte je Monat), kumulativ und nicht-kumulativ. Daten bleiben ausschließlich lokal; es gibt keine Telemetrie ohne ausdrückliche Zustimmung (Opt-in, Standard aus).
- **Akzeptanzkriterien:**
  - [ ] Nach 3 Crawls zeigt die Statistik 3 Crawls im aktuellen Monat.
  - [ ] Ohne Opt-in erfolgt kein Netzwerkzugriff außer Update-Prüfung (Netzwerkmitschnitt im Test).
- **Testfixture:** Unit-Test.

---

## 12 Update-Mechanismus

### OUT-250 Update-Prüfung
- **Priorität:** P1
- **Beschreibung:** Die Desktop-App prüft beim Start (höchstens einmal je 24 h) und manuell über Hilfe → „Nach Updates suchen“ ein signiertes Update-Manifest (z. B. auf GitHub Releases).
- **Eingaben/Konfiguration:** Einstellungen → Updates: „Automatisch nach Updates suchen“ (Standard an); Schlüssel `updates.check = false` in `settings.toml`; Umgebungsvariable `OPENSPIDER_DISABLE_UPDATE_CHECK=1` (für verwaltete Umgebungen) **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Die Anfrage überträgt nur Version, Betriebssystem und Architektur (keine Nutzerkennung).
  - [ ] Offline/Timeout (5 s) → keine Fehlermeldung beim Start, nur Log-Eintrag.
  - [ ] Abgeschaltet → kein Netzwerkzugriff auf den Update-Server (Netzwerkmitschnitt).
  - [ ] Das CLI prüft nie automatisch; `openspider --check-update` gibt verfügbare Version aus (Exit-Code 0 = aktuell, 10 = Update verfügbar).
- **Testfixture:** Mock-Update-Server im `os-testsite`.

### OUT-251 Herunterladen und Installieren
- **Priorität:** P1
- **Beschreibung:** Ein verfügbares Update wird im Hintergrund heruntergeladen; danach bietet die App „Jetzt installieren und neu starten“ oder „Beim Beenden installieren“ an. Release-Notes werden angezeigt.
- **Akzeptanzkriterien:**
  - [ ] Während eines laufenden Crawls wird kein Neustart erzwungen; Installieren ist erst nach Pause/Ende möglich (Hinweis).
  - [ ] Abgebrochener Download wird beim nächsten Start fortgesetzt oder verworfen, nie halb installiert.
  - [ ] Nach dem Update startet die App neu und öffnet den zuletzt geöffneten Crawl.
  - [ ] Update aktualisiert auch das CLI-Binary und registrierte Aufgaben (OUT-166).
- **Testfixture:** Mock-Update-Server, manueller Release-Test je OS.

### OUT-252 Integrität und Signatur von Updates
- **Priorität:** P0
- **Beschreibung:** Updates werden nur installiert, wenn die Paketsignatur (Ed25519, Tauri-Updater-Signatur) mit dem in der App eingebetteten öffentlichen Schlüssel übereinstimmt; zusätzlich muss die OS-Codesignatur (Apple Developer ID / Authenticode) gültig sein.
- **Akzeptanzkriterien:**
  - [ ] Manipuliertes Paket (1 Byte geändert) → Installation verweigert, Warnung, Log-Eintrag.
  - [ ] Manifest über HTTP (statt HTTPS) wird abgelehnt.
  - [ ] Downgrade über den Updater ist nicht möglich (Versionsvergleich nach SemVer).
- **Testfixture:** Mock-Update-Server mit gültigem und manipuliertem Paket.

### OUT-253 Updates in verwalteten Umgebungen
- **Priorität:** P2
- **Beschreibung:** Administratoren können Updates zentral steuern: MSI-Installationen per Softwareverteilung, Update-Prüfung per Richtlinie deaktiviert **(OS-Festlegung)**:
  - Windows: Registry `HKLM\Software\Policies\OpenSpider` bzw. `HKCU\Software\Policies\OpenSpider`, Wert `DisableUpdateCheck` (REG_DWORD); MSI-Eigenschaft `DISABLEUPDATECHECK=1` schreibt den HKLM-Wert bei der Installation.
  - macOS: verwaltete Einstellungen (Konfigurationsprofil) in der Domain `de.openspider.app`, Schlüssel `DisableUpdateCheck` (Boolean).
  - Rangfolge: HKLM/Profil > HKCU > Umgebungsvariable `OPENSPIDER_DISABLE_UPDATE_CHECK` > `settings.toml` (OUT-250).
- **Akzeptanzkriterien:**
  - [ ] `DisableUpdateCheck`=1 (bzw. `true`) → kein Netzwerkzugriff auf den Update-Server (Netzwerkmitschnitt über 2 App-Starts und manuelles „Nach Updates suchen“); der Menüpunkt zeigt „Durch Administrator deaktiviert“.
  - [ ] Wert `0`/`false` oder nicht vorhanden → Benutzereinstellung gilt.
  - [ ] Rangfolgetest mit allen Kombinationen von HKLM ∈ {fehlt, 0, 1} × HKCU ∈ {fehlt, 0, 1}: ein explizit gesetzter HKLM-Wert entscheidet immer (HKLM=0, HKCU=1 → Prüfung aktiv); nur wenn HKLM fehlt, entscheidet HKCU; fehlen beide, gilt die Benutzereinstellung.
  - [ ] Gesetzte Richtlinie überschreibt die Benutzereinstellung; die Option ist in der UI ausgegraut mit Hinweis „durch Richtlinie festgelegt“.
  - [ ] Ungültiger Typ (z. B. REG_SZ „ja“) → wird ignoriert, Warnung im Log.
  - [ ] `msiexec /i OpenSpider.msi /qn DISABLEUPDATECHECK=1` → Registry-Wert gesetzt; Deinstallation entfernt ihn.
- **Testfixture:** Unit-Tests des Richtlinien-Resolvers mit gemocktem Registry-/Defaults-Backend; manueller Test in Windows-VM und mit macOS-Konfigurationsprofil.

### OUT-254 Update-Kanäle
- **Priorität:** P2
- **Beschreibung:** Kanäle `stable` (Standard) und `beta`; Wechsel in den Einstellungen.
- **Akzeptanzkriterien:**
  - [ ] Wechsel von beta zu stable bietet erst bei einer neueren stable-Version ein Update an (kein Downgrade).
- **Testfixture:** Mock-Update-Server.

### OUT-255 Datenkompatibilität über Versionen
- **Priorität:** P0
- **Beschreibung:** Nach einem Update bleiben alle Crawls, Konfigurationen, Presets, Aufgaben und Einstellungen nutzbar (Migration gemäß OUT-226).
- **Akzeptanzkriterien:**
  - [ ] CI-Test: Crawl-Dateien und Konfigurationen jeder veröffentlichten Version (Fixture-Archiv) werden mit der aktuellen Version geöffnet und exportiert.
- **Testfixture:** `tests/fixtures/migrations/`.

---

## 13 Plattformanforderungen

### OUT-260 macOS Apple Silicon (arm64)
- **Priorität:** P0
- **Beschreibung:** Native arm64-Builds von App, CLI und gebündelten Komponenten (inkl. Chromium für Rendering).
- **Akzeptanzkriterien:**
  - [ ] Unterstützt macOS 12 (Monterey) und neuer **(OS-Festlegung)**.
  - [ ] Kein Prozess läuft unter Rosetta 2 (Prüfung über Aktivitätsanzeige/`sysctl.proc_translated`).
  - [ ] Vollständige Testsuite läuft in CI auf `macos-14` (arm64).
- **Testfixture:** CI-Matrix.

### OUT-261 macOS Intel (x86_64)
- **Priorität:** P0
- **Beschreibung:** Native x86_64-Builds; Auslieferung als getrennte DMGs je Architektur und zusätzlich als Universal-Binary (P1).
- **Akzeptanzkriterien:**
  - [ ] Unterstützt macOS 12 und neuer.
  - [ ] Vollständige Testsuite in CI auf `macos-13` (x64).
  - [ ] Das Update-Manifest liefert je Architektur das passende Paket.
- **Testfixture:** CI-Matrix.

### OUT-262 Windows x64
- **Priorität:** P0
- **Beschreibung:** Windows 10 (22H2) und Windows 11, 64 Bit.
- **Akzeptanzkriterien:**
  - [ ] Benötigte WebView2-Runtime wird vom Installer geprüft und bei Bedarf installiert (Bootstrapper, offline-Variante im MSI-Paket optional).
  - [ ] Pfade > 260 Zeichen funktionieren für Exporte und Crawl-Dateien (Long-Path-fähige APIs).
  - [ ] Funktioniert in einer Remote-Desktop-Sitzung (RDP): App-Start, Crawl mit JS-Rendering, Trennen und Wiederverbinden der Sitzung während des Crawls sowie Wechsel der Bildschirmauflösung führen nicht zum Absturz; der Crawl läuft weiter.
  - [ ] Vollständige Testsuite in CI auf `windows-latest`.
- **Testfixture:** CI-Matrix + manueller RDP-Test pro Release.

### OUT-263 Windows on ARM (arm64)
- **Priorität:** P2
- **Beschreibung:** Native arm64-Builds für Windows 11 on ARM. Bis dahin wird der x64-Build unter der Windows-Emulation unterstützt.
- **Akzeptanzkriterien:**
  - Phase 1 (Emulation, P1-Pflicht ab erstem Release):
    - [ ] x64-Installer (MSI und NSIS) installiert auf Windows 11 arm64 ohne Fehler.
    - [ ] Smoke-Suite unter Emulation besteht: Crawl von `out-basic-site` mit und ohne JS-Rendering, Export `Internal:All` byte-identisch zur x64-Referenz, XML-Sitemap-Erzeugung, geplante Aufgabe läuft zur geplanten Zeit.
    - [ ] Crawl-Geschwindigkeit unter Emulation ≥ 50 % der nativen x64-Leistung auf vergleichbarer Hardware (Richtwert, Messung dokumentiert).
  - Phase 2 (nativ, P2):
    - [ ] Eigene arm64-Pakete; der Installer wählt automatisch die passende Architektur, das Update-Manifest liefert je Architektur das passende Paket.
    - [ ] Kein Prozess (App, CLI, Chromium) läuft emuliert (Prüfung über Task-Manager-Spalte „Architektur“).
    - [ ] Vollständige Testsuite in CI auf einem Windows-arm64-Runner.
- **Testfixture:** Smoke-Suite `tests/smoke/` auf Windows-11-arm64-VM bzw. Runner.

### OUT-264 Linux
- **Priorität:** P2
- **Beschreibung:** Nicht Zielplattform des Projekts (Zielplattformen: macOS arm64/x64, Windows x64). **Bewusste Paritätslücke:** Die Referenz liefert Linux-Pakete für Ubuntu (`.deb`) und Fedora (`.rpm`) und seit Version 24.0 zusätzlich Arm64-Linux-Pakete (Ubuntu arm64, Fedora aarch64). OpenSpider bietet bis auf Weiteres keine Linux-Pakete und keine Linux-Desktop-App an; diese Lücke wird in der Paritätsmatrix (README/Website) ausdrücklich als „nicht unterstützt“ geführt. Der Kern und das CLI bleiben aber Linux-kompilierbar (x86_64, aarch64), damit Headless-Crawls auf Servern aus dem Quellcode möglich sind. Pakete `.deb`/`.rpm` (x86_64 und arm64) sind Kandidaten für eine spätere Version.
- **Akzeptanzkriterien:**
  - [ ] `cargo build -p os-cli` gelingt in CI auf `ubuntu-latest` (x86_64) und einem aarch64-Linux-Runner bzw. per Cross-Build (Build-Check, keine Release-Pflicht).
  - [ ] `openspider --crawl {BASE} --export-tabs "Internal:All"` auf Linux x86_64 liefert denselben Export wie auf macOS (CI-Job, Golden-Vergleich).
  - [ ] JS-Rendering ohne Display funktioniert mit Chromium headless (kein virtueller Framebuffer nötig).
  - [ ] Die Paritätsmatrix in der Doku listet Linux-Desktop und Linux-Pakete (x64/arm64) als bewusste Lücke mit Verweis auf diesen Eintrag.
- **Testfixture:** CI-Build-Job; `out-basic-site`.

### OUT-265 macOS: Paketierung, Signatur, Notarisierung
- **Priorität:** P0
- **Beschreibung:** Auslieferung als signiertes, notarisiertes DMG mit Hardened Runtime; App startet ohne Gatekeeper-Warnung.
- **Akzeptanzkriterien:**
  - [ ] `spctl --assess --type execute OpenSpider.app` → accepted; `stapler validate` → ok.
  - [ ] Alle eingebetteten Binärdateien (CLI, Chromium, Hilfsprozesse) sind signiert.
  - [ ] Benötigte Berechtigungen sind minimal (Netzwerk-Client; keine Kamera/Mikrofon; Zugriff auf vom Nutzer gewählte Dateien).
- **Testfixture:** Release-Pipeline-Check.

### OUT-266 Windows: Installer und Signatur
- **Priorität:** P0
- **Beschreibung:** Zwei Installer-Varianten: MSI (für Softwareverteilung, Per-Machine) und EXE/NSIS (Per-User, ohne Admin-Rechte). Alle Binärdateien Authenticode-signiert.
- **Eingaben/Konfiguration:** Stille Installation: `msiexec /i OpenSpider.msi /qn INSTALLDIR="…" ADDTOPATH=1`; NSIS: `OpenSpider-setup.exe /S /D=<pfad>`; stille Deinstallation entsprechend.
- **Akzeptanzkriterien:**
  - [ ] SmartScreen zeigt den verifizierten Herausgeber.
  - [ ] Stille Installation und Deinstallation ohne Dialoge (Exit-Code 0).
  - [ ] Deinstallation entfernt Programmdateien, Startmenü-Einträge, PATH-Eintrag und geplante Aufgaben; Benutzerdaten (Crawls, Einstellungen) bleiben, außer der Nutzer wählt „Benutzerdaten entfernen“ (interaktiv) bzw. `REMOVEUSERDATA=1`.
- **Testfixture:** Release-Pipeline-Test in Windows-VM.

### OUT-267 Hardware-Anforderungen und Ressourcenlimits
- **Priorität:** P1
- **Beschreibung:** Mindestens 4 GB RAM, empfohlen 8 GB (bis ~1 Mio. URLs) bzw. 16 GB (darüber), SSD empfohlen. Statt einer JVM-Speicherzuteilung gibt es einstellbare Obergrenzen.
- **Eingaben/Konfiguration:** Einstellungen → Ressourcen: „Max. Arbeitsspeicher für Caches“ (Standard: 25 % des RAM, min. 512 MB), „Max. parallele Render-Prozesse“ (Standard: CPU-Kerne / 2, max. 8).
- **Akzeptanzkriterien:**
  - [ ] Crawl von 1 Mio. URLs (synthetische Fixture, ohne Rendering) bleibt auf einem 8-GB-Rechner unter 2 GB RSS.
  - [ ] Bei Speicherknappheit (verfügbarer RAM < 10 %) drosselt der Crawler und warnt, statt abzustürzen.
  - [ ] Es gibt kein festes URL-Limit; ab 5 Mio. URLs erscheint ein Hinweis zur Laufzeit/Datenträgergröße.
- **Testfixture:** synthetischer Großcrawl im nächtlichen CI-Job.

### OUT-268 Dateisystem, Pfade, Zeichenkodierung
- **Priorität:** P0
- **Beschreibung:** Plattformneutrale Behandlung von Pfaden und Dateinamen.
- **Akzeptanzkriterien:**
  - [ ] Unicode-Pfade (`Jürgen`, `日本`) funktionieren für alle Ein-/Ausgaben auf beiden Plattformen.
  - [ ] Dateinamen aus URLs werden für beide Dateisysteme gültig gemacht (OUT-008), case-insensitive Kollisionen auf macOS/Windows werden mit Hash-Suffix aufgelöst.
  - [ ] Keine hartkodierten Pfadtrenner (Lint-Regel im Code-Review, siehe `CLAUDE.md`).
- **Testfixture:** Unit-Tests mit Pfad-Edge-Cases.

### OUT-269 Proxy
- **Priorität:** P1
- **Beschreibung:** Netzwerkzugriffe (Crawl, Integrationen, Updates, Google/E-Mail) können über einen Proxy laufen: „System-Proxy verwenden“ (Standard), „Manuell“ (HTTP/HTTPS/SOCKS5, Host, Port, optional Benutzer/Kennwort im Schlüsselbund), „Ausnahmen“ (Hostliste). (Crawl-spezifische Proxy-Details siehe Bereich Fetch, falls dort spezifiziert.)
- **Akzeptanzkriterien:**
  - [ ] Mit manuellem Proxy laufen alle Requests nachweislich über den Proxy (Test-Proxy im `os-testsite` zählt Requests).
  - [ ] PAC-Dateien des Systems werden ausgewertet (macOS/Windows), P2.
- **Testfixture:** Test-Proxy.

### OUT-270 Vertrauenswürdige Zertifikate
- **Priorität:** P1
- **Beschreibung:** Zusätzlich zum OS-Zertifikatsspeicher können eigene CA-Zertifikate hinterlegt werden (z. B. für TLS-inspizierende Unternehmensproxys). „Ermitteln“ zeigt den Aussteller des Zertifikats einer Test-URL; „Hinzufügen“ übernimmt es.
- **Eingaben/Konfiguration:** Dateien `.crt`, `.pem`, `.cer`, `.der`.
- **Akzeptanzkriterien:**
  - [ ] Fixture mit selbstsigniertem Zertifikat: ohne Eintrag → TLS-Fehler (No Response); nach Hinzufügen der CA → 200.
  - [ ] Ungültige Datei → Fehlermeldung, keine Änderung.
  - [ ] Standard: OS-Zertifikatsspeicher wird genutzt (rustls mit plattformeigenem Verifier).
- **Testfixture:** `out-security` (eigene Test-CA).

### OUT-271 Oberflächensprache und Theme
- **Priorität:** P1
- **Beschreibung:** UI-Sprachen Deutsch und Englisch (P0), weitere über Übersetzungsdateien (P2); Theme Hell/Dunkel/System.
- **Eingaben/Konfiguration:** Einstellungen → Darstellung: `Sprache` ∈ {System (Standard), Deutsch, English, …}; `Theme` ∈ {System (Standard), Hell, Dunkel}.
- **Akzeptanzkriterien:**
  - [ ] Erststart mit Systemsprache Deutsch → UI Deutsch; mit nicht unterstützter Systemsprache (z. B. Polnisch, solange keine Übersetzung existiert) → Englisch.
  - [ ] Sprachwechsel wirkt sofort für alle Fenster (ohne Neustart) **(OS-Festlegung)**; laufende Crawls werden davon nicht beeinflusst.
  - [ ] Sprachwechsel ändert nicht die kanonischen Exportnamen, Dateinamen (OUT-008), CLI-Namen und Spaltenköpfe der Exporte (außer Option „Spaltenüberschriften in UI-Sprache“, Abschnitt 0).
  - [ ] Vollständigkeit: CI-Prüfung, dass jeder Übersetzungsschlüssel in `de` und `en` vorhanden ist; fehlt ein Schlüssel in einer weiteren Sprache, wird der englische Text angezeigt (kein Schlüsselname in der UI).
  - [ ] Zahlen- und Datumsanzeige in der UI folgt der UI-Sprache (de: `1.234,5`; en: `1,234.5`), Exporte bleiben sprachunabhängig (Abschnitt 0).
  - [ ] Theme „System“ folgt dem OS-Wechsel Hell↔Dunkel innerhalb von 1 s ohne Neustart; Hell/Dunkel fest gewählt ignoriert den OS-Wechsel.
  - [ ] Kontrast aller Text-/Hintergrundpaare in beiden Themes ≥ 4,5:1 (WCAG AA; automatisierter Check der Design-Tokens).
  - [ ] Einstellung bleibt nach Neustart erhalten (`settings.toml`).
- **Testfixture:** UI-E2E-Test mit simulierter Systemsprache (de, en, pl) und simuliertem OS-Theme-Wechsel; i18n-Lint in CI.

### OUT-272 Dateizuordnungen und Desktop-Integration
- **Priorität:** P1
- **Beschreibung:** Registrierung der Dateitypen `.ospider`, `.osconfig`, `.osauth`, `.ospreset`, `.ostask` mit Icon; Startmenü/Launchpad-Eintrag; Anzeige des Crawl-Fortschritts im Dock-/Taskleisten-Symbol (P2).
- **Akzeptanzkriterien:**
  - [ ] Doppelklick auf `.ospider` öffnet die App mit dem Crawl (OUT-232).
- **Testfixture:** manueller Test je OS.

---

## 14 Benachrichtigungen & Automatisierungs-Schnittstellen

### OUT-280 E-Mail-Konten
- **Priorität:** P1
- **Beschreibung:** Konten für den Versand von Benachrichtigungen: SMTP (Host, Port, STARTTLS/TLS, Benutzer, Kennwort im Schlüsselbund), Gmail (OAuth, Versand-Berechtigung) und Microsoft 365/Outlook (OAuth). Testmail-Schaltfläche.
- **Eingaben/Konfiguration:** Einstellungen → Benachrichtigungen.
- **Akzeptanzkriterien:**
  - [ ] Testmail an Test-SMTP (`os-testsite`) kommt an; falsches Kennwort → verständliche Fehlermeldung mit SMTP-Antwortcode.
  - [ ] Unverschlüsseltes SMTP nur nach expliziter Bestätigung.
- **Testfixture:** Test-SMTP-Server.

### OUT-281 E-Mail bei Crawl-Ende (global und je Aufgabe)
- **Priorität:** P1
- **Beschreibung:** Global „E-Mail bei Crawl-Ende“ an festgelegte Empfänger für jeden Crawl; für geplante Aufgaben eigener Reiter „Benachrichtigungen“ mit aufgabenspezifischen Empfängern. Optional auch bei Fehlschlag (Standard an) **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Crawl in der UI mit globaler Option → eine Mail nach Ende.
  - [ ] Fehlgeschlagener geplanter Lauf → Mail mit Betreff-Präfix `[FAILED]` und Fehlermeldung.
  - [ ] Entfernen eines Empfängers macht die Aufgabe nicht ungültig (Referenzfehler 21.3 nicht übernehmen); ohne Empfänger wird keine Mail gesendet.
- **Testfixture:** Test-SMTP-Server.

### OUT-282 Inhalt der Abschlussmail
- **Priorität:** P1
- **Beschreibung:** Die Mail (HTML + Klartext-Alternative) enthält: Aufgabe/Projekt, Start-URL, Start/Ende/Dauer, gecrawlte URLs, Verteilung der Antwortcodes, indexierbar/nicht indexierbar, eine Tabelle aller Issues (Name, Typ, Priorität, URLs). Ist Auto-Compare aktiv (OUT-152), zusätzlich je Issue die Veränderung zum Vorlauf (Previous, Current, Change) und hervorgehobene starke Veränderungen (z. B. Anstieg nicht indexierbarer Seiten, verschwundene Titles, neue/fehlende Seiten).
- **Eingaben/Konfiguration:** Schwelle „starke Veränderung“: Änderung ≥ 10 % und ≥ 10 URLs **(OS-Festlegung)**.
- **Akzeptanzkriterien:**
  - [ ] Zahlen in der Mail stimmen mit dem Crawl Overview überein.
  - [ ] Mit Vergleich: Fixture `out-compare` → Mail nennt 3 neue und 2 fehlende Seiten.
  - [ ] Mail enthält keine Geheimnisse und keine vollständigen Exporte im Text (nur als Anhang, OUT-283).
- **Testfixture:** Test-SMTP-Server, Golden-Text der Klartext-Variante.

### OUT-283 Exporte als E-Mail-Anhang
- **Priorität:** P2
- **Beschreibung:** Gewählte Exporte werden gezippt an die Abschlussmail angehängt; „Leere Exporte überspringen“ (OUT-012) gilt auch für Anhänge.
- **Eingaben/Konfiguration:** Auswahl der anzuhängenden Exporte (aus der Exportauswahl der Aufgabe); max. Anhanggröße (Standard 20 MB).
- **Akzeptanzkriterien:**
  - [ ] ZIP größer als Limit → Mail ohne Anhang mit Warnhinweis und Pfad/Link zum Exportordner.
  - [ ] ZIP-Dateinamen entsprechen OUT-008.
- **Testfixture:** Test-SMTP-Server.

### OUT-284 Desktop-Benachrichtigung
- **Priorität:** P2
- **Beschreibung:** Systembenachrichtigung (macOS Notification Center, Windows-Info-Center/Toast) bei Crawl-Ende, Crawl-Fehler/-Abbruch, Abschluss der Crawl-Analyse und Abschluss eines Exports, der länger als 10 s gedauert hat **(OS-Festlegung** der Schwelle), sofern die App nicht im Vordergrund ist.
- **Eingaben/Konfiguration:** Einstellungen → Benachrichtigungen → Desktop: Hauptschalter (Standard an) und je Ereignis ein Schalter.
- **Ausgabe:** Titel `OpenSpider`; Text z. B. „Crawl abgeschlossen: site.test – 1 234 URLs, 12 Fehler (4xx/5xx)“, „Crawl fehlgeschlagen: <Grund>“, „Export fertig: internal_all.csv“.
- **Akzeptanzkriterien:**
  - [ ] App im Hintergrund, Crawl endet → genau eine Benachrichtigung mit Host, Anzahl gecrawlter URLs und Anzahl 4xx+5xx (Zahlen = Crawl Overview).
  - [ ] App im Vordergrund → keine Systembenachrichtigung (nur In-App-Hinweis).
  - [ ] Export von 8 s Dauer → keine Benachrichtigung; von 12 s → eine.
  - [ ] Klick auf die Benachrichtigung bringt die App mit dem betreffenden Crawl (bzw. bei Export mit geöffnetem Zielordner-Hinweis) in den Vordergrund.
  - [ ] Hauptschalter aus → keine Benachrichtigungen; einzelnes Ereignis aus → nur dieses unterdrückt.
  - [ ] Hat der Nutzer Benachrichtigungen auf OS-Ebene verweigert (macOS-Berechtigung), zeigt die Einstellungsseite einen Hinweis mit Link zu den Systemeinstellungen; kein Fehler, kein erneutes Nachfragen bei jedem Crawl.
  - [ ] CLI- und geplante Läufe (OUT-163) erzeugen keine Desktop-Benachrichtigungen (dafür OUT-281).
- **Testfixture:** Benachrichtigungs-Adapter mit Mock-Backend (Unit-Test der Auslöselogik inkl. Vordergrund-Status und Dauer-Schwelle); manueller Test je OS.

### OUT-285 MCP-Server (Steuerung durch KI-Assistenten)
- **Priorität:** P2
- **Beschreibung:** Optionaler, lokal laufender MCP-Server, über den KI-Assistenten Crawls starten, Fortschritt abfragen (inkl. Crawl-ID), Daten abfragen und Exporte/Reports auslösen können – technisch eine Hülle um dieselben Funktionen wie das CLI (Abschnitt 8). Transporte: STDIO (headless) und Streamable HTTP auf `127.0.0.1` (Port konfigurierbar). Standardmäßig deaktiviert. (Falls ein anderer Spec-Bereich Integrationen/KI verantwortet, gilt dessen Definition; hier nur die Betriebsaspekte.)
- **Akzeptanzkriterien:**
  - [ ] HTTP-Transport bindet ausschließlich an Loopback; Zugriff von anderen Hosts ist nicht möglich.
  - [ ] Exporte über MCP verwenden dieselben kanonischen Namen und erzeugen dieselben Dateien wie das CLI.
  - [ ] MCP-Logausgaben gehen nie auf stdout im STDIO-Modus (würde das Protokoll stören).
  - [ ] Große Ergebnisse werden seitenweise bzw. als Datei geliefert statt in einer übergroßen Antwort.
- **Testfixture:** MCP-Client-Test gegen `out-basic-site`.

---

## Anhang A – Fixture-Übersicht dieses Bereichs

| Fixture | Zweck | Genutzt von |
|---|---|---|
| `out-basic-site` | kleine Website mit allen Inhaltstypen und je einem Fehlerfall | Engine, Formate, CLI, Multi-Export |
| `out-csv-edge` | Sonderzeichen, Zeilenumbrüche, Formeln, Kodierungsfehler | OUT-002, -003, -016 |
| `out-list-order` | Listen mit Duplikaten, ungültigen Zeilen, Schema-losen URLs | OUT-015, -085, -183 |
| `out-segments` | Pfad-Segmente Blog/Shop/Info | OUT-011, -113, -130, -213 |
| `out-detail-tabs` | Inlinks/Canonical, JSON-LD, Rechtschreibung/Grammatik, Nah-Duplikat, Accessibility, Konsole, N-grams, Header | OUT-021 – -027, -052 |
| `out-site-structure` | Tiefenkette bis 11, 404 in Tiefe 1 | OUT-025, -080 |
| `out-crawl-path` | lineare Tiefe, Sitemap-Orphan | OUT-023 |
| `out-queue` | Crawl-Limit mit Restwarteschlange | OUT-030 |
| `out-links`, `out-links-anchor`, `out-links-follow`, `out-uncrawlable-links` | Linktypen, Anker, follow/nofollow, nicht crawlbare Links | OUT-031 – -042 |
| `out-render`, `out-pdf`, `out-headers` | Rendering, PDFs, Header, Cookies | OUT-043 – -049, -107, -110, -111 |
| `out-response-codes`, `out-security` | alle Statuscodes inkl. HTTP-Refresh-Header, TLS, Mixed Content, externe HTTP-Ziele | OUT-050, -051, -067, -080, -097, -270 |
| `out-content-dupes`, `out-images`, `out-canonicals`, `out-directives`, `out-structured-data`, `out-sitemaps-in`, `out-amp`, `out-custom-search`, `out-custom-extraction`, `out-a11y`, `out-issues` | filterbasierte Bulk-Exporte | OUT-052 – -066 |
| `out-redirects`, `out-pagination`, `out-hreflang`, `out-serp`, `out-mobile` | Reports (inkl. HSTS-Host, `Refresh`-Header, gemischte Ketten) | OUT-081 – -109 |
| `out-sitemap-gen`, `out-sitemap-images` | Sitemap-Generator | OUT-120 – -133 |
| `out-compare`, `out-compare-mapping`, `out-compare-ua` | Crawl-Vergleich | OUT-140 – -153 |
| `out-schedule` | Sitemap-Index, Formular-/Basic-Auth | OUT-162, -187, -209, -223 |
| Mocks im `os-testsite` | Google Sheets/Drive, GA4/GSC, PSI, KI (Text/Bild/Audio/Embeddings), SMTP, Update-Server, Proxy | OUT-005, -021, -025, -064, -065, -099, -104 – -106, -210 – -214, -250, -269, -280 |

## Anhang B – Offene Punkte / Annahmen

- Die vollständigen Spaltenlisten der Haupt-Tabs und die exakte Filterliste je Tab stammen aus den anderen Spec-Bereichen; dieses Dokument verweist über die Filter-Registry darauf (Registry-Tests in OUT-020, -054, -055, -066, -181, -211).
- Spaltennamen einzelner Bulk-Exporte und Reports sind dort, wo die öffentliche Doku sie nicht nennt, als **(OS-Festlegung)** gekennzeichnet; Paritätsabgleich erfolgt über manuell erzeugte Golden-Exporte der Referenz (Free-Version, ≤ 500 URLs, siehe `docs/ARCHITECTURE.md`).
- Die Referenz nennt für Google Sheets ein Limit von 5 Mio. Zellen; Google erlaubt inzwischen 10 Mio. – OUT-005 macht das Limit konfigurierbar.
- Die Referenz teilt Sitemaps ab 49 999 URLs; das Protokoll erlaubt 50 000. OpenSpider übernimmt 49 999 als Standard (OUT-129).
- Revision nach Spec-Review (2026-10-13, Score 86, „needs_work“) – Prüfstand gegen öffentliche Quellen:
  - Belegt in der Referenz-Doku bzw. den Release-Notes: Response-Code-Filter `Redirection (HTTP Refresh)` und Bulk-Export `All Error Inlinks` (beide 23.0, OUT-050/-067/-080); Change-Detection-Filterliste mit 14 Elementen beginnend mit `Indexability` und Content-Schwelle „mehr als 10 %“ (OUT-144/-145); Site-Structure-Tiefen 1 bis 10+ nach Antwortcode (OUT-025); Spelling-&-Grammar-Panel mit den 100 häufigsten Fehlern und URL-Anzahl (OUT-025); CLI-Syntax `--project-crawl-comparison "true"` (OUT-152/-202); Looker-Studio-Export mit Datums- und URL-Feld, frei wählbarer Metrikreihenfolge und separat verlinkter Datentyp-Tabelle (OUT-210/-214); HSTS-Policy als interne 307 (OUT-081); Insecure-Content-Report umfasst HTTP-Links, Canonicals, Paginierung und Mixed Content ohne Einschränkung auf interne Ziele (OUT-097); Arm64-Linux-Pakete seit 24.0 (OUT-264).
  - Entfernte, nicht belegte Versionsangaben: FAQ-Rich-Result-Entfernung „24.2“ (OUT-102), CLI-Namensliste „seit 24.2“ (OUT-203), RDP-Absturz als Referenzproblem (OUT-262; laut Review in 20.4 behoben, nun eigenes Kriterium). Die Release-History führt 24.1–24.3 nur als Bugfix-Versionen.
  - Noch per Golden-Export zu bestätigen: getrennte Bulk-Exporte `Content:Spelling Errors`/`Content:Grammar Errors` (OUT-052, OpenSpider registriert vorsorglich beide Varianten plus den kombinierten Export); exakte Werteliste `Chain Type` inkl. `HTTP Refresh` (OUT-081); Verfügbarkeit von `All Error Inlinks` im Bereich „Internal & External“ (OUT-067); Spaltennamen der Detail-Exporte Lighthouse/Accessibility/N-grams (OUT-021).
  - Clean-Room-Hinweis: `Description`/`How To Fix` (OUT-025, OUT-063) und die Kurzbeschreibungen in `Grammar Errors:Detail` (OUT-052) werden eigenständig verfasst; Issue-Texte der Referenz dürfen weder kopiert noch als Vorlage für Umformulierungen genutzt werden.

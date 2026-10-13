# PARITY – Paritätsprüfung der OpenSpider-Spezifikation gegen Screaming Frog SEO Spider 24.3

Stand: 2026-10-13 · Prüfer: Fable (Fortschrittskontrolle) · Geprüft: `docs/spec/01-crawl.md` … `07-performance.md` (Revision 2) · Referenz: öffentliche Doku (User Guide „Configuration“, „Tabs“, „General“), Release History und Release-Notes 18.0–24.3 des Referenzprodukts.

Diese Datei ist ein **Prüfbericht**, keine Funktionsliste. Sie ändert keine Spec-Einträge; Korrekturen werden als Vorschläge mit ID genannt und sind vom Planer (Opus) in die Bereichsdokumente zu übernehmen. Status aller Funktionen = **offen** (noch nichts implementiert).


## 1 Zusammenfassung

- **943 Funktions-IDs** in 7 Bereichen (P0: 291, P1: 507, P2: 145; Zählung nach der *ersten* genannten Priorität bei Mischangaben, siehe Abschnitt 5).
- Keine doppelten IDs, Nummernlücken sind absichtliche Blockreserven. Eine Reihenfolgeabweichung: INTEG-189 steht im Dokument vor INTEG-187/188.
- **Funktionale Abdeckung der Referenz 24.3: sehr hoch.** Alle Tabs und Filter der Referenz (33 Haupt-Tabs, 17 untere Detail-Tabs, 8 Seitenleisten-Panels), alle Konfigurationsabschnitte, Bulk-Exporte, Reports, Sitemaps, Compare/Change Detection, Scheduling, CLI, APIs, KI/Embeddings, MCP sowie die Neuerungen 19.0–24.3 (inkl. 24.1/24.2-Kleinupdates) sind abgebildet. Es wurden **3 kleine echte Lücken** und **2 nur teilweise abgedeckte Punkte** gefunden (Abschnitt 3).
- **Hauptproblem ist nicht Vollständigkeit, sondern Mehrfachspezifikation:** ca. 25 Themen sind in zwei bis drei Bereichsdokumenten parallel spezifiziert; in **17 Fällen widersprechen sich Priorität, Default oder Definition** (Abschnitt 4). Diese Konflikte müssen vor der Umsetzung aufgelöst werden, sonst implementiert Sonnet je nach gelesener Datei unterschiedlich.
- **Gesamtbewertung Vollständigkeit: 93 / 100** (Herleitung in Abschnitt 6).


## 2 Zählung

### 2.1 Je Bereich und Priorität

| Bereich (Dokument) | P0 | P1 | P2 | Gesamt |
|---|---:|---:|---:|---:|
| 01 Crawl (CRAWL) | 52 | 64 | 28 | 144 |
| 02 Onpage (ONPAGE) | 77 | 64 | 9 | 150 |
| 03 Technisch (TECH) | 45 | 118 | 11 | 174 |
| 04 Integrationen (INTEG) | 28 | 94 | 32 | 154 |
| 05 UI (UI) | 37 | 39 | 20 | 96 |
| 06 Output/Betrieb (OUT) | 47 | 124 | 45 | 216 |
| 07 Performance (PERF) | 5 | 4 | 0 | 9 |
| **Summe** | **291** | **507** | **145** | **943** |

Abgleich mit den Eigenzählungen der Dokumente: 04 nennt „154 Funktionen (P0 28, P1 94, P2 32)“ – stimmt mit dieser Zählung überein. 01, 02, 03, 05, 06 führen keine Summenzeile; Zählung hier maßgeblich.

### 2.2 Je Unterabschnitt

| Dokument · Abschnitt | P0 | P1 | P2 | Gesamt |
|---|---:|---:|---:|---:|
| 01 · A. Crawl-Kern | 17 | 7 | 1 | 25 |
| 01 · B. Crawl-Modi | 3 | 6 | 3 | 12 |
| 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | 9 | 7 | 4 | 20 |
| 01 · D. XML-Sitemaps | 0 | 3 | 0 | 3 |
| 01 · E. Extraktion | 0 | 4 | 1 | 5 |
| 01 · F. Limits | 3 | 5 | 3 | 11 |
| 01 · G. Rendering | 1 | 6 | 4 | 11 |
| 01 · H. Erweitert | 2 | 5 | 3 | 10 |
| 01 · I. robots.txt | 3 | 2 | 0 | 5 |
| 01 · J. URL-Umschreibung | 0 | 3 | 2 | 5 |
| 01 · K. Bereichssteuerung | 2 | 2 | 0 | 4 |
| 01 · L. Geschwindigkeit, User-Agent, Header | 3 | 2 | 0 | 5 |
| 01 · M. Authentifizierung und Proxy | 1 | 4 | 2 | 7 |
| 01 · N. Speicher, Persistenz, Steuerung | 6 | 5 | 3 | 14 |
| 01 · O. Ergänzungen | 2 | 3 | 2 | 7 |
| 02 · 1 Gemeinsame Mechanik | 7 | 2 | 0 | 9 |
| 02 · 2 Internal | 11 | 4 | 0 | 15 |
| 02 · 3 External | 2 | 0 | 0 | 2 |
| 02 · 4 Security | 2 | 12 | 0 | 14 |
| 02 · 5 Response Codes | 10 | 7 | 1 | 18 |
| 02 · 6 URL | 8 | 4 | 0 | 12 |
| 02 · 7 Page Titles | 6 | 4 | 0 | 10 |
| 02 · 8 Meta Description | 6 | 3 | 0 | 9 |
| 02 · 9 Meta Keywords | 0 | 4 | 0 | 4 |
| 02 · 10 H1 | 5 | 2 | 0 | 7 |
| 02 · 11 H2 | 5 | 1 | 0 | 6 |
| 02 · 12 Content | 5 | 14 | 8 | 27 |
| 02 · 13 Images | 7 | 5 | 0 | 12 |
| 02 · 14 Querschnitt | 3 | 2 | 0 | 5 |
| 03 · Gemeinsame Grundlagen | 6 | 3 | 0 | 9 |
| 03 · Canonicals | 9 | 7 | 0 | 16 |
| 03 · Pagination | 0 | 12 | 1 | 13 |
| 03 · Directives | 9 | 9 | 1 | 19 |
| 03 · Hreflang | 8 | 12 | 0 | 20 |
| 03 · JavaScript | 0 | 13 | 2 | 15 |
| 03 · Links | 7 | 10 | 0 | 17 |
| 03 · AMP | 0 | 0 | 4 | 4 |
| 03 · Structured Data | 0 | 11 | 0 | 11 |
| 03 · Sitemaps | 6 | 4 | 0 | 10 |
| 03 · Mobile | 0 | 8 | 0 | 8 |
| 03 · Accessibility | 0 | 9 | 1 | 10 |
| 03 · Validation | 0 | 11 | 2 | 13 |
| 03 · Change Detection | 0 | 9 | 0 | 9 |
| 04 · A Gemeinsame API-Infrastruktur | 9 | 8 | 2 | 19 |
| 04 · B Custom Search | 8 | 7 | 1 | 16 |
| 04 · C Custom Extraction | 11 | 3 | 2 | 16 |
| 04 · D Custom JavaScript | 0 | 6 | 7 | 13 |
| 04 · E Custom Link Positions | 0 | 4 | 1 | 5 |
| 04 · F Google Analytics 4 | 0 | 12 | 0 | 12 |
| 04 · G Search Console – Search Analytics | 0 | 8 | 0 | 8 |
| 04 · H Search Console – URL Inspection | 0 | 8 | 1 | 9 |
| 04 · I PageSpeed Insights / Lighthouse | 0 | 14 | 4 | 18 |
| 04 · J Linkmetriken | 0 | 6 | 2 | 8 |
| 04 · K KI-Anbieter & Prompts | 0 | 14 | 6 | 20 |
| 04 · L Embeddings & semantische Analyse | 0 | 4 | 6 | 10 |
| 05 · A. Hauptfenster | 14 | 4 | 2 | 20 |
| 05 · B. Unteres Detailfenster | 5 | 11 | 5 | 21 |
| 05 · C. Rechte Seitenleiste | 5 | 6 | 2 | 13 |
| 05 · D. Crawl-Analyse & Link Score | 2 | 1 | 0 | 3 |
| 05 · E. Visualisierungen | 0 | 7 | 5 | 12 |
| 05 · F. Interaktion | 3 | 2 | 3 | 8 |
| 05 · G. Einstellungen | 2 | 5 | 3 | 10 |
| 05 · H. Performance | 6 | 3 | 0 | 9 |
| 06 · 1 Export-Engine & Formate | 6 | 7 | 3 | 16 |
| 06 · 2 Tab- und Detail-Exporte | 2 | 3 | 3 | 8 |
| 06 · 3 Bulk-Exporte | 4 | 28 | 7 | 39 |
| 06 · 4 Reports | 4 | 23 | 7 | 34 |
| 06 · 5 XML- und Bild-Sitemaps | 3 | 9 | 2 | 14 |
| 06 · 6 Crawl-Vergleich | 0 | 9 | 5 | 14 |
| 06 · 7 Geplante Crawls | 0 | 9 | 3 | 12 |
| 06 · 8 Kommandozeile / Headless | 14 | 12 | 4 | 30 |
| 06 · 9 Looker-Studio-Export | 0 | 3 | 2 | 5 |
| 06 · 10 Konfigurationsdateien & Crawl-Dateien | 5 | 7 | 1 | 13 |
| 06 · 11 Logging & Diagnose | 1 | 4 | 1 | 6 |
| 06 · 12 Update-Mechanismus | 2 | 2 | 2 | 6 |
| 06 · 13 Plattformanforderungen | 6 | 5 | 2 | 13 |
| 06 · 14 Benachrichtigungen & Automatisierungs-Schnittstellen | 0 | 3 | 3 | 6 |
| 07 · Stabilität & Performance | 5 | 4 | 0 | 9 |

## 3 Gefundene Lücken (gegen Referenz 24.3)

Legende: **Lücke** = Referenzfunktion ohne Spec-Eintrag; **teilweise** = Eintrag vorhanden, Referenzdetail fehlt. Vorgeschlagene IDs nutzen freie Nummern des jeweiligen Blocks.

| # | Referenzfunktion (Version) | Befund | Vorgeschlagene ID / Maßnahme | Prio-Vorschlag |
|---|---|---|---|---|
| G-1 | Bulk Export › Links › „Non-Indexable Page Inlinks Only“ (23.3) | **Lücke.** Der Filter existiert (TECH-142), aber im Bulk-Export-Katalog (OUT-031 … OUT-041) fehlt der Eintrag; die generische Regel OUT-066 schließt den Links-Tab ausdrücklich nicht ein. TECH-142 nennt keinen Export. | **OUT-068** „Links: Non-Indexable Page Inlinks Only“ (Standard-Linkformat, Inlinks auf Seiten, die nur Inlinks von nicht indexierbaren Seiten haben); Querverweis in TECH-142 ergänzen. | P1 |
| G-2 | „Reset Columns for All Tables“ (Spaltenlayout aller Tabellen zurücksetzen, 23.1) | **Lücke.** UI-009 hat nur „Auf Standard zurücksetzen“ für die *aktuelle* Tabelle; ein globales Zurücksetzen aller Tabellen fehlt (Referenz: Kontextmenü Spaltenkopf). | **UI-021** „Spaltenlayout aller Tabellen zurücksetzen“ (Kontextmenü Spaltenkopf + Einstellungen; setzt Sichtbarkeit, Reihenfolge, Breite und Sortierung aller Tabs zurück; UI-125-Persistenz wird geleert). | P1 |
| G-3 | HTTP-Header-Voreinstellungen für Shopify-Bot-Authentifizierung (23.1) | **Lücke (klein).** CRAWL-115 nennt Voreinstellungen (Accept-Language, Cookie, Referer, Authorization …), die seit 23.1 vorhandenen Header-Namen für die Shopify-Bot-Verifizierung fehlen. | Kein neuer Eintrag: Akzeptanzkriterium in **CRAWL-115** ergänzen („Voreinstellungsliste enthält die Header-Namen für Shopify-Bot-Authentifizierung“); Namen aus der Referenz-UI per Black-Box-Abgleich (Q-Punkt in 01). | P2 |
| G-4 | Looker-Studio/Data-Studio-Export: Warnung in der Aufgabe, wenn gewählte Metriken nach einem Update nicht mehr befüllt werden (23.0, PSI-Umbau) | **teilweise.** OUT-212 regelt Spaltenstabilität (entfernte Metrik → leere Werte), aber kein Nutzerhinweis beim Öffnen/Lauf einer Aufgabe mit verwaisten Metriken. | Akzeptanzkriterium in **OUT-211** oder **OUT-212**: Aufgabe mit nicht mehr existierender Metrik zeigt beim Bearbeiten eine Warnung und schreibt beim Lauf einen Log-Hinweis. | P2 |
| G-5 | CLI: Bei ungültigem Report-/Export-Namen werden die verfügbaren Namen aufgelistet (24.2) | **teilweise.** OUT-203 schlägt ähnliche Namen vor (Levenshtein ≤ 3), listet aber nicht alle gültigen Namen; `--help`-Unterbefehl zum Auflisten der Export-/Report-Namen fehlt (OUT-181 nur Hilfe/Version). | Akzeptanzkriterium in **OUT-203**: bei fehlendem Ähnlichkeitstreffer vollständige Liste des betroffenen Katalogs ausgeben; optional **OUT-181** um `--list-exports` erweitern. | P1 |

**Geprüft und vorhanden** (Auswahl der Neuerungen 20.0–24.3, damit niemand erneut sucht): MCP-Server inkl. Autostart, Werkzeugliste, Markdown-Download (UI-129, OUT-285) · Auto Compare (OUT-152/TECH-308/CRAWL-036) · Änderungstabelle in der Abschlussmail (OUT-282) · Export-Anhänge gezippt mit Größenwarnung (OUT-283) · Pages With Uncrawlable Internal Outlinks + Spalte Link Crawlability (TECH-143/131, OUT-041, CRAWL-053) · Usage Stats kumulativ/nicht kumulativ (UI-128, OUT-245) · Arm64-Linux (OUT-264, bewusste Paritätslücke) · Crawl Invalid Links (CRAWL-054) · Leere Exporte überspringen (OUT-012) · Ahrefs Land/Volumen (INTEG-157) · Legenden-Toggle im Cluster-Diagramm (INTEG-186) · Live-Modellvalidierung (INTEG-160) · systemweiter System-Prompt (INTEG-177) · Tokenverbrauch (INTEG-178) · Ollama-Bildgenerierung und Request-Timeout (INTEG-166/179) · zuletzt gecrawlte URLs im Scheduling-Seed (CRAWL-136) · GSC-Read-Timeout 2 min (INTEG-128) · axe 4.11 (TECH-250) · FAQ-Rich-Result entfernt (TECH-188) · Limit je Subdomain (CRAWL-071) · Move Up/Down (INTEG-033/055) · Prozent-Kodierung (CRAWL-106) · All Error Inlinks / Redirects to Error (ONPAGE-067, OUT-067, OUT-084) · Redirection (HTTP Refresh) (ONPAGE-059) · Crawl Retention + Sperren (OUT-229, CRAWL-128) · Embedding-Regeln (INTEG-184/ONPAGE-162) · Links in Visualisierungen und Cluster (UI-097, INTEG-186) · Indexability-Spalten im Directives-Tab (TECH-051) · 2-MB-Grenzwerte konfigurierbar (TECH-289/290, CRAWL-140) · Semantic Search Mindestscore (INTEG-185) · Segmente aus Issues (INTEG-165, CRAWL-134) · APIs-Modus (INTEG-012) · Google-Sheets-Quelle und mehrere Sitemaps im Listenmodus (CRAWL-027) · Multi-Export mit Presets (OUT-075) · Konsolidierte Arbeitsmappe (OUT-007) · Spalten-Konfigurator (UI-009) · Konfig-Diff (UI-116) · Visualisierungen im externen Browser (OUT-026) · Moz v3 / Majestic Topics (INTEG-154/156) · N-grams (UI-049) · Custom JS (INTEG-060 ff.) · Accessibility (TECH-250 ff.) · Carbon (TECH-291/292) · Visual Custom Extraction (INTEG-054) · 3D-Visualisierungen (UI-098) · GA4 (INTEG-090 ff.) · PDF-Parsing (CRAWL-057/064, ONPAGE-166) · Validation-Tab (TECH-280 ff.) · In-App-Updates (OUT-250 ff.) · Auth für Scheduling/CLI (OUT-187/223) · Flesch (ONPAGE-149).


## 4 Doppelte und widersprüchliche Einträge

### 4.1 Widersprüche (müssen aufgelöst werden)

| # | Thema | Einträge | Widerspruch | Empfehlung |
|---|---|---|---|---|
| K-1 | Embeddings / semantische Ähnlichkeit | ONPAGE-159 … 164 (alle P2) vs. INTEG-180 … 189 (180–183 P1) | Doppelt spezifiziert mit abweichender Priorität **und** Semantik: ONPAGE-160 zählt Paare mit Ähnlichkeit **≥** Schwelle, INTEG-181 verlangt **strikt >** (Paar mit exakt 0,95 zählt nicht). ONPAGE-162 definiert Regeln als Quell-/Zielmuster-Paare, INTEG-184 als Einschluss-/Ausschlussregeln mit optionalem „nur innerhalb gleicher Regel“. ONPAGE-159 definiert Config-Schlüssel (`embeddings.*`), INTEG nicht. | Eine Quelle festlegen (Vorschlag: INTEG-180 ff. für Anbieter/Konfiguration, ONPAGE-160/161 nur für Filter/Spalten mit Verweis); Vergleichsoperator und Regelmodell vereinheitlichen; Priorität einheitlich (P1 oder P2). |
| K-2 | Auto Compare (Scheduling/CLI) | CRAWL-036 (P2), OUT-152 (P1), TECH-308 (P1) | Dreifach spezifiziert. Priorität P2 vs. P1. CLI-Schalter: CRAWL-036/OUT-202 `--project-crawl-comparison`, TECH-308 führt zusätzlich `--compare-with-previous` ein. TECH-308 fordert „Datenbank-Speichermodus“ als Voraussetzung – widerspricht der Konvention in 01 („keine Funktion ist modusabhängig“). | OUT-152 als einzige Vollspezifikation; CRAWL-036 und TECH-308 auf Verweis kürzen; Zusatzschalter streichen; Modusbedingung entfernen; P1. |
| K-3 | Change Detection – Standardauswahl der Elemente | CRAWL-033, OUT-144, TECH-300 | OUT-144: Standard **nur** Indexability, Page Titles, Meta Description, H1 an. TECH-300: Standard **alle außer Content** an. CRAWL-033 nennt eine kürzere Elementliste („Interne Links (eindeutige Inlinks)“, „Strukturierte Daten (Typen/Fehler)“) als die 14 Elemente in OUT-144/TECH-300. | Elementliste und Default in genau einem Eintrag (OUT-144) festlegen, TECH-300 und CRAWL-033 darauf verweisen. |
| K-4 | Vergleichsmodus: untere Inlinks/Outlinks-Ansicht | CRAWL-144 (P1) vs. OUT-149 (P2) | Priorität P1 vs. P2; Filterwerte „Alle/Hinzugefügt/Entfernt/Unverändert“ (CRAWL-144) vs. „Added/Removed“ (OUT-149). | Eine Fassung; P1 übernehmen, Filterwerte angleichen. |
| K-5 | Geplante Crawls | CRAWL-136 (P2) vs. OUT-160 … 171 (überwiegend P1) | Gleiche Funktion, P2 vs. P1. CRAWL-136 verweist zwar auf OUT, trägt aber selbst P2. | CRAWL-136 auf P1 setzen oder als reinen Verweis ohne eigene Priorität führen. |
| K-6 | Kommandozeile | CRAWL-135 (P1) vs. OUT-180 … 195 (P0) | Kernflags sind in OUT P0, der Sammeleintrag in CRAWL P1. | CRAWL-135 auf P0 (Mindestumfang) setzen. |
| K-7 | APIs-Modus | CRAWL-037 (P2) vs. INTEG-012 (P1) | Priorität. | P1 (INTEG-012) übernehmen; CRAWL-037 als Verweis. |
| K-8 | Oberflächensprache | CRAWL-137 (P2), UI-121 (P0 de/en, P1 es/fr/it), OUT-271 (P1) | Drei Prioritäten für dieselbe Funktion; CRAWL-137 fordert Sprachwechsel ohne Neustart, UI-121 ist maßgeblich für i18n. | UI-121 als Quelle; CRAWL-137 und OUT-271 auf Verweis kürzen. |
| K-9 | Pixelbreiten-Berechnung | ONPAGE-004 (P1) vs. UI-038 (P0) | Priorität; ONPAGE-004 nur Liberation Sans Regular, UI-038 Regular **und** Bold plus CJK/Emoji-Fallback (1 em). Beide definieren die Formel. | UI-038 als Engine-Spec (P0); ONPAGE-004 nur Spaltenanbindung + Testvektoren. |
| K-10 | Issue-Klassifizierung (Typ/Priorität) | ONPAGE-006 (P1) vs. UI-063 (P0) vs. 03 Anhang B | Methodik widerspricht sich: ONPAGE-006 übernimmt die Zuordnung „als Abbild der Referenz-Issue-Bibliothek“; UI-063 und 03/Anhang B legen eine **eigene** Entscheidungslogik fest, Abweichungen zulässig. Priorität P1 vs. P0. | Eine Methodik (Vorschlag: UI-063-Regeln, Referenz nur als Paritätsabgleich); ONPAGE-006-Tabelle als Erwartungswerte für den Paritätstest kennzeichnen. |
| K-11 | Live-Aktualisierung | ONPAGE-193 (P1, ≤ 10 Hz), UI-018 (P0, Standard 500 ms, 250–5000 ms), PERF-004 (≤ 10 Updates/s) | Priorität und Taktangabe: 10 Hz = 100 ms vs. 500 ms Standard. | UI-018 maßgeblich; ONPAGE-193 auf Verweis; PERF-004 als Obergrenze formulieren. |
| K-12 | Respect Self Referencing Meta Refresh | CRAWL-094 (P2) vs. TECH-068 (P1) | Priorität. | P1 (TECH-068); CRAWL-094 Verweis. |
| K-13 | JavaScript-Fehler / Konsolen-Log | CRAWL-084 (P2), TECH-114 (P1), UI-040 (P1) | Crawl-Schalter P2, Filter und Detail-Tab P1 – Filter ohne Schalter nicht erfüllbar. | CRAWL-084 auf P1. |
| K-14 | Mobile-Tab | TECH-230 … 237 vs. INTEG-140/146/147 | 04 selbst vermerkt: TECH-231 beschreibt Pass/Fail und Spalte `Font Size`, INTEG-140 Scores 0–100 und `Font Display`. TECH-236 (P1) vs. INTEG-146 (P2) für „Unsupported Plugins“; Mobile Alternate Link: CRAWL-051 (P2) vs. TECH-237/INTEG-147 (P1). | TECH-231 an INTEG-140 angleichen; eine Priorität je Filter; CRAWL-051 auf P1. |
| K-15 | Untere Detail-Tabs Spelling & Grammar / Accessibility | ONPAGE-157 (P1) vs. UI-047 (P2); TECH-259 (P1) vs. UI-048 (P2) | Fachspec P1, UI-Spec P2 – Detailansicht ist Bestandteil der Fachfunktion. | UI-047/048 auf P1. |
| K-16 | Semantische Suche – Mindestscore | INTEG-185 (Standard 0,0) vs. UI-072 (Standard 0,5 ⚖) | Abweichender Default. | Einen Wert festlegen (Referenz: konfigurierbar seit 22.2, Default unbekannt → Q-Punkt). |
| K-17 | Cookies | CRAWL-062 (P1), OUT-049/111 (P1) vs. UI-043 (P2) | Detail-Tab P2, obwohl Erfassung und Exporte P1. | UI-043 auf P1. |

### 4.2 Mehrfachspezifikationen ohne Widerspruch (zur Kenntnis; eine Quelle festlegen, Rest auf Verweis kürzen)

- Export der aktuellen Ansicht: ONPAGE-191, UI-015, OUT-020 (alle P0).
- Konfigurations-Diff: CRAWL-133, UI-116, OUT-224 (alle P2).
- Crawl-Verwaltung/Aufbewahrung: CRAWL-128, CRAWL-143, UI-124, OUT-228, OUT-229.
- Proxy: CRAWL-122, OUT-269 (P1), INTEG-016 (P2, nur API-Verbindungen).
- Segmente: CRAWL-134, UI-020, UI-067, UI-068, OUT-011, OUT-130.
- Vergleichsmodus allgemein: CRAWL-032/034/035 vs. OUT-140 … 148 vs. TECH-301 … 307 (Change Detection dreifach: CRAWL-033, OUT-144, TECH-300 ff.; URL-Mapping dreifach: CRAWL-034, OUT-146, TECH-306).
- Speichermodus: CRAWL-124, UI-122. Speicher-/Cache-Grenzen: CRAWL-129, UI-123, PERF-001.
- Sitemap-Crawl: CRAWL-058 … 060, TECH-210.
- „Respect …“-Schalter: CRAWL-089/091/092/093 vs. TECH-009/067/024/042.
- HTML-Validierung: CRAWL-138, TECH-008. Green Hosting: CRAWL-139, TECH-292.
- Link Score: UI-081 (P0, Algorithmus), ONPAGE-017 (P1), TECH-145 (P1).
- Crawl-Analyse: ONPAGE-007, TECH-005, UI-080, CRAWL-141.
- Untere Detail-Tabs: ONPAGE-145/172/194, TECH-089/189/259, INTEG-126/144/183 vs. UI-031 … 050.
- All Error Inlinks / Redirects to Error: ONPAGE-067, OUT-067, OUT-084.
- Tab-Bulk-Exporte: ONPAGE-165/181, TECH-025/088/190/219, INTEG-034, OUT-050 … 066.
- Aggregierte Ankertexte: TECH-146, UI-034. Crawl-Pfad: UI-050, OUT-023. Inhaltsbereich: ONPAGE-140, INTEG-027.
- Usage Stats: UI-128, OUT-245. MCP: UI-129, OUT-285. Update-Prüfung: CRAWL-142, OUT-250.

### 4.3 Prioritätshinweise (keine Widersprüche, aber Referenz-Gewicht beachten)

- MCP-Server (OUT-285, UI-129) ist P2, obwohl Headline-Feature von 24.0. Für Parität mit 24.3 mindestens P1 erwägen.
- Export-Anhänge per E-Mail (OUT-283) P2, Referenz-Feature 24.0; Desktop-Benachrichtigung (OUT-284) P2.
- Linux (OUT-264) P2 als dokumentierte, bewusste Paritätslücke – in Ordnung, solange die CLI auf Linux baut (AK vorhanden).
- Crawl Invalid Links (CRAWL-054) P2, Referenz 24.0 – vertretbar.

## 5 Tabelle aller Funktions-IDs

Status aller Einträge: **offen**. Spalte „Priorität“ gibt den Spec-Wert wieder; bei Mischangaben (z. B. „P0 (…) / P1 (…)“) zählt in Abschnitt 2 die zuerst genannte Priorität.

Mischangaben: CRAWL-024 („P1 (Spalte `HTTP-Version`) / P2 (Auswahlschalter)“), CRAWL-077 („P0 (Textmodus) / P1 (Umschalter)“), CRAWL-123 („P1 (wegen Testbarkeit aller Fixtures; im UI P2)“), CRAWL-140 („P0 (Seite und Mechanik) / P1 (Größengrenzwerte)“), CRAWL-144 („P1 (Inlinks/Outlinks, Issues) / P2 (gerenderte Seite)“), ONPAGE-012 („P0 (Total Transferred: P1, nur mit Rendering)“), ONPAGE-142 („P0 (Spalten der P1/P2-Funktionen erscheinen mit diesen)“), UI-004 („P0 (Anzeige, Wechseln) / P1 (Umsortieren, Ausblenden, Konfigurieren)“), UI-008 („P0 (ein/aus) / P1 (Drag & Drop, Persistenz)“), UI-011 („P0 (Klartext auf Address) / P1 (Regex, Spaltenwahl, Regeln, Groß/Klein)“), UI-038 („P0 (wird auch von den Filtern „Over/Below X Pixels“ genutzt)“), UI-064 („P0 (Kategorien Response Codes bis Links) / P1 (übrige)“), UI-080 („P0 (Mechanik + Link Score) / P1 (alle Teilschritte)“), UI-110 („P0 (Kopieren, Im Browser öffnen, Erneut crawlen, Entfernen, Export) / P1 (Rest)“), UI-113 („P0 (Grundkürzel) / P1 (Rest)“), UI-121 („P0 (de, en) / P1 (es, fr, it) / P2 (weitere per Community)“), OUT-021 („P0 (Inlinks, Outlinks), P1 (übrige)“).

| ID | Titel | Priorität | Bereich | Status |
|---|---|---|---|---|
| CRAWL-001 | Crawl starten und Start-URL prüfen | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-002 | URL-Parsing und Auflösung relativer Links | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-003 | URL-Normalisierung und Deduplizierung | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-004 | Fragment-Bezeichner | P1 | 01 · A. Crawl-Kern | offen |
| CRAWL-005 | Crawl-Reihenfolge (Frontier) und Crawl-Tiefe | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-006 | Klassifikation intern/extern | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-007 | Crawl-Bereich: Subdomain (Standard) | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-008 | Crawl-Bereich: Alle Subdomains | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-009 | Crawl-Bereich: Unterordner | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-010 | Links außerhalb des Startordners prüfen | P1 | 01 · A. Crawl-Kern | offen |
| CRAWL-011 | HTTP-Abruf (Methode, Kompression, Header) | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-012 | Statuscode-Erfassung und Klassen | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-013 | Klassifikation „Keine Antwort“ | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-014 | HTTP-Weiterleitungen (3xx) | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-015 | Meta-Refresh- und HTTP-Refresh-Weiterleitungen | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-016 | JavaScript-Weiterleitungen | P1 | 01 · A. Crawl-Kern | offen |
| CRAWL-017 | Weiterleitungsketten und -schleifen | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-018 | HSTS-Richtlinie respektieren | P1 | 01 · A. Crawl-Kern | offen |
| CRAWL-019 | Inhaltstyp-Erkennung | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-020 | „Seiten als HTML annehmen“ | P1 | 01 · A. Crawl-Kern | offen |
| CRAWL-021 | Zeichensatz-Erkennung | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-022 | Antwortzeit, Größe, Last-Modified | P0 | 01 · A. Crawl-Kern | offen |
| CRAWL-023 | TLS und Zertifikate | P1 | 01 · A. Crawl-Kern | offen |
| CRAWL-024 | HTTP-Versionen (HTTP/1.1, HTTP/2) | P1 (Spalte `HTTP-Version`) / P2 (Auswahlschalter) | 01 · A. Crawl-Kern | offen |
| CRAWL-025 | IPv6 | P2 | 01 · A. Crawl-Kern | offen |
| CRAWL-026 | Spider-Modus | P0 | 01 · B. Crawl-Modi | offen |
| CRAWL-027 | Listen-Modus: Eingabequellen | P0 | 01 · B. Crawl-Modi | offen |
| CRAWL-028 | Listen-Modus: Verhalten | P0 | 01 · B. Crawl-Modi | offen |
| CRAWL-029 | Listen-Modus: Export in Upload-Reihenfolge | P1 | 01 · B. Crawl-Modi | offen |
| CRAWL-030 | Weiterleitungen/Canonicals immer bis zum Ziel folgen | P1 | 01 · B. Crawl-Modi | offen |
| CRAWL-031 | SERP-Modus | P1 | 01 · B. Crawl-Modi | offen |
| CRAWL-032 | Vergleichsmodus: Auswahl und Kategorien | P1 | 01 · B. Crawl-Modi | offen |
| CRAWL-033 | Vergleichsmodus: Änderungserkennung | P1 | 01 · B. Crawl-Modi | offen |
| CRAWL-034 | Vergleichsmodus: URL-Zuordnung | P1 | 01 · B. Crawl-Modi | offen |
| CRAWL-035 | Vergleichsmodus: Seitenstruktur-Ansicht | P2 | 01 · B. Crawl-Modi | offen |
| CRAWL-036 | Vergleich automatisch (geplante/CLI-Crawls) | P2 | 01 · B. Crawl-Modi | offen |
| CRAWL-037 | API-Modus (Daten ohne Crawl) | P2 | 01 · B. Crawl-Modi | offen |
| CRAWL-038 | Grundprinzip „Speichern“ vs. „Crawlen“ | P0 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-039 | Bilder | P0 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-040 | CSS | P0 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-041 | JavaScript-Dateien | P0 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-042 | Medien (Audio/Video) | P1 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-043 | SWF | P2 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-044 | Interne Hyperlinks | P0 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-045 | Externe Links | P0 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-046 | Canonicals | P0 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-047 | Paginierung (rel next/prev) | P1 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-048 | Hreflang | P1 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-049 | AMP | P2 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-050 | Meta Refresh und iframes | P1 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-051 | Mobile-Alternate-Links | P2 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-052 | Link-Quellen im HTML (welche Elemente Links erzeugen) | P0 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-053 | Nicht crawlbare Links speichern | P1 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-054 | Ungültige Links crawlen | P2 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-055 | Bilder aus srcset extrahieren | P1 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-056 | Nofollow-Links folgen (intern/extern) | P0 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-057 | Links in PDF-Dokumenten | P1 | 01 · C. Spider-Konfiguration: Ressourcen- und Seitenlinks | offen |
| CRAWL-058 | Sitemap-Erkennung über robots.txt | P1 | 01 · D. XML-Sitemaps | offen |
| CRAWL-059 | Verlinkte und vorgegebene Sitemaps crawlen | P1 | 01 · D. XML-Sitemaps | offen |
| CRAWL-060 | Sitemap-Parser | P1 | 01 · D. XML-Sitemaps | offen |
| CRAWL-061 | Schalter für Seitendetails | P1 | 01 · E. Extraktion | offen |
| CRAWL-062 | URL-Details: Antwortzeit, Last-Modified, HTTP-Header, Cookies | P1 | 01 · E. Extraktion | offen |
| CRAWL-063 | HTML und gerendertes HTML speichern | P1 | 01 · E. Extraktion | offen |
| CRAWL-064 | PDF speichern und Eigenschaften extrahieren | P2 | 01 · E. Extraktion | offen |
| CRAWL-065 | Direktiven- und Strukturdaten-Schalter | P1 | 01 · E. Extraktion | offen |
| CRAWL-066 | Maximale Gesamtzahl URLs | P0 | 01 · F. Limits | offen |
| CRAWL-067 | Maximale Crawl-Tiefe | P0 | 01 · F. Limits | offen |
| CRAWL-068 | URLs pro Crawl-Tiefe | P2 | 01 · F. Limits | offen |
| CRAWL-069 | Maximale Ordnertiefe | P1 | 01 · F. Limits | offen |
| CRAWL-070 | Maximale Anzahl Query-Parameter | P1 | 01 · F. Limits | offen |
| CRAWL-071 | Maximale URLs pro Subdomain | P2 | 01 · F. Limits | offen |
| CRAWL-072 | Maximale Anzahl verfolgter Weiterleitungen | P0 | 01 · F. Limits | offen |
| CRAWL-073 | Maximale URL-Länge | P1 | 01 · F. Limits | offen |
| CRAWL-074 | Maximale Links pro URL | P1 | 01 · F. Limits | offen |
| CRAWL-075 | Maximale Seitengröße | P1 | 01 · F. Limits | offen |
| CRAWL-076 | Limit nach URL-Pfad | P2 | 01 · F. Limits | offen |
| CRAWL-077 | Rendering-Modus wählen | P0 (Textmodus) / P1 (Umschalter) | 01 · G. Rendering | offen |
| CRAWL-078 | JavaScript-Rendering mit Headless-Chromium | P1 | 01 · G. Rendering | offen |
| CRAWL-079 | Link-Herkunft (Roh-HTML vs. gerendertes HTML) | P1 | 01 · G. Rendering | offen |
| CRAWL-080 | AJAX-Timeout | P1 | 01 · G. Rendering | offen |
| CRAWL-081 | Viewport und Geräte-Voreinstellungen | P1 | 01 · G. Rendering | offen |
| CRAWL-082 | Gerenderte Screenshots | P1 | 01 · G. Rendering | offen |
| CRAWL-083 | Blockierte Ressourcen beim Rendern | P1 | 01 · G. Rendering | offen |
| CRAWL-084 | JavaScript-Fehler / Konsolen-Log | P2 | 01 · G. Rendering | offen |
| CRAWL-085 | Shadow DOM und iframes abflachen | P2 | 01 · G. Rendering | offen |
| CRAWL-086 | Website-Archiv (Ordner / WARC) | P2 | 01 · G. Rendering | offen |
| CRAWL-087 | Altes AJAX-Crawling-Schema | P2 | 01 · G. Rendering | offen |
| CRAWL-088 | Cookie-Verhalten | P0 | 01 · H. Erweitert | offen |
| CRAWL-089 | Nicht indexierbare URLs bei Problemen ignorieren | P1 | 01 · H. Erweitert | offen |
| CRAWL-090 | Paginierte URLs bei Duplikat-Filtern ignorieren | P2 | 01 · H. Erweitert | offen |
| CRAWL-091 | noindex respektieren | P1 | 01 · H. Erweitert | offen |
| CRAWL-092 | Canonical respektieren | P1 | 01 · H. Erweitert | offen |
| CRAWL-093 | next/prev respektieren | P2 | 01 · H. Erweitert | offen |
| CRAWL-094 | Selbstreferenzierenden Meta-Refresh respektieren | P2 | 01 · H. Erweitert | offen |
| CRAWL-095 | Antwort-Timeout | P0 | 01 · H. Erweitert | offen |
| CRAWL-096 | Wiederholung bei 5xx-Antworten | P1 | 01 · H. Erweitert | offen |
| CRAWL-097 | Wiederholung bei Netzwerkfehlern und 429 | P1 | 01 · H. Erweitert | offen |
| CRAWL-098 | robots.txt abrufen und auswerten | P0 | 01 · I. robots.txt | offen |
| CRAWL-099 | robots.txt-Modi | P0 | 01 · I. robots.txt | offen |
| CRAWL-100 | Fehlerfälle beim robots.txt-Abruf | P0 | 01 · I. robots.txt | offen |
| CRAWL-101 | Anzeige blockierter URLs (intern/extern) | P1 | 01 · I. robots.txt | offen |
| CRAWL-102 | Eigene robots.txt pro Subdomain und Tester | P1 | 01 · I. robots.txt | offen |
| CRAWL-103 | Parameter entfernen | P1 | 01 · J. URL-Umschreibung | offen |
| CRAWL-104 | Regex-Ersetzung | P1 | 01 · J. URL-Umschreibung | offen |
| CRAWL-105 | Gefundene URLs kleinschreiben | P2 | 01 · J. URL-Umschreibung | offen |
| CRAWL-106 | Prozent-Kodierungsmodus | P2 | 01 · J. URL-Umschreibung | offen |
| CRAWL-107 | Testfeld für Umschreibungen | P1 | 01 · J. URL-Umschreibung | offen |
| CRAWL-108 | CDNs als intern behandeln | P1 | 01 · K. Bereichssteuerung | offen |
| CRAWL-109 | Include-Regeln (Regex) | P0 | 01 · K. Bereichssteuerung | offen |
| CRAWL-110 | Exclude-Regeln (Regex) | P0 | 01 · K. Bereichssteuerung | offen |
| CRAWL-111 | Regex-Tester für Include/Exclude | P1 | 01 · K. Bereichssteuerung | offen |
| CRAWL-112 | Parallele Threads | P0 | 01 · L. Geschwindigkeit, User-Agent, Header | offen |
| CRAWL-113 | Maximale URLs pro Sekunde | P0 | 01 · L. Geschwindigkeit, User-Agent, Header | offen |
| CRAWL-114 | User-Agent und Voreinstellungen | P0 | 01 · L. Geschwindigkeit, User-Agent, Header | offen |
| CRAWL-115 | Eigene HTTP-Request-Header | P1 | 01 · L. Geschwindigkeit, User-Agent, Header | offen |
| CRAWL-116 | Accept-Language | P1 | 01 · L. Geschwindigkeit, User-Agent, Header | offen |
| CRAWL-117 | Standard-Authentifizierung: Basic | P0 | 01 · M. Authentifizierung und Proxy | offen |
| CRAWL-118 | Standard-Authentifizierung: Digest | P1 | 01 · M. Authentifizierung und Proxy | offen |
| CRAWL-119 | NTLM und Negotiate/Kerberos | P2 | 01 · M. Authentifizierung und Proxy | offen |
| CRAWL-120 | Formularbasierte Anmeldung (eingebetteter Browser) | P1 | 01 · M. Authentifizierung und Proxy | offen |
| CRAWL-121 | Authentifizierungsprofile (verschlüsselt) | P2 | 01 · M. Authentifizierung und Proxy | offen |
| CRAWL-122 | Proxy | P1 | 01 · M. Authentifizierung und Proxy | offen |
| CRAWL-123 | Host-Auflösung überschreiben | P1 (wegen Testbarkeit aller Fixtures; im UI P2) | 01 · M. Authentifizierung und Proxy | offen |
| CRAWL-124 | Speichermodus: Datenbank vs. Arbeitsspeicher | P0 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-125 | Automatisches Speichern und Wiederherstellung | P0 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-126 | Crawl pausieren und fortsetzen | P0 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-127 | Crawl speichern, öffnen, importieren, exportieren | P0 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-128 | Crawl-Verwaltung (Projekte, Sperren, Aufbewahrung) | P1 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-129 | Speicherwarnung und Ressourcenschutz | P1 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-130 | Fortschritt und Crawl-Statistik | P0 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-131 | Einzelne URLs neu crawlen, entfernen, hinzufügen | P1 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-132 | Konfigurationsprofile | P0 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-133 | Konfigurations-Diff zum Standard | P2 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-134 | Segmente | P1 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-135 | Kommandozeile / Headless-Betrieb | P1 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-136 | Geplante Crawls | P2 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-137 | Oberflächensprache | P2 | 01 · N. Speicher, Persistenz, Steuerung | offen |
| CRAWL-138 | HTML-Validierung durchführen | P1 | 01 · O. Ergänzungen | offen |
| CRAWL-139 | Green Hosting für die CO2-Berechnung | P2 | 01 · O. Ergänzungen | offen |
| CRAWL-140 | Voreinstellungen (crawl-relevante Grenzwerte) | P0 (Seite und Mechanik) / P1 (Größengrenzwerte) | 01 · O. Ergänzungen | offen |
| CRAWL-141 | Automatische Crawl-Analyse am Crawl-Ende | P0 | 01 · O. Ergänzungen | offen |
| CRAWL-142 | Update-Prüfung und Fokus-Modus (Abgrenzung) | P2 | 01 · O. Ergänzungen | offen |
| CRAWL-143 | Kontextmenü der Crawl-Verwaltung | P1 | 01 · O. Ergänzungen | offen |
| CRAWL-144 | Vergleichsmodus: Detailansichten (Links, Issues, gerenderte Seite) | P1 (Inlinks/Outlinks, Issues) / P2 (gerenderte Seite) | 01 · O. Ergänzungen | offen |
| ONPAGE-001 | Tab-Gerüst: Spalten, Filter, Zähler | P0 | 02 · 1 Gemeinsame Mechanik | offen |
| ONPAGE-002 | Präferenzen für Schwellwerte | P0 | 02 · 1 Gemeinsame Mechanik | offen |
| ONPAGE-003 | Textnormalisierung und Zeichenlänge | P0 | 02 · 1 Gemeinsame Mechanik | offen |
| ONPAGE-004 | Pixelbreiten-Berechnung | P1 | 02 · 1 Gemeinsame Mechanik | offen |
| ONPAGE-005 | Duplikat-Gruppierung (Wertgleichheit über Seiten) | P0 | 02 · 1 Gemeinsame Mechanik | offen |
| ONPAGE-006 | Issue-Klassifizierung (Typ/Priorität) | P1 | 02 · 1 Gemeinsame Mechanik | offen |
| ONPAGE-007 | Abhängigkeit „Crawl-Analyse erforderlich“ | P0 | 02 · 1 Gemeinsame Mechanik | offen |
| ONPAGE-008 | Mehrfachvorkommen und Spalten „…1/…2“ | P0 | 02 · 1 Gemeinsame Mechanik | offen |
| ONPAGE-009 | Grundmenge der Element-Tabs | P0 | 02 · 1 Gemeinsame Mechanik | offen |
| ONPAGE-010 | Internal: Basisspalten | P0 | 02 · 2 Internal | offen |
| ONPAGE-011 | Internal: Onpage-Element-Spalten | P0 | 02 · 2 Internal | offen |
| ONPAGE-012 | Internal: Size, Transferred, Total Transferred | P0 (Total Transferred: P1, nur mit Rendering) | 02 · 2 Internal | offen |
| ONPAGE-013 | Internal: Word Count, Text Ratio | P0 | 02 · 2 Internal | offen |
| ONPAGE-014 | Internal: Crawl Depth, Folder Depth | P0 | 02 · 2 Internal | offen |
| ONPAGE-015 | Internal: Link-Zählspalten | P0 | 02 · 2 Internal | offen |
| ONPAGE-016 | Internal: JS-Link-Zählspalten | P1 | 02 · 2 Internal | offen |
| ONPAGE-017 | Internal: Link Score (Spaltenanbindung) | P1 | 02 · 2 Internal | offen |
| ONPAGE-018 | Internal: Technische Spalten | P0 | 02 · 2 Internal | offen |
| ONPAGE-019 | Internal: Content-Analyse-Spalten | P1 | 02 · 2 Internal | offen |
| ONPAGE-020 | Inhaltstyp-Klassifizierung | P0 | 02 · 2 Internal | offen |
| ONPAGE-021 | Internal-Filter HTML | P0 | 02 · 2 Internal | offen |
| ONPAGE-022 | Internal-Filter JavaScript, CSS, Images, PDF | P0 | 02 · 2 Internal | offen |
| ONPAGE-023 | Internal-Filter Flash, Other (+ Erweiterung Media, Fonts, XML) | P1 | 02 · 2 Internal | offen |
| ONPAGE-024 | Internal-Filter Unknown | P0 | 02 · 2 Internal | offen |
| ONPAGE-025 | External: Definition und Spalten | P0 | 02 · 3 External | offen |
| ONPAGE-026 | External: Inhaltstyp-Filter | P0 | 02 · 3 External | offen |
| ONPAGE-030 | Security: Spalten | P1 | 02 · 4 Security | offen |
| ONPAGE-031 | Security-Filter HTTP URLs | P0 | 02 · 4 Security | offen |
| ONPAGE-032 | Security-Filter HTTPS URLs | P0 | 02 · 4 Security | offen |
| ONPAGE-033 | Security-Filter Mixed Content | P1 | 02 · 4 Security | offen |
| ONPAGE-034 | Security-Filter Form URL Insecure | P1 | 02 · 4 Security | offen |
| ONPAGE-035 | Security-Filter Form on HTTP URL | P1 | 02 · 4 Security | offen |
| ONPAGE-036 | Security-Filter Unsafe Cross-Origin Links | P1 | 02 · 4 Security | offen |
| ONPAGE-037 | Security-Filter Protocol-Relative Resource Links | P1 | 02 · 4 Security | offen |
| ONPAGE-038 | Security-Filter Missing HSTS Header | P1 | 02 · 4 Security | offen |
| ONPAGE-039 | Security-Filter Missing Content-Security-Policy Header | P1 | 02 · 4 Security | offen |
| ONPAGE-040 | Security-Filter Missing X-Content-Type-Options Header | P1 | 02 · 4 Security | offen |
| ONPAGE-041 | Security-Filter Missing X-Frame-Options Header | P1 | 02 · 4 Security | offen |
| ONPAGE-042 | Security-Filter Missing Secure Referrer-Policy Header | P1 | 02 · 4 Security | offen |
| ONPAGE-043 | Security-Filter Bad Content Type | P1 | 02 · 4 Security | offen |
| ONPAGE-050 | Response Codes: Spalten | P0 | 02 · 5 Response Codes | offen |
| ONPAGE-051 | Response Codes: Aufteilung Internal/External und „All“ | P0 | 02 · 5 Response Codes | offen |
| ONPAGE-052 | Filter Blocked by Robots.txt | P0 | 02 · 5 Response Codes | offen |
| ONPAGE-053 | Filter Blocked Resource | P1 | 02 · 5 Response Codes | offen |
| ONPAGE-054 | Filter No Response | P0 | 02 · 5 Response Codes | offen |
| ONPAGE-055 | Filter Success (2XX) | P0 | 02 · 5 Response Codes | offen |
| ONPAGE-056 | Filter Redirection (3XX) | P0 | 02 · 5 Response Codes | offen |
| ONPAGE-057 | Filter Redirection (JavaScript) | P1 | 02 · 5 Response Codes | offen |
| ONPAGE-058 | Filter Redirection (Meta Refresh) | P0 | 02 · 5 Response Codes | offen |
| ONPAGE-059 | Filter Redirection (HTTP Refresh) | P1 | 02 · 5 Response Codes | offen |
| ONPAGE-060 | Filter Redirect Chain | P1 | 02 · 5 Response Codes | offen |
| ONPAGE-061 | Filter Redirect Loop | P1 | 02 · 5 Response Codes | offen |
| ONPAGE-062 | Filter Client Error (4XX) | P0 | 02 · 5 Response Codes | offen |
| ONPAGE-063 | Filter Server Error (5XX) | P0 | 02 · 5 Response Codes | offen |
| ONPAGE-064 | Redirect Type „HSTS Policy“ | P1 | 02 · 5 Response Codes | offen |
| ONPAGE-065 | Status-Texte für Fehlerfälle ohne Antwort | P0 | 02 · 5 Response Codes | offen |
| ONPAGE-066 | Statuscodes außerhalb 2xx–5xx | P2 | 02 · 5 Response Codes | offen |
| ONPAGE-067 | Bulk-Export „All Error Inlinks“ und Report „Redirects to Error“ | P1 | 02 · 5 Response Codes | offen |
| ONPAGE-070 | URL: Spalten | P0 | 02 · 6 URL | offen |
| ONPAGE-071 | URL-Filter Non ASCII Characters | P0 | 02 · 6 URL | offen |
| ONPAGE-072 | URL-Filter Underscores | P0 | 02 · 6 URL | offen |
| ONPAGE-073 | URL-Filter Uppercase | P0 | 02 · 6 URL | offen |
| ONPAGE-074 | URL-Filter Multiple Slashes | P0 | 02 · 6 URL | offen |
| ONPAGE-075 | URL-Filter Repetitive Path | P1 | 02 · 6 URL | offen |
| ONPAGE-076 | URL-Filter Contains A Space | P0 | 02 · 6 URL | offen |
| ONPAGE-077 | URL-Filter Internal Search | P1 | 02 · 6 URL | offen |
| ONPAGE-078 | URL-Filter Parameters | P0 | 02 · 6 URL | offen |
| ONPAGE-079 | URL-Filter Broken Bookmark | P1 | 02 · 6 URL | offen |
| ONPAGE-080 | URL-Filter GA Tracking Parameters | P1 | 02 · 6 URL | offen |
| ONPAGE-081 | URL-Filter Over 115 Characters | P0 | 02 · 6 URL | offen |
| ONPAGE-090 | Page Titles: Extraktion und Spalten | P0 | 02 · 7 Page Titles | offen |
| ONPAGE-091 | Titel-Filter Missing | P0 | 02 · 7 Page Titles | offen |
| ONPAGE-092 | Titel-Filter Duplicate | P0 | 02 · 7 Page Titles | offen |
| ONPAGE-093 | Titel-Filter Over 60 Characters | P0 | 02 · 7 Page Titles | offen |
| ONPAGE-094 | Titel-Filter Below 30 Characters | P0 | 02 · 7 Page Titles | offen |
| ONPAGE-095 | Titel-Filter Over 561 Pixels | P1 | 02 · 7 Page Titles | offen |
| ONPAGE-096 | Titel-Filter Below 200 Pixels | P1 | 02 · 7 Page Titles | offen |
| ONPAGE-097 | Titel-Filter Same as H1 | P1 | 02 · 7 Page Titles | offen |
| ONPAGE-098 | Titel-Filter Multiple | P0 | 02 · 7 Page Titles | offen |
| ONPAGE-099 | Titel-Filter Outside <head> | P1 | 02 · 7 Page Titles | offen |
| ONPAGE-100 | Meta Description: Extraktion und Spalten | P0 | 02 · 8 Meta Description | offen |
| ONPAGE-101 | MD-Filter Missing | P0 | 02 · 8 Meta Description | offen |
| ONPAGE-102 | MD-Filter Duplicate | P0 | 02 · 8 Meta Description | offen |
| ONPAGE-103 | MD-Filter Over 155 Characters | P0 | 02 · 8 Meta Description | offen |
| ONPAGE-104 | MD-Filter Below 70 Characters | P0 | 02 · 8 Meta Description | offen |
| ONPAGE-105 | MD-Filter Over 985 Pixels | P1 | 02 · 8 Meta Description | offen |
| ONPAGE-106 | MD-Filter Below 400 Pixels | P1 | 02 · 8 Meta Description | offen |
| ONPAGE-107 | MD-Filter Multiple | P0 | 02 · 8 Meta Description | offen |
| ONPAGE-108 | MD-Filter Outside <head> | P1 | 02 · 8 Meta Description | offen |
| ONPAGE-110 | Meta Keywords: Extraktion und Spalten | P1 | 02 · 9 Meta Keywords | offen |
| ONPAGE-111 | MK-Filter Missing | P1 | 02 · 9 Meta Keywords | offen |
| ONPAGE-112 | MK-Filter Duplicate | P1 | 02 · 9 Meta Keywords | offen |
| ONPAGE-113 | MK-Filter Multiple | P1 | 02 · 9 Meta Keywords | offen |
| ONPAGE-120 | H1: Extraktion und Spalten | P0 | 02 · 10 H1 | offen |
| ONPAGE-121 | H1-Filter Missing | P0 | 02 · 10 H1 | offen |
| ONPAGE-122 | H1-Filter Duplicate | P0 | 02 · 10 H1 | offen |
| ONPAGE-123 | H1-Filter Over 70 Characters | P0 | 02 · 10 H1 | offen |
| ONPAGE-124 | H1-Filter Multiple | P0 | 02 · 10 H1 | offen |
| ONPAGE-125 | H1-Filter Alt Text in h1 | P1 | 02 · 10 H1 | offen |
| ONPAGE-126 | H1-Filter Non-sequential | P1 | 02 · 10 H1 | offen |
| ONPAGE-130 | H2: Extraktion und Spalten | P0 | 02 · 11 H2 | offen |
| ONPAGE-131 | H2-Filter Missing | P0 | 02 · 11 H2 | offen |
| ONPAGE-132 | H2-Filter Duplicate | P0 | 02 · 11 H2 | offen |
| ONPAGE-133 | H2-Filter Over 70 Characters | P0 | 02 · 11 H2 | offen |
| ONPAGE-134 | H2-Filter Multiple | P0 | 02 · 11 H2 | offen |
| ONPAGE-135 | H2-Filter Non-sequential | P1 | 02 · 11 H2 | offen |
| ONPAGE-140 | Inhaltsbereich (Content Area) | P0 | 02 · 12 Content | offen |
| ONPAGE-141 | Wortzählung | P0 | 02 · 12 Content | offen |
| ONPAGE-142 | Content: Spalten | P0 (Spalten der P1/P2-Funktionen erscheinen mit diesen) | 02 · 12 Content | offen |
| ONPAGE-143 | Content-Filter Exact Duplicates | P0 | 02 · 12 Content | offen |
| ONPAGE-144 | Content-Filter Near Duplicates (MinHash) | P1 | 02 · 12 Content | offen |
| ONPAGE-145 | Lower-Tab Duplicate Details | P1 | 02 · 12 Content | offen |
| ONPAGE-146 | Content-Filter Low Content Pages | P0 | 02 · 12 Content | offen |
| ONPAGE-147 | Content-Filter Soft 404 Pages | P1 | 02 · 12 Content | offen |
| ONPAGE-148 | Content-Filter Lorem Ipsum Placeholder | P1 | 02 · 12 Content | offen |
| ONPAGE-149 | Lesbarkeit: Flesch-Berechnung und Spalten | P1 | 02 · 12 Content | offen |
| ONPAGE-150 | Content-Filter Readability Difficult | P1 | 02 · 12 Content | offen |
| ONPAGE-151 | Content-Filter Readability Very Difficult | P1 | 02 · 12 Content | offen |
| ONPAGE-152 | Lesbarkeit für weitere Sprachen (Erweiterung) | P2 | 02 · 12 Content | offen |
| ONPAGE-153 | Rechtschreibprüfung: Engine und Sprachwahl | P1 | 02 · 12 Content | offen |
| ONPAGE-154 | Grammatikprüfung: Engine und Regeln | P1 | 02 · 12 Content | offen |
| ONPAGE-155 | Content-Filter Spelling Errors | P1 | 02 · 12 Content | offen |
| ONPAGE-156 | Content-Filter Grammar Errors | P1 | 02 · 12 Content | offen |
| ONPAGE-157 | Lower-Tab Spelling & Grammar Details, Ignorieren, Wörterbuch | P1 | 02 · 12 Content | offen |
| ONPAGE-158 | Neu-Prüfung ohne Recrawl und Top-100-Fehlerübersicht | P2 | 02 · 12 Content | offen |
| ONPAGE-159 | Embeddings: Anbieter und Konfiguration | P2 | 02 · 12 Content | offen |
| ONPAGE-160 | Content-Filter Semantically Similar | P2 | 02 · 12 Content | offen |
| ONPAGE-161 | Content-Filter Low Relevance Content | P2 | 02 · 12 Content | offen |
| ONPAGE-162 | Embedding-Regeln (URL-Muster) | P2 | 02 · 12 Content | offen |
| ONPAGE-163 | Lower-Tab Duplicate Details: Semantic Similarity | P2 | 02 · 12 Content | offen |
| ONPAGE-164 | Embeddings nur für indexierbare Seiten | P2 | 02 · 12 Content | offen |
| ONPAGE-165 | Bulk-Exporte Content | P1 | 02 · 12 Content | offen |
| ONPAGE-166 | PDF-Dokumente: Titel, Keywords, Wortzahl, Lesbarkeit, Rechtschreibung | P1 | 02 · 12 Content | offen |
| ONPAGE-170 | Bildquellen-Erkennung | P0 | 02 · 13 Images | offen |
| ONPAGE-171 | Images: Spalten | P0 | 02 · 13 Images | offen |
| ONPAGE-172 | Lower-Tab Image Details | P1 | 02 · 13 Images | offen |
| ONPAGE-173 | Bildgröße bestimmen | P0 | 02 · 13 Images | offen |
| ONPAGE-174 | Image-Filter Over 100 KB | P0 | 02 · 13 Images | offen |
| ONPAGE-175 | Image-Filter Missing Alt Text | P0 | 02 · 13 Images | offen |
| ONPAGE-176 | Image-Filter Missing Alt Attribute | P0 | 02 · 13 Images | offen |
| ONPAGE-177 | Image-Filter Alt Text Over 100 Characters | P0 | 02 · 13 Images | offen |
| ONPAGE-178 | Image-Filter Background Images | P1 | 02 · 13 Images | offen |
| ONPAGE-179 | Image-Filter Missing Size Attributes | P1 | 02 · 13 Images | offen |
| ONPAGE-180 | Image-Filter Incorrectly Sized Images | P1 | 02 · 13 Images | offen |
| ONPAGE-181 | Bulk-Exporte Images | P1 | 02 · 13 Images | offen |
| ONPAGE-190 | Übersichtszähler je Tab/Filter | P0 | 02 · 14 Querschnitt | offen |
| ONPAGE-191 | Export der aktuellen Filteransicht | P0 | 02 · 14 Querschnitt | offen |
| ONPAGE-192 | Verknüpfung mit Lower-Tabs | P0 | 02 · 14 Querschnitt | offen |
| ONPAGE-193 | Live-Aktualisierung während des Crawls | P1 | 02 · 14 Querschnitt | offen |
| ONPAGE-194 | Lower-Tab SERP Snippet: Geräteumschaltung und Bearbeitung | P1 | 02 · 14 Querschnitt | offen |
| TECH-001 | Indexierbarkeitsmodell | P0 | 03 · Gemeinsame Grundlagen | offen |
| TECH-002 | URL-Vergleich und Auflösung für technische Verweise | P0 | 03 · Gemeinsame Grundlagen | offen |
| TECH-003 | Head-Zugehörigkeit von Elementen („Outside \<head>“) | P0 | 03 · Gemeinsame Grundlagen | offen |
| TECH-004 | Mehrwertige Spalten „… 1, … 2, …“ | P0 | 03 · Gemeinsame Grundlagen | offen |
| TECH-005 | Live-Filter vs. Nachlauf-Analyse („Crawl-Analyse erforderlich“) | P0 | 03 · Gemeinsame Grundlagen | offen |
| TECH-006 | Rohes vs. gerendertes HTML als Datenquelle | P1 | 03 · Gemeinsame Grundlagen | offen |
| TECH-007 | Fixture-Konventionen für diesen Bereich | P0 | 03 · Gemeinsame Grundlagen | offen |
| TECH-008 | Option „Perform HTML Validation“ (Validation- und „Outside \<head>“-Filter) | P1 | 03 · Gemeinsame Grundlagen | offen |
| TECH-009 | Option „Ignore Non-Indexable URLs for Issues“ | P1 | 03 · Gemeinsame Grundlagen | offen |
| TECH-010 | Canonical-Extraktion (Link-Element und HTTP-Header) | P0 | 03 · Canonicals | offen |
| TECH-011 | Tab „Canonicals“: Population und Spalten | P0 | 03 · Canonicals | offen |
| TECH-012 | Filter „Contains Canonical“ | P0 | 03 · Canonicals | offen |
| TECH-013 | Filter „Self Referencing“ | P0 | 03 · Canonicals | offen |
| TECH-014 | Filter „Canonicalised“ | P0 | 03 · Canonicals | offen |
| TECH-015 | Filter „Missing“ | P0 | 03 · Canonicals | offen |
| TECH-016 | Filter „Multiple“ | P0 | 03 · Canonicals | offen |
| TECH-017 | Filter „Multiple Conflicting“ | P0 | 03 · Canonicals | offen |
| TECH-018 | Filter „Non-Indexable Canonical“ | P0 | 03 · Canonicals | offen |
| TECH-019 | Filter „Canonical Is Relative“ | P1 | 03 · Canonicals | offen |
| TECH-020 | Filter „Unlinked“ | P1 | 03 · Canonicals | offen |
| TECH-021 | Filter „Invalid Attribute In Annotation“ | P1 | 03 · Canonicals | offen |
| TECH-022 | Filter „Contains Fragment URL“ | P1 | 03 · Canonicals | offen |
| TECH-023 | Filter „Outside \<head>“ (Canonical) | P1 | 03 · Canonicals | offen |
| TECH-024 | Canonicals crawlen/speichern und „Respect Canonical“ | P1 | 03 · Canonicals | offen |
| TECH-025 | Report „Canonical Chains“ und Export „Non-Indexable Canonicals“ | P1 | 03 · Canonicals | offen |
| TECH-030 | Extraktion rel="next"/rel="prev" | P1 | 03 · Pagination | offen |
| TECH-031 | Tab „Pagination“: Population und Spalten | P1 | 03 · Pagination | offen |
| TECH-032 | Filter „Contains Pagination“ | P1 | 03 · Pagination | offen |
| TECH-033 | Filter „First Page“ | P1 | 03 · Pagination | offen |
| TECH-034 | Filter „Paginated 2+ Pages“ | P1 | 03 · Pagination | offen |
| TECH-035 | Filter „Pagination URL Not In Anchor Tag“ | P1 | 03 · Pagination | offen |
| TECH-036 | Filter „Non-200 Pagination URLs“ | P1 | 03 · Pagination | offen |
| TECH-037 | Filter „Unlinked Pagination URLs“ | P1 | 03 · Pagination | offen |
| TECH-038 | Filter „Non-Indexable“ (Pagination) | P1 | 03 · Pagination | offen |
| TECH-039 | Filter „Multiple Pagination URLs“ | P1 | 03 · Pagination | offen |
| TECH-040 | Filter „Pagination Loop“ | P1 | 03 · Pagination | offen |
| TECH-041 | Filter „Sequence Error“ | P1 | 03 · Pagination | offen |
| TECH-042 | Pagination crawlen/speichern, „Respect Next/Prev“ und Reports | P2 | 03 · Pagination | offen |
| TECH-050 | Directive-Extraktion (Meta Robots, X-Robots-Tag, Meta Refresh) | P0 | 03 · Directives | offen |
| TECH-051 | Tab „Directives“: Population und Spalten | P0 | 03 · Directives | offen |
| TECH-052 | Filter „Index“ | P0 | 03 · Directives | offen |
| TECH-053 | Filter „Noindex“ | P0 | 03 · Directives | offen |
| TECH-054 | Filter „Follow“ | P0 | 03 · Directives | offen |
| TECH-055 | Filter „Nofollow“ | P0 | 03 · Directives | offen |
| TECH-056 | Filter „None“ | P0 | 03 · Directives | offen |
| TECH-057 | Filter „NoArchive“ | P1 | 03 · Directives | offen |
| TECH-058 | Filter „NoSnippet“ | P1 | 03 · Directives | offen |
| TECH-059 | Filter „Max-Snippet“, „Max-Image-Preview“, „Max-Video-Preview“ | P1 | 03 · Directives | offen |
| TECH-060 | Filter „NoODP“, „NoYDIR“ (Altlasten) | P2 | 03 · Directives | offen |
| TECH-061 | Filter „NoImageIndex“ | P1 | 03 · Directives | offen |
| TECH-062 | Filter „NoTranslate“ | P1 | 03 · Directives | offen |
| TECH-063 | Filter „Unavailable_After“ | P1 | 03 · Directives | offen |
| TECH-064 | Filter „Refresh“ | P0 | 03 · Directives | offen |
| TECH-065 | Filter „Outside \<head>“ (Meta Robots) | P1 | 03 · Directives | offen |
| TECH-066 | Konfliktauflösung widersprüchlicher Direktiven | P0 | 03 · Directives | offen |
| TECH-067 | „Respect Noindex“ und „Follow Internal/External Nofollow“ | P1 | 03 · Directives | offen |
| TECH-068 | Option „Respect Self Referencing Meta Refresh“ | P1 | 03 · Directives | offen |
| TECH-070 | Hreflang-Extraktion (HTML, HTTP-Header, XML-Sitemap) | P0 | 03 · Hreflang | offen |
| TECH-071 | Tab „Hreflang“: Population und Spalten | P0 | 03 · Hreflang | offen |
| TECH-072 | Validierung von Sprach-/Regionscodes | P0 | 03 · Hreflang | offen |
| TECH-073 | Filter „Contains Hreflang“ | P0 | 03 · Hreflang | offen |
| TECH-074 | Filter „Non-200 Hreflang URLs“ | P0 | 03 · Hreflang | offen |
| TECH-075 | Filter „Unlinked Hreflang URLs“ | P1 | 03 · Hreflang | offen |
| TECH-076 | Filter „Missing Return Links“ | P0 | 03 · Hreflang | offen |
| TECH-077 | Filter „Inconsistent Language & Region Return Links“ | P1 | 03 · Hreflang | offen |
| TECH-078 | Filter „Non-Canonical Return Links“ | P1 | 03 · Hreflang | offen |
| TECH-079 | Filter „Noindex Return Links“ | P1 | 03 · Hreflang | offen |
| TECH-080 | Filter „Incorrect Language & Region Codes“ | P0 | 03 · Hreflang | offen |
| TECH-081 | Filter „Multiple Entries“ | P1 | 03 · Hreflang | offen |
| TECH-082 | Filter „Missing Self Reference“ | P1 | 03 · Hreflang | offen |
| TECH-083 | Filter „Not Using Canonical“ | P1 | 03 · Hreflang | offen |
| TECH-084 | Filter „Missing X-Default“ | P1 | 03 · Hreflang | offen |
| TECH-085 | Filter „Missing“ (Hreflang) | P0 | 03 · Hreflang | offen |
| TECH-086 | Filter „Outside \<head>“ (Hreflang) | P1 | 03 · Hreflang | offen |
| TECH-087 | Hreflang crawlen, externe Domains als intern (CDN-Liste) | P1 | 03 · Hreflang | offen |
| TECH-088 | Hreflang-Reports (Bulk-Exporte je Problemtyp) | P1 | 03 · Hreflang | offen |
| TECH-089 | Untere Detailansicht „URL Details“ mit Bestätigungsstatus je Annotation | P1 | 03 · Hreflang | offen |
| TECH-100 | Vergleich Original-HTML vs. gerendertes HTML (Grundlage) | P1 | 03 · JavaScript | offen |
| TECH-101 | Tab „JavaScript“: Population und Spalten | P1 | 03 · JavaScript | offen |
| TECH-102 | Filter „Pages With Blocked Resources“ | P1 | 03 · JavaScript | offen |
| TECH-103 | Filter „Contains JavaScript Links“ | P1 | 03 · JavaScript | offen |
| TECH-104 | Filter „Contains JavaScript Content“ | P1 | 03 · JavaScript | offen |
| TECH-105 | Filter „Noindex Only In Original HTML“ | P1 | 03 · JavaScript | offen |
| TECH-106 | Filter „Nofollow Only In Original HTML“ | P1 | 03 · JavaScript | offen |
| TECH-107 | Filter „Canonical Only In Rendered HTML“ | P1 | 03 · JavaScript | offen |
| TECH-108 | Filter „Canonical Mismatch“ | P1 | 03 · JavaScript | offen |
| TECH-109 | Filter „Page Title Only In Rendered HTML“ / „Page Title Updated By JavaScript“ | P1 | 03 · JavaScript | offen |
| TECH-110 | Filter „Meta Description Only In Rendered HTML“ / „Meta Description Updated By JavaScript“ | P1 | 03 · JavaScript | offen |
| TECH-111 | Filter „H1 Only In Rendered HTML“ / „H1 Updated By JavaScript“ | P1 | 03 · JavaScript | offen |
| TECH-112 | Filter „Uses Old AJAX Crawling Scheme URLs“ | P2 | 03 · JavaScript | offen |
| TECH-113 | Filter „Uses Old AJAX Crawling Scheme Meta Fragment Tag“ | P2 | 03 · JavaScript | offen |
| TECH-114 | Filter „Pages With JavaScript Errors“ und Konsolen-Log | P1 | 03 · JavaScript | offen |
| TECH-130 | Tab „Links“: Population und Spalten | P0 | 03 · Links | offen |
| TECH-131 | Link-Attribute: Follow, Anchor, Link-Pfad, Crawlability | P0 | 03 · Links | offen |
| TECH-132 | Filter „Pages With High Crawl Depth“ | P0 | 03 · Links | offen |
| TECH-133 | Filter „Pages Without Internal Outlinks“ | P0 | 03 · Links | offen |
| TECH-134 | Filter „Internal Nofollow Outlinks“ | P0 | 03 · Links | offen |
| TECH-135 | Filter „Internal Outlinks With No Anchor Text“ | P0 | 03 · Links | offen |
| TECH-136 | Filter „Non-Descriptive Anchor Text In Internal Outlinks“ | P1 | 03 · Links | offen |
| TECH-137 | Filter „Pages With High External Outlinks“ | P1 | 03 · Links | offen |
| TECH-138 | Filter „Pages With High Internal Outlinks“ | P1 | 03 · Links | offen |
| TECH-139 | Filter „Follow & Nofollow Internal Inlinks To Page“ | P1 | 03 · Links | offen |
| TECH-140 | Filter „Internal Nofollow Inlinks Only“ | P1 | 03 · Links | offen |
| TECH-141 | Filter „Outlinks To Localhost“ | P1 | 03 · Links | offen |
| TECH-142 | Filter „Non-Indexable Page Inlinks Only“ | P1 | 03 · Links | offen |
| TECH-143 | Filter „Pages With Uncrawlable Internal Outlinks“ | P1 | 03 · Links | offen |
| TECH-144 | Link-Präferenzen (Schwellwerte, Anchor-Liste) | P0 | 03 · Links | offen |
| TECH-145 | „% of Total“ und Link Score als Spalten | P1 | 03 · Links | offen |
| TECH-146 | Aggregierte Ankertext-Ansicht („Anchors“) in Inlinks/Outlinks | P1 | 03 · Links | offen |
| TECH-160 | AMP-Erkennung und Crawl-Optionen | P2 | 03 · AMP | offen |
| TECH-161 | Tab „AMP“: Population und Spalten | P2 | 03 · AMP | offen |
| TECH-162 | AMP-SEO-Filter (Non-200, Return Link, Canonical, Indexierbarkeit) | P2 | 03 · AMP | offen |
| TECH-163 | AMP-Validierungsfilter | P2 | 03 · AMP | offen |
| TECH-180 | Extraktion JSON-LD | P1 | 03 · Structured Data | offen |
| TECH-181 | Extraktion Microdata | P1 | 03 · Structured Data | offen |
| TECH-182 | Extraktion RDFa | P1 | 03 · Structured Data | offen |
| TECH-183 | Tab „Structured Data“: Population und Spalten | P1 | 03 · Structured Data | offen |
| TECH-184 | Filter „Contains Structured Data“ / „Missing Structured Data“ | P1 | 03 · Structured Data | offen |
| TECH-185 | Filter „JSON-LD URLs“, „Microdata URLs“, „RDFa URLs“ | P1 | 03 · Structured Data | offen |
| TECH-186 | Filter „Parse Errors“ | P1 | 03 · Structured Data | offen |
| TECH-187 | Schema.org-Validierung und Filter „Validation Errors“/„Validation Warnings“ | P1 | 03 · Structured Data | offen |
| TECH-188 | Google-Rich-Result-Validierung und Filter „Rich Result Validation Errors“/„… Warnings“ | P1 | 03 · Structured Data | offen |
| TECH-189 | Detailansicht „Structured Data Details“ | P1 | 03 · Structured Data | offen |
| TECH-190 | Reports „Validation Errors & Warnings“ (+ Summary) | P1 | 03 · Structured Data | offen |
| TECH-210 | Sitemap-Einbindung in den Crawl | P0 | 03 · Sitemaps | offen |
| TECH-211 | Tab „Sitemaps“: Population und Spalten | P0 | 03 · Sitemaps | offen |
| TECH-212 | Filter „URLs In Sitemap“ | P0 | 03 · Sitemaps | offen |
| TECH-213 | Filter „URLs Not In Sitemap“ | P0 | 03 · Sitemaps | offen |
| TECH-214 | Filter „Orphan URLs“ | P0 | 03 · Sitemaps | offen |
| TECH-215 | Filter „Non-Indexable URLs In Sitemap“ | P0 | 03 · Sitemaps | offen |
| TECH-216 | Filter „URLs In Multiple Sitemaps“ | P1 | 03 · Sitemaps | offen |
| TECH-217 | Filter „XML Sitemap With Over 50k URLs“ | P1 | 03 · Sitemaps | offen |
| TECH-218 | Filter „XML Sitemap With Over 50MB“ | P1 | 03 · Sitemaps | offen |
| TECH-219 | Sitemap-Inlinks und Bulk-Exporte | P1 | 03 · Sitemaps | offen |
| TECH-230 | Mobile-Prüfungen: Ausführungsweg (lokal/PSI) | P1 | 03 · Mobile | offen |
| TECH-231 | Tab „Mobile“: Population und Spalten | P1 | 03 · Mobile | offen |
| TECH-232 | Filter „Viewport Not Set“ | P1 | 03 · Mobile | offen |
| TECH-233 | Filter „Target Size“ | P1 | 03 · Mobile | offen |
| TECH-234 | Filter „Content Not Sized Correctly“ | P1 | 03 · Mobile | offen |
| TECH-235 | Filter „Illegible Font Size“ | P1 | 03 · Mobile | offen |
| TECH-236 | Filter „Unsupported Plugins“ | P1 | 03 · Mobile | offen |
| TECH-237 | Filter „Mobile Alternate Link“ | P1 | 03 · Mobile | offen |
| TECH-250 | Accessibility-Prüfung mit axe-core | P1 | 03 · Accessibility | offen |
| TECH-251 | Tab „Accessibility“: Population und Spalten | P1 | 03 · Accessibility | offen |
| TECH-252 | Filtergruppe „Best Practice“ (28 Regeln) | P1 | 03 · Accessibility | offen |
| TECH-253 | Filtergruppe „WCAG 2.0 A“ (56 Regeln) | P1 | 03 · Accessibility | offen |
| TECH-254 | Filtergruppe „WCAG 2.0 AA“ | P1 | 03 · Accessibility | offen |
| TECH-255 | Filtergruppe „WCAG 2.0 AAA“ | P1 | 03 · Accessibility | offen |
| TECH-256 | Filtergruppe „WCAG 2.1 AA“ | P1 | 03 · Accessibility | offen |
| TECH-257 | Filtergruppe „WCAG 2.2 AA“ | P1 | 03 · Accessibility | offen |
| TECH-258 | Accessibility-Score und Score-Filter | P2 | 03 · Accessibility | offen |
| TECH-259 | Detailansicht „Accessibility Details“ und Exporte | P1 | 03 · Accessibility | offen |
| TECH-280 | Strukturanalyse auf Token-Ebene (Grundlage) | P1 | 03 · Validation | offen |
| TECH-281 | Tab „Validation“: Population und Spalten | P1 | 03 · Validation | offen |
| TECH-282 | Filter „Invalid HTML Elements In \<head>“ | P1 | 03 · Validation | offen |
| TECH-283 | Filter „\<head> Not First In \<html> Element“ | P1 | 03 · Validation | offen |
| TECH-284 | Filter „Missing \<head> Tag“ | P1 | 03 · Validation | offen |
| TECH-285 | Filter „Multiple \<head> Tags“ | P1 | 03 · Validation | offen |
| TECH-286 | Filter „Missing \<body> Tag“ | P1 | 03 · Validation | offen |
| TECH-287 | Filter „Multiple \<body> Tags“ | P1 | 03 · Validation | offen |
| TECH-288 | Filter „\<body> Element Preceding \<html>“ | P1 | 03 · Validation | offen |
| TECH-289 | Filter „HTML Document Over 2MB“ | P1 | 03 · Validation | offen |
| TECH-290 | Filter „Resource Over 2MB“ | P1 | 03 · Validation | offen |
| TECH-291 | CO2-Schätzung und Filter „High Carbon Rating“ | P2 | 03 · Validation | offen |
| TECH-292 | Option „Green Hosting Carbon Calculation“ | P2 | 03 · Validation | offen |
| TECH-300 | Change-Detection-Konfiguration | P1 | 03 · Change Detection | offen |
| TECH-301 | Tab „Change Detection“: Population und Spalten | P1 | 03 · Change Detection | offen |
| TECH-302 | Elementfilter für Text-Elemente (Titel, Meta Description, H1) | P1 | 03 · Change Detection | offen |
| TECH-303 | Elementfilter für Zahlen (Wortzahl, Tiefe, Links, Typen) | P1 | 03 · Change Detection | offen |
| TECH-304 | Elementfilter „Indexability“ | P1 | 03 · Change Detection | offen |
| TECH-305 | Elementfilter „Content“ (MinHash-Ähnlichkeit) | P1 | 03 · Change Detection | offen |
| TECH-306 | URL-Mapping zwischen den Crawls | P1 | 03 · Change Detection | offen |
| TECH-307 | Umschalter „Changed“/„Not Changed“ und Sortierung nach Änderung | P1 | 03 · Change Detection | offen |
| TECH-308 | Automatischer Crawl-Vergleich („Auto Compare Crawls“) für geplante und CLI-Crawls | P1 | 03 · Change Detection | offen |
| INTEG-001 | API-Zugriffsdialog und Verbindungsstatus | P0 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-002 | Sichere Ablage von Zugangsdaten | P0 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-003 | Google-OAuth-2.0 (Desktop-Loopback mit PKCE) | P0 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-004 | Token-Lebenszyklus und Ablauf | P1 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-005 | Ratenbegrenzung und Wiederholungslogik | P0 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-006 | Kontingentzähler und Tageskontingente | P1 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-007 | API-Warteschlange und Fortschrittsanzeige | P0 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-008 | API-Daten nachträglich bzw. bei pausiertem Crawl anfordern | P1 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-009 | API-Daten für einzelne URLs neu anfordern | P1 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-010 | Invalidierung bei Konfigurationsänderung | P1 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-011 | Automatisch verbinden beim Start | P2 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-012 | APIs-Modus (Datenabruf ohne Crawl) | P1 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-013 | Integrationsspalten im Internal-Tab | P0 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-014 | URL-Abgleich zwischen API-Daten und Crawl | P0 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-015 | Neue URLs aus API-Quellen crawlen (Orphan-Erkennung) | P1 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-016 | Proxy und eigener Truststore für API-Verbindungen | P2 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-017 | API-Integrationen in Zeitplanung und CLI | P1 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-018 | Datenschutz und lokale Datenhaltung | P0 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-019 | Testbarkeit: überschreibbare API-Basis-URLs und Mock-Server | P0 | 04 · A Gemeinsame API-Infrastruktur | offen |
| INTEG-020 | Suchfilter definieren | P0 | 04 · B Custom Search | offen |
| INTEG-021 | Suchbereich „HTML" | P0 | 04 · B Custom Search | offen |
| INTEG-022 | Suchbereich „Page Text" | P0 | 04 · B Custom Search | offen |
| INTEG-023 | Suchbereich „Page Text No Anchors" | P1 | 04 · B Custom Search | offen |
| INTEG-024 | Suchbereich „HTML Head" | P1 | 04 · B Custom Search | offen |
| INTEG-025 | Suchbereich „HTML Body" | P1 | 04 · B Custom Search | offen |
| INTEG-026 | Suchbereich „XPath" | P1 | 04 · B Custom Search | offen |
| INTEG-027 | Suchbereich „Content Area" | P1 | 04 · B Custom Search | offen |
| INTEG-028 | Regex-Semantik | P0 | 04 · B Custom Search | offen |
| INTEG-029 | Groß-/Kleinschreibung bei Textsuche | P0 | 04 · B Custom Search | offen |
| INTEG-030 | Rohes vs. gerendertes HTML | P0 | 04 · B Custom Search | offen |
| INTEG-031 | Custom-Search-Tab und Ergebnisspalten | P0 | 04 · B Custom Search | offen |
| INTEG-032 | Bulk Add | P1 | 04 · B Custom Search | offen |
| INTEG-033 | Reihenfolge der Filter | P2 | 04 · B Custom Search | offen |
| INTEG-034 | Bulk-Export „Inlinks je Suchfilter" | P1 | 04 · B Custom Search | offen |
| INTEG-035 | Geltungsbereich der Suche | P0 | 04 · B Custom Search | offen |
| INTEG-040 | Extraktor definieren und Limits | P0 | 04 · C Custom Extraction | offen |
| INTEG-041 | XPath 1.0 | P0 | 04 · C Custom Extraction | offen |
| INTEG-042 | XPath 2.0 / 3.0 / 3.1 | P1 | 04 · C Custom Extraction | offen |
| INTEG-043 | CSS-Selektor mit optionalem Attribut | P0 | 04 · C Custom Extraction | offen |
| INTEG-044 | Regex-Extraktion | P0 | 04 · C Custom Extraction | offen |
| INTEG-045 | Ausgabeart „HTML-Element" | P0 | 04 · C Custom Extraction | offen |
| INTEG-046 | Ausgabeart „Inner HTML" | P0 | 04 · C Custom Extraction | offen |
| INTEG-047 | Ausgabeart „Text" | P0 | 04 · C Custom Extraction | offen |
| INTEG-048 | Ausgabeart „Funktionswert" | P1 | 04 · C Custom Extraction | offen |
| INTEG-049 | Mehrfachtreffer und nummerierte Spalten | P0 | 04 · C Custom Extraction | offen |
| INTEG-050 | Kombinierte Ausdrücke (Union mit `\|`) | P1 | 04 · C Custom Extraction | offen |
| INTEG-051 | Syntaxprüfung im Dialog | P0 | 04 · C Custom Extraction | offen |
| INTEG-052 | Geltungsbereich der Extraktion | P0 | 04 · C Custom Extraction | offen |
| INTEG-053 | Custom-Extraction-Tab und Export | P0 | 04 · C Custom Extraction | offen |
| INTEG-054 | Visuelle Extraktion | P2 | 04 · C Custom Extraction | offen |
| INTEG-055 | Reihenfolge der Extraktoren | P2 | 04 · C Custom Extraction | offen |
| INTEG-060 | Snippet-Verwaltung | P1 | 04 · D Custom JavaScript | offen |
| INTEG-061 | Extraktions-Snippets | P1 | 04 · D Custom JavaScript | offen |
| INTEG-062 | Aktions-Snippets | P1 | 04 · D Custom JavaScript | offen |
| INTEG-063 | Laufzeit-API: Rückgabe von Daten und Fehlern | P1 | 04 · D Custom JavaScript | offen |
| INTEG-064 | Laufzeit-API: Text in Datei speichern | P2 | 04 · D Custom JavaScript | offen |
| INTEG-065 | Laufzeit-API: URLs herunterladen | P2 | 04 · D Custom JavaScript | offen |
| INTEG-066 | Laufzeit-API: externe Skripte laden | P2 | 04 · D Custom JavaScript | offen |
| INTEG-067 | Chrome-Konsolen-Hilfsfunktionen | P2 | 04 · D Custom JavaScript | offen |
| INTEG-068 | Snippet-Tester und Debugging im externen Browser | P2 | 04 · D Custom JavaScript | offen |
| INTEG-069 | Mitgelieferte Snippet-Bibliothek | P2 | 04 · D Custom JavaScript | offen |
| INTEG-070 | Benutzer-Bibliothek, Import/Export | P2 | 04 · D Custom JavaScript | offen |
| INTEG-071 | Custom-JavaScript-Tab | P1 | 04 · D Custom JavaScript | offen |
| INTEG-072 | Sicherheitsgrenzen für Snippets | P1 | 04 · D Custom JavaScript | offen |
| INTEG-080 | Klassifikation der Link-Position | P1 | 04 · E Custom Link Positions | offen |
| INTEG-081 | Standardregeln | P1 | 04 · E Custom Link Positions | offen |
| INTEG-082 | Eigene Regeln und Reihenfolge | P1 | 04 · E Custom Link Positions | offen |
| INTEG-083 | Funktion abschaltbar | P2 | 04 · E Custom Link Positions | offen |
| INTEG-084 | Ausgabe in Inlinks/Outlinks | P1 | 04 · E Custom Link Positions | offen |
| INTEG-090 | Verbindung und Auswahl von Konto, Property, Datenstream | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-091 | Zeitraum | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-092 | Metrikauswahl | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-093 | Seitendimension (vollständige URL vs. Landingpage) | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-094 | Dimensionsfilter | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-095 | Abgleichoptionen und Ergebnislimit | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-096 | Paginierung großer Reports | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-097 | Analytics-Tab: Spalten | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-098 | Analytics-Tab: Filter | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-099 | GA4-Kontingente | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-100 | GA4-Daten in Segmenten und Reports | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-101 | Kompatibilitätsprüfung von GA4-Metriken und -Dimensionen | P1 | 04 · F Google Analytics 4 | offen |
| INTEG-110 | Verbindung und Property-Auswahl | P1 | 04 · G Search Console – Search Analytics | offen |
| INTEG-111 | Zeitraum und Datenstand | P1 | 04 · G Search Console – Search Analytics | offen |
| INTEG-112 | Suchtyp | P1 | 04 · G Search Console – Search Analytics | offen |
| INTEG-113 | Dimensionsfilter (Suchanfrage, Land, Gerät) | P1 | 04 · G Search Console – Search Analytics | offen |
| INTEG-114 | Paginierung und Zeilenlimit | P1 | 04 · G Search Console – Search Analytics | offen |
| INTEG-115 | Search-Console-Tab: Spalten | P1 | 04 · G Search Console – Search Analytics | offen |
| INTEG-116 | Search-Console-Tab: Search-Analytics-Filter | P1 | 04 · G Search Console – Search Analytics | offen |
| INTEG-117 | Aggregation und Doppel-URLs | P1 | 04 · G Search Console – Search Analytics | offen |
| INTEG-120 | URL Inspection aktivieren und Tageskontingent | P1 | 04 · H Search Console – URL Inspection | offen |
| INTEG-121 | Nicht-indexierbare URLs auslassen | P1 | 04 · H Search Console – URL Inspection | offen |
| INTEG-122 | Mehrere Properties verwenden | P1 | 04 · H Search Console – URL Inspection | offen |
| INTEG-123 | URL-Inspection-Spalten | P1 | 04 · H Search Console – URL Inspection | offen |
| INTEG-124 | Ableitung der Spalte „Summary" | P1 | 04 · H Search Console – URL Inspection | offen |
| INTEG-125 | URL-Inspection-Filter | P1 | 04 · H Search Console – URL Inspection | offen |
| INTEG-126 | Detailansicht URL Inspection | P2 | 04 · H Search Console – URL Inspection | offen |
| INTEG-127 | Fehlerbehandlung URL Inspection | P1 | 04 · H Search Console – URL Inspection | offen |
| INTEG-128 | Read-Timeout für Search-Console-Anfragen | P1 | 04 · H Search Console – URL Inspection | offen |
| INTEG-130 | PSI-Verbindung und Quelle (Remote/Lokal) | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-131 | Gerätetyp | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-132 | Lokaler Lighthouse-Lauf | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-133 | Metrikgruppen-Auswahl | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-134 | Overview-Metriken | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-135 | CrUX-Felddaten (URL und Origin) | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-136 | Lighthouse-Labordaten | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-137 | Insights-Spalten | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-138 | Diagnostics-Spalten | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-139 | PageSpeed-Tab: Filter | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-140 | Mobile-Prüfungen aus Lighthouse | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-141 | Accessibility-Score aus Lighthouse | P2 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-142 | PSI-Ratenbegrenzung und Durchsatz | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-143 | PSI-/Lighthouse-Fehlerbehandlung | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-144 | Lighthouse-Details (unterer Tab) und Ressourcen-Export | P2 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-145 | Report „PageSpeed Opportunities Summary" | P2 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-146 | Nicht unterstützte Plugins (Mobile) | P2 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-147 | Mobile Alternate Link | P1 | 04 · I PageSpeed Insights / Lighthouse | offen |
| INTEG-150 | Ebenen der Linkmetriken | P1 | 04 · J Linkmetriken | offen |
| INTEG-151 | Link-Metrics-Tab | P1 | 04 · J Linkmetriken | offen |
| INTEG-152 | Majestic | P1 | 04 · J Linkmetriken | offen |
| INTEG-153 | Ahrefs (API v3, OAuth) | P1 | 04 · J Linkmetriken | offen |
| INTEG-154 | Moz (API v3) | P1 | 04 · J Linkmetriken | offen |
| INTEG-155 | Batching und Verbrauchsschätzung | P2 | 04 · J Linkmetriken | offen |
| INTEG-156 | Erweiterte Linkmetriken (Topics, Anchor-Texte) | P2 | 04 · J Linkmetriken | offen |
| INTEG-157 | Ahrefs-Optionen: Land und Volumenbasis | P1 | 04 · J Linkmetriken | offen |
| INTEG-160 | KI-Anbieterkonten | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-161 | OpenAI-kompatible und eigene Endpunkte | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-162 | Prompt-Konfiguration | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-163 | Inhaltseingaben für Prompts | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-164 | Erweiterter Prompt mit mehreren Zielen und Platzhaltern | P2 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-165 | Prompt nur auf Segment anwenden | P2 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-166 | Modellkategorien | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-167 | Bild- und Sprachausgabe | P2 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-168 | Prompt-Bibliothek (mitgeliefert) | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-169 | Benutzer-Prompts speichern, Import/Export | P2 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-170 | Prompt-Tester | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-171 | Ratenbegrenzung je Modell | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-172 | Inhaltslänge begrenzen | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-173 | Leere Eingaben nicht senden | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-174 | KI-Tab und Ergebnisspalten | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-175 | Modellparameter | P2 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-176 | Kostenschutz | P2 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-177 | Systemweiter System-Prompt je Anbieter | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-178 | Tokenverbrauch in den Kontoinformationen | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-179 | Request-Timeout für KI-Anbieter | P1 | 04 · K KI-Anbieter & Prompts | offen |
| INTEG-180 | Embedding-Funktionalität aktivieren | P1 | 04 · L Embeddings & semantische Analyse | offen |
| INTEG-181 | Semantische Ähnlichkeit | P1 | 04 · L Embeddings & semantische Analyse | offen |
| INTEG-182 | Low Relevance Content | P1 | 04 · L Embeddings & semantische Analyse | offen |
| INTEG-183 | Detailansicht „Semantic Similarity" | P1 | 04 · L Embeddings & semantische Analyse | offen |
| INTEG-184 | Embedding-Filterregeln | P2 | 04 · L Embeddings & semantische Analyse | offen |
| INTEG-185 | Semantische Suche | P2 | 04 · L Embeddings & semantische Analyse | offen |
| INTEG-186 | Content-Cluster-Diagramm | P2 | 04 · L Embeddings & semantische Analyse | offen |
| INTEG-189 | Voreinstellungen für die Dimensionsreduktion | P2 | 04 · L Embeddings & semantische Analyse | offen |
| INTEG-187 | Dimensionsreduktion der Vektoren | P2 | 04 · L Embeddings & semantische Analyse | offen |
| INTEG-188 | Mehrere Embedding-Sätze | P2 | 04 · L Embeddings & semantische Analyse | offen |
| UI-001 | Fenstergrundlayout mit drei Bereichen | P0 | 05 · A. Hauptfenster | offen |
| UI-002 | Menüleiste | P0 | 05 · A. Hauptfenster | offen |
| UI-003 | Kopfleiste: URL-Eingabe, Modus, Bereich, Start/Pause/Löschen | P0 | 05 · A. Hauptfenster | offen |
| UI-004 | Top-Tabs: Anzeige, Reihenfolge, Ausblenden | P0 (Anzeige, Wechseln) / P1 (Umsortieren, Ausblenden, Konfigurieren) | 05 · A. Hauptfenster | offen |
| UI-005 | Fokus-Modus | P2 | 05 · A. Hauptfenster | offen |
| UI-006 | Filter-Dropdown je Tab | P0 | 05 · A. Hauptfenster | offen |
| UI-007 | Datentabelle: Grundverhalten | P0 | 05 · A. Hauptfenster | offen |
| UI-008 | Spaltenauswahl und -reihenfolge (Schnellmenü) | P0 (ein/aus) / P1 (Drag & Drop, Persistenz) | 05 · A. Hauptfenster | offen |
| UI-009 | Erweiterter Spalten-Konfigurator | P1 | 05 · A. Hauptfenster | offen |
| UI-010 | Sortierung | P0 | 05 · A. Hauptfenster | offen |
| UI-011 | Suche in der oberen Tabelle | P0 (Klartext auf Address) / P1 (Regex, Spaltenwahl, Regeln, Groß/Klein) | 05 · A. Hauptfenster | offen |
| UI-012 | Zeilenauswahl und Kopieren | P0 | 05 · A. Hauptfenster | offen |
| UI-013 | Fortschritts- und Statusleiste | P0 | 05 · A. Hauptfenster | offen |
| UI-014 | Zeilenzähler und Filterergebnis | P0 | 05 · A. Hauptfenster | offen |
| UI-015 | Export der aktuellen Ansicht | P0 | 05 · A. Hauptfenster | offen |
| UI-016 | Leer-, Hinweis- und Voraussetzungszustände | P1 | 05 · A. Hauptfenster | offen |
| UI-017 | Baumansicht der oberen Tabelle | P2 | 05 · A. Hauptfenster | offen |
| UI-018 | Live-Aktualisierung während des Crawls | P0 | 05 · A. Hauptfenster | offen |
| UI-019 | Benachrichtigungen und Fehlerdialoge | P1 | 05 · A. Hauptfenster | offen |
| UI-020 | Segment-Spalte und Segmentfilter in Tabellen | P1 | 05 · A. Hauptfenster | offen |
| UI-030 | Detailfenster: Grundverhalten | P0 | 05 · B. Unteres Detailfenster | offen |
| UI-031 | URL Details | P0 | 05 · B. Unteres Detailfenster | offen |
| UI-032 | Inlinks | P0 | 05 · B. Unteres Detailfenster | offen |
| UI-033 | Outlinks | P0 | 05 · B. Unteres Detailfenster | offen |
| UI-034 | Ankertext-Aggregation (Ansicht „Anchors“) | P1 | 05 · B. Unteres Detailfenster | offen |
| UI-035 | Image Details | P1 | 05 · B. Unteres Detailfenster | offen |
| UI-036 | Resources | P1 | 05 · B. Unteres Detailfenster | offen |
| UI-037 | SERP Snippet | P1 | 05 · B. Unteres Detailfenster | offen |
| UI-038 | Pixelbreiten-Engine | P0 (wird auch von den Filtern „Over/Below X Pixels“ genutzt) | 05 · B. Unteres Detailfenster | offen |
| UI-039 | Rendered Page (Screenshot) | P1 | 05 · B. Unteres Detailfenster | offen |
| UI-040 | Chrome Console Log | P1 | 05 · B. Unteres Detailfenster | offen |
| UI-041 | View Source mit Diff | P1 | 05 · B. Unteres Detailfenster | offen |
| UI-042 | HTTP Headers | P1 | 05 · B. Unteres Detailfenster | offen |
| UI-043 | Cookies | P2 | 05 · B. Unteres Detailfenster | offen |
| UI-044 | Duplicate Details | P1 | 05 · B. Unteres Detailfenster | offen |
| UI-045 | Structured Data Details | P1 | 05 · B. Unteres Detailfenster | offen |
| UI-046 | Lighthouse Details | P2 | 05 · B. Unteres Detailfenster | offen |
| UI-047 | Spelling & Grammar Details | P2 | 05 · B. Unteres Detailfenster | offen |
| UI-048 | Accessibility Details | P2 | 05 · B. Unteres Detailfenster | offen |
| UI-049 | N-Grams | P2 | 05 · B. Unteres Detailfenster | offen |
| UI-050 | Crawl-Pfad | P1 | 05 · B. Unteres Detailfenster | offen |
| UI-060 | Seitenleiste: Grundverhalten | P0 | 05 · C. Rechte Seitenleiste | offen |
| UI-061 | Overview | P0 | 05 · C. Rechte Seitenleiste | offen |
| UI-062 | Issues-Tab | P0 | 05 · C. Rechte Seitenleiste | offen |
| UI-063 | Issue-Klassifizierung (Typ & Priorität) | P0 | 05 · C. Rechte Seitenleiste | offen |
| UI-064 | Issue-Katalog | P0 (Kategorien Response Codes bis Links) / P1 (übrige) | 05 · C. Rechte Seitenleiste | offen |
| UI-065 | Issue-Erklärungen | P1 | 05 · C. Rechte Seitenleiste | offen |
| UI-066 | Site Structure | P1 | 05 · C. Rechte Seitenleiste | offen |
| UI-067 | Segments-Tab | P1 | 05 · C. Rechte Seitenleiste | offen |
| UI-068 | Segment-Konfiguration | P1 | 05 · C. Rechte Seitenleiste | offen |
| UI-069 | Response Times | P1 | 05 · C. Rechte Seitenleiste | offen |
| UI-070 | API-Tab | P1 | 05 · C. Rechte Seitenleiste | offen |
| UI-071 | Spelling & Grammar (Seitenleiste) | P2 | 05 · C. Rechte Seitenleiste | offen |
| UI-072 | Semantische Suche | P2 | 05 · C. Rechte Seitenleiste | offen |
| UI-080 | Crawl-Analyse-Nachlauf | P0 (Mechanik + Link Score) / P1 (alle Teilschritte) | 05 · D. Crawl-Analyse & Link Score | offen |
| UI-081 | Link Score | P0 | 05 · D. Crawl-Analyse & Link Score | offen |
| UI-082 | Analyse-Status und Invalidierung | P1 | 05 · D. Crawl-Analyse & Link Score | offen |
| UI-090 | Visualisierungs-Rahmen | P1 | 05 · E. Visualisierungen | offen |
| UI-091 | Crawl-Baum (Tree Graph) | P1 | 05 · E. Visualisierungen | offen |
| UI-092 | Kraftgerichteter Crawl-Graph | P1 | 05 · E. Visualisierungen | offen |
| UI-093 | Verzeichnisbaum (Tree Graph) | P1 | 05 · E. Visualisierungen | offen |
| UI-094 | Kraftgerichteter Verzeichnisgraph | P2 | 05 · E. Visualisierungen | offen |
| UI-095 | Knotenfarbe und -größe nach Metrik | P1 | 05 · E. Visualisierungen | offen |
| UI-096 | Knotenbudget, Teilbaum öffnen, Wurzel setzen | P1 | 05 · E. Visualisierungen | offen |
| UI-097 | Links eines Knotens einblenden | P2 | 05 · E. Visualisierungen | offen |
| UI-098 | 3D-Varianten | P2 | 05 · E. Visualisierungen | offen |
| UI-099 | Inlink-Ankertext-Wortwolke | P1 | 05 · E. Visualisierungen | offen |
| UI-100 | Body-Text-Wortwolke | P2 | 05 · E. Visualisierungen | offen |
| UI-101 | Content-Cluster-Diagramm | P2 | 05 · E. Visualisierungen | offen |
| UI-110 | Kontextmenü der oberen Tabelle | P0 (Kopieren, Im Browser öffnen, Erneut crawlen, Entfernen, Export) / P1 (Rest) | 05 · F. Interaktion | offen |
| UI-111 | Kontextmenü im Detailfenster | P1 | 05 · F. Interaktion | offen |
| UI-112 | Kontextmenü der Tabs | P1 | 05 · F. Interaktion | offen |
| UI-113 | Tastaturkürzel | P0 (Grundkürzel) / P1 (Rest) | 05 · F. Interaktion | offen |
| UI-114 | Erneut crawlen und Entfernen | P0 | 05 · F. Interaktion | offen |
| UI-115 | Drag & Drop | P2 | 05 · F. Interaktion | offen |
| UI-116 | Konfigurations-Diff-Fenster | P2 | 05 · F. Interaktion | offen |
| UI-117 | Externe Prüfwerkzeuge öffnen | P2 | 05 · F. Interaktion | offen |
| UI-120 | Erscheinungsbild (Theme, Akzentfarbe) | P1 | 05 · G. Einstellungen | offen |
| UI-121 | Sprache | P0 (de, en) / P1 (es, fr, it) / P2 (weitere per Community) | 05 · G. Einstellungen | offen |
| UI-122 | Speichermodus | P0 | 05 · G. Einstellungen | offen |
| UI-123 | Speicher- und Cache-Grenzen | P1 | 05 · G. Einstellungen | offen |
| UI-124 | Crawl-Verwaltung und Aufbewahrung | P1 | 05 · G. Einstellungen | offen |
| UI-125 | Persistenz des UI-Zustands | P1 | 05 · G. Einstellungen | offen |
| UI-126 | Barrierefreiheit der App | P1 | 05 · G. Einstellungen | offen |
| UI-127 | Diagnose und Debug-Info | P2 | 05 · G. Einstellungen | offen |
| UI-128 | Nutzungsstatistik (Hilfe-Menü) | P2 | 05 · G. Einstellungen | offen |
| UI-129 | MCP-Menü | P2 | 05 · G. Einstellungen | offen |
| UI-140 | Virtualisierte Tabellen | P0 | 05 · H. Performance | offen |
| UI-141 | Server-seitiges Sortieren, Filtern, Suchen | P0 | 05 · H. Performance | offen |
| UI-142 | Seitenweises Datenprotokoll zwischen Kern und UI | P0 | 05 · H. Performance | offen |
| UI-143 | Reaktionsfähigkeit unter Crawl-Last | P0 | 05 · H. Performance | offen |
| UI-144 | Speicherbudget des UI-Prozesses | P0 | 05 · H. Performance | offen |
| UI-145 | Ladezeiten Detailfenster und Seitenleiste | P0 | 05 · H. Performance | offen |
| UI-146 | Visualisierungs-Performance | P1 | 05 · H. Performance | offen |
| UI-147 | Start- und Öffnungszeiten | P1 | 05 · H. Performance | offen |
| UI-148 | Performance-Regressionstests | P1 | 05 · H. Performance | offen |
| OUT-001 | Gemeinsame Export-Pipeline | P0 | 06 · 1 Export-Engine & Formate | offen |
| OUT-002 | CSV-Format | P0 | 06 · 1 Export-Engine & Formate | offen |
| OUT-003 | Excel-Workbook (XLSX) | P0 | 06 · 1 Export-Engine & Formate | offen |
| OUT-004 | Excel 97–2004 (XLS) | P2 | 06 · 1 Export-Engine & Formate | offen |
| OUT-005 | Google-Sheets-Export | P1 | 06 · 1 Export-Engine & Formate | offen |
| OUT-006 | Exportdialog und Formatwahl | P0 | 06 · 1 Export-Engine & Formate | offen |
| OUT-007 | Konsolidierung mehrerer Exporte in einer Arbeitsmappe | P1 | 06 · 1 Export-Engine & Formate | offen |
| OUT-008 | Dateinamen-Konvention | P0 | 06 · 1 Export-Engine & Formate | offen |
| OUT-009 | Spaltenauswahl und -reihenfolge | P1 | 06 · 1 Export-Engine & Formate | offen |
| OUT-010 | Exportfortschritt und Abbruch | P1 | 06 · 1 Export-Engine & Formate | offen |
| OUT-011 | Segment-Einschränkung von Exporten | P1 | 06 · 1 Export-Engine & Formate | offen |
| OUT-012 | Leere Exporte überspringen | P1 | 06 · 1 Export-Engine & Formate | offen |
| OUT-013 | Ausgabeordner: Überschreiben vs. Zeitstempel-Ordner | P0 | 06 · 1 Export-Engine & Formate | offen |
| OUT-014 | Kopieren aus Tabellen in die Zwischenablage | P2 | 06 · 1 Export-Engine & Formate | offen |
| OUT-015 | Exporte in Listenmodus-Reihenfolge | P1 | 06 · 1 Export-Engine & Formate | offen |
| OUT-016 | Formel- und Injection-Schutz für CSV | P2 | 06 · 1 Export-Engine & Formate | offen |
| OUT-020 | Export des aktuellen Haupt-Tabs inkl. Filter | P0 | 06 · 2 Tab- und Detail-Exporte | offen |
| OUT-021 | Export der unteren Detail-Tabs | P0 (Inlinks, Outlinks), P1 (übrige) | 06 · 2 Tab- und Detail-Exporte | offen |
| OUT-022 | Mehrfachauswahl-Export der Detail-Tabs | P1 | 06 · 2 Tab- und Detail-Exporte | offen |
| OUT-023 | Crawl-Path-Report einer URL | P1 | 06 · 2 Tab- und Detail-Exporte | offen |
| OUT-024 | Exporte im unteren Fenster: SERP Snippet, Rendered Page, View Source | P2 | 06 · 2 Tab- und Detail-Exporte | offen |
| OUT-025 | Export der rechten Übersichtsfenster | P1 | 06 · 2 Tab- und Detail-Exporte | offen |
| OUT-026 | Export der Visualisierungen | P2 | 06 · 2 Tab- und Detail-Exporte | offen |
| OUT-027 | Export der Suchergebnisse aus „View Source“/HTML-Suche | P2 | 06 · 2 Tab- und Detail-Exporte | offen |
| OUT-030 | Queued URLs | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-031 | Links: All Inlinks (Standard-Linkformat) | P0 | 06 · 3 Bulk-Exporte | offen |
| OUT-032 | Links: All Outlinks | P0 | 06 · 3 Bulk-Exporte | offen |
| OUT-033 | Links: All Anchor Text | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-034 | Links: External Links | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-035 | Links: Internal Nofollow Outlinks | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-036 | Links: Internal Outlinks With No Anchor Text | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-037 | Links: Non-Descriptive Anchor Text In Internal Outlinks | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-038 | Links: Follow & Nofollow Internal Inlinks To Page | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-039 | Links: Internal Nofollow Inlinks Only | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-040 | Links: Outlinks to Localhost | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-041 | Links: Pages With Uncrawlable Internal Outlinks | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-042 | Path Type: Absolute / Protocol-Relative / Root-Relative / Path-Relative Links | P2 | 06 · 3 Bulk-Exporte | offen |
| OUT-043 | Web: Screenshots | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-044 | Web: Archived Website | P2 | 06 · 3 Bulk-Exporte | offen |
| OUT-045 | Web: All Page Source | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-046 | Web: All Page Text | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-047 | Web: All PDF Documents / All PDF Content | P2 | 06 · 3 Bulk-Exporte | offen |
| OUT-048 | Web: All HTTP Request Headers / All HTTP Response Headers | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-049 | Web: All Cookies | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-050 | Response Codes: Inlinks-Exporte je Filter | P0 | 06 · 3 Bulk-Exporte | offen |
| OUT-051 | Security: Exporte je Filter | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-052 | Content: Duplikat-, Ähnlichkeits- und Rechtschreib-Exporte | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-053 | Images: Exporte je Filter | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-054 | Canonicals: Exporte je Filter | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-055 | Directives: Exporte je Filter | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-056 | Structured Data: Exporte (RDF-Tripel) | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-057 | Sitemaps: Exporte je Filter | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-058 | JavaScript: Exporte je Filter | P2 | 06 · 3 Bulk-Exporte | offen |
| OUT-059 | AMP: Exporte je Filter | P2 | 06 · 3 Bulk-Exporte | offen |
| OUT-060 | Custom Search: Exporte je Suchfilter | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-061 | Custom Extraction: Exporte | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-062 | Accessibility: All Violations und je WCAG-Stufe | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-063 | Issues: Alle Issues als Dateisammlung | P0 | 06 · 3 Bulk-Exporte | offen |
| OUT-064 | URL Inspection: Rich Results / Referring Pages / Sitemaps | P2 | 06 · 3 Bulk-Exporte | offen |
| OUT-065 | AI: generierte Bilder und Audiodateien | P2 | 06 · 3 Bulk-Exporte | offen |
| OUT-066 | Weitere Tab-Bulk-Exporte (generische Regel) | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-067 | Response Codes: All Error Inlinks | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-075 | Multi-Export (Mehrfachexport mit Vorlagen) | P1 | 06 · 3 Bulk-Exporte | offen |
| OUT-080 | Crawl Overview | P0 | 06 · 4 Reports | offen |
| OUT-081 | Redirects: All Redirects | P0 | 06 · 4 Reports | offen |
| OUT-082 | Redirects: Redirect Chains | P0 | 06 · 4 Reports | offen |
| OUT-083 | Redirects: Redirect & Canonical Chains | P1 | 06 · 4 Reports | offen |
| OUT-084 | Redirects: Redirects to Errors | P1 | 06 · 4 Reports | offen |
| OUT-085 | Ketten-Reports im Listenmodus | P1 | 06 · 4 Reports | offen |
| OUT-086 | Canonicals: Canonical Chains | P1 | 06 · 4 Reports | offen |
| OUT-087 | Canonicals: Non-Indexable Canonicals | P1 | 06 · 4 Reports | offen |
| OUT-088 | Pagination: Non-200 Pagination URLs | P2 | 06 · 4 Reports | offen |
| OUT-089 | Pagination: Unlinked Pagination URLs | P2 | 06 · 4 Reports | offen |
| OUT-090 | Hreflang: All Hreflang URLs | P1 | 06 · 4 Reports | offen |
| OUT-091 | Hreflang: Non-200 Hreflang URLs | P1 | 06 · 4 Reports | offen |
| OUT-092 | Hreflang: Unlinked Hreflang URLs | P1 | 06 · 4 Reports | offen |
| OUT-093 | Hreflang: Missing Return Links | P1 | 06 · 4 Reports | offen |
| OUT-094 | Hreflang: Inconsistent Language & Region Return Links | P1 | 06 · 4 Reports | offen |
| OUT-095 | Hreflang: Non Canonical Return Links | P1 | 06 · 4 Reports | offen |
| OUT-096 | Hreflang: Noindex Return Links | P1 | 06 · 4 Reports | offen |
| OUT-097 | Insecure Content | P1 | 06 · 4 Reports | offen |
| OUT-098 | SERP Summary | P1 | 06 · 4 Reports | offen |
| OUT-099 | Orphan Pages | P1 | 06 · 4 Reports | offen |
| OUT-100 | Structured Data: Validation Errors & Warnings Summary | P1 | 06 · 4 Reports | offen |
| OUT-101 | Structured Data: Validation Errors & Warnings | P1 | 06 · 4 Reports | offen |
| OUT-102 | Structured Data: Google Rich Results Features Summary | P1 | 06 · 4 Reports | offen |
| OUT-103 | Structured Data: Google Rich Results Features | P1 | 06 · 4 Reports | offen |
| OUT-104 | PageSpeed: Opportunities Summary | P2 | 06 · 4 Reports | offen |
| OUT-105 | PageSpeed: CSS Coverage Summary | P2 | 06 · 4 Reports | offen |
| OUT-106 | PageSpeed: JavaScript Coverage Summary | P2 | 06 · 4 Reports | offen |
| OUT-107 | JavaScript: Chrome Console Log Summary | P2 | 06 · 4 Reports | offen |
| OUT-108 | Mobile | P2 | 06 · 4 Reports | offen |
| OUT-109 | Accessibility: Violations Summary | P1 | 06 · 4 Reports | offen |
| OUT-110 | HTTP Header Summary | P1 | 06 · 4 Reports | offen |
| OUT-111 | Cookie Summary | P1 | 06 · 4 Reports | offen |
| OUT-112 | Report-Verfügbarkeit und Voraussetzungen | P0 | 06 · 4 Reports | offen |
| OUT-113 | Reports im Vergleichs- und Segmentkontext | P1 | 06 · 4 Reports | offen |
| OUT-120 | XML-Sitemap-Erzeugung (Grundfunktion) | P0 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-121 | Sitemap: Einschlussoptionen für Seiten | P1 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-122 | Sitemap: Ausschluss einzelner URLs | P1 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-123 | Sitemap: lastmod | P1 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-124 | Sitemap: priority | P1 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-125 | Sitemap: changefreq | P1 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-126 | Sitemap: Bilder in der XML-Sitemap | P1 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-127 | Bild-Sitemap (separat) | P1 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-128 | Sitemap: hreflang | P1 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-129 | Sitemap: Aufteilung und Sitemap-Index | P0 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-130 | Sitemap je Segment | P2 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-131 | Sitemap: XML-Korrektheit und Kodierung | P0 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-132 | Sitemap: gzip-Ausgabe | P2 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-133 | Sitemaps in CLI und Scheduling | P1 | 06 · 5 XML- und Bild-Sitemaps | offen |
| OUT-140 | Compare-Modus: Crawls auswählen | P1 | 06 · 6 Crawl-Vergleich | offen |
| OUT-141 | Current/Previous tauschen | P1 | 06 · 6 Crawl-Vergleich | offen |
| OUT-142 | Vergleichsklassifikation je Filter: Added / New / Removed / Missing | P1 | 06 · 6 Crawl-Vergleich | offen |
| OUT-143 | Vergleich im Issues-Panel | P1 | 06 · 6 Crawl-Vergleich | offen |
| OUT-144 | Change Detection (Elementänderungen) | P1 | 06 · 6 Crawl-Vergleich | offen |
| OUT-145 | Content Change Detection | P2 | 06 · 6 Crawl-Vergleich | offen |
| OUT-146 | URL-Mapping zwischen Crawls | P1 | 06 · 6 Crawl-Vergleich | offen |
| OUT-147 | Crawl-Overview-Export im Vergleichsmodus | P1 | 06 · 6 Crawl-Vergleich | offen |
| OUT-148 | Site Structure im Vergleich | P2 | 06 · 6 Crawl-Vergleich | offen |
| OUT-149 | Inlinks/Outlinks-Vergleich im unteren Fenster | P2 | 06 · 6 Crawl-Vergleich | offen |
| OUT-150 | Gespeicherte Vergleiche | P2 | 06 · 6 Crawl-Vergleich | offen |
| OUT-151 | Tab-Exporte im Vergleichsmodus | P1 | 06 · 6 Crawl-Vergleich | offen |
| OUT-152 | Automatischer Vergleich (Auto Compare) für Scheduling und CLI | P1 | 06 · 6 Crawl-Vergleich | offen |
| OUT-153 | Paritätsprüfungen über Vergleich | P2 | 06 · 6 Crawl-Vergleich | offen |
| OUT-160 | Aufgabenverwaltung | P1 | 06 · 7 Geplante Crawls | offen |
| OUT-161 | Aufgabe: Allgemein (Name, Projekt, Zeitplan) | P1 | 06 · 7 Geplante Crawls | offen |
| OUT-162 | Aufgabe: Crawl-Einstellungen | P1 | 06 · 7 Geplante Crawls | offen |
| OUT-163 | Aufgabe: Ausführungsart (headless) | P1 | 06 · 7 Geplante Crawls | offen |
| OUT-164 | Aufgabe: API-Datenquellen | P2 | 06 · 7 Geplante Crawls | offen |
| OUT-165 | Aufgabe: Exporte | P1 | 06 · 7 Geplante Crawls | offen |
| OUT-166 | Ausführung über den Betriebssystem-Planer | P1 | 06 · 7 Geplante Crawls | offen |
| OUT-167 | Parallele und überlappende Läufe | P1 | 06 · 7 Geplante Crawls | offen |
| OUT-168 | Gültigkeitsprüfung von Aufgaben | P1 | 06 · 7 Geplante Crawls | offen |
| OUT-169 | Laufhistorie | P1 | 06 · 7 Geplante Crawls | offen |
| OUT-170 | Aufgaben exportieren/importieren | P2 | 06 · 7 Geplante Crawls | offen |
| OUT-171 | Laufende geplante Crawls abbrechen | P2 | 06 · 7 Geplante Crawls | offen |
| OUT-180 | CLI-Binary und Aufruf je Plattform | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-181 | Hilfe und Version | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-182 | `--crawl <url>` | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-183 | `--crawl-list <quelle>` | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-184 | `--load-crawl <datei\|id>` | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-185 | `--list-crawls` | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-186 | `--config <datei>` | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-187 | `--auth-config <datei>` | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-188 | `--headless` | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-189 | `--save-crawl` | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-190 | `--output-folder`, `--overwrite`, `--timestamped-output` | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-191 | `--project-name`, `--task-name` | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-192 | `--export-tabs "<Tab>:<Filter>,…"` | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-193 | `--bulk-export "<Menü>:<Eintrag>,…"` | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-194 | `--save-report "<Menü>:<Report>,…"` | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-195 | `--export-format csv\|xls\|xlsx\|gsheet` | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-196 | Zusätzliche Export-Optionen | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-197 | `--create-sitemap`, `--create-images-sitemap` | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-198 | `--google-drive-account <konto>` | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-199 | `--email-on-complete <adresse,…>` | P2 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-200 | `--export-custom-summary "<Metrik>,…"` | P2 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-201 | API-Optionen | P2 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-202 | `--crawl-comparison <id1> <id2>`, `--project-crawl-comparison ["true"\|"false"]` | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-203 | Validierung aller Namen vor dem Crawl | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-204 | Exit-Codes | P0 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-205 | Fortschritt und maschinenlesbare Ausgabe | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-206 | Abbruch per Signal | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-207 | JS-Rendering im Headless-Betrieb | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-208 | Weitere Modi per CLI | P2 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-209 | Aufgabenausführung `--run-task <id>` | P1 | 06 · 8 Kommandozeile / Headless | offen |
| OUT-210 | Zeitreihen-Export je Crawl | P1 | 06 · 9 Looker-Studio-Export | offen |
| OUT-211 | Metrikkatalog | P1 | 06 · 9 Looker-Studio-Export | offen |
| OUT-212 | Spaltenstabilität bei Änderungen | P1 | 06 · 9 Looker-Studio-Export | offen |
| OUT-213 | Segmentierte Zeitreihen | P2 | 06 · 9 Looker-Studio-Export | offen |
| OUT-214 | Looker-Studio-Vorlage und Datentypen | P2 | 06 · 9 Looker-Studio-Export | offen |
| OUT-220 | Konfigurationsprofile speichern, laden, als Standard setzen | P0 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-221 | Format der Konfigurationsdatei | P0 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-222 | Trennung von Crawl-Konfiguration und App-Einstellungen | P1 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-223 | Authentifizierungsprofile | P1 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-224 | Konfigurations-Unterschiede anzeigen | P2 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-225 | Geheimnisse außerhalb von Konfigurationsdateien | P0 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-226 | Versionierung und Migration | P1 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-227 | Speicherorte je Betriebssystem | P0 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-228 | Crawl-Verwaltung | P1 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-229 | Automatische Crawl-Aufbewahrung (Retention) | P1 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-230 | Crawl-Datei speichern und öffnen | P0 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-231 | Speicherort der Crawl-Daten | P1 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-232 | Einzelinstanz und Dateizuordnung beim Öffnen | P1 | 06 · 10 Konfigurationsdateien & Crawl-Dateien | offen |
| OUT-240 | Logdateien | P1 | 06 · 11 Logging & Diagnose | offen |
| OUT-241 | Log-Level | P1 | 06 · 11 Logging & Diagnose | offen |
| OUT-242 | Rotation und Größenbegrenzung | P1 | 06 · 11 Logging & Diagnose | offen |
| OUT-243 | Schutz sensibler Daten in Logs | P0 | 06 · 11 Logging & Diagnose | offen |
| OUT-244 | Diagnosepaket („Logs speichern“) und Debug-Informationen | P1 | 06 · 11 Logging & Diagnose | offen |
| OUT-245 | Lokale Nutzungsstatistik | P2 | 06 · 11 Logging & Diagnose | offen |
| OUT-250 | Update-Prüfung | P1 | 06 · 12 Update-Mechanismus | offen |
| OUT-251 | Herunterladen und Installieren | P1 | 06 · 12 Update-Mechanismus | offen |
| OUT-252 | Integrität und Signatur von Updates | P0 | 06 · 12 Update-Mechanismus | offen |
| OUT-253 | Updates in verwalteten Umgebungen | P2 | 06 · 12 Update-Mechanismus | offen |
| OUT-254 | Update-Kanäle | P2 | 06 · 12 Update-Mechanismus | offen |
| OUT-255 | Datenkompatibilität über Versionen | P0 | 06 · 12 Update-Mechanismus | offen |
| OUT-260 | macOS Apple Silicon (arm64) | P0 | 06 · 13 Plattformanforderungen | offen |
| OUT-261 | macOS Intel (x86_64) | P0 | 06 · 13 Plattformanforderungen | offen |
| OUT-262 | Windows x64 | P0 | 06 · 13 Plattformanforderungen | offen |
| OUT-263 | Windows on ARM (arm64) | P2 | 06 · 13 Plattformanforderungen | offen |
| OUT-264 | Linux | P2 | 06 · 13 Plattformanforderungen | offen |
| OUT-265 | macOS: Paketierung, Signatur, Notarisierung | P0 | 06 · 13 Plattformanforderungen | offen |
| OUT-266 | Windows: Installer und Signatur | P0 | 06 · 13 Plattformanforderungen | offen |
| OUT-267 | Hardware-Anforderungen und Ressourcenlimits | P1 | 06 · 13 Plattformanforderungen | offen |
| OUT-268 | Dateisystem, Pfade, Zeichenkodierung | P0 | 06 · 13 Plattformanforderungen | offen |
| OUT-269 | Proxy | P1 | 06 · 13 Plattformanforderungen | offen |
| OUT-270 | Vertrauenswürdige Zertifikate | P1 | 06 · 13 Plattformanforderungen | offen |
| OUT-271 | Oberflächensprache und Theme | P1 | 06 · 13 Plattformanforderungen | offen |
| OUT-272 | Dateizuordnungen und Desktop-Integration | P1 | 06 · 13 Plattformanforderungen | offen |
| OUT-280 | E-Mail-Konten | P1 | 06 · 14 Benachrichtigungen & Automatisierungs-Schnittstellen | offen |
| OUT-281 | E-Mail bei Crawl-Ende (global und je Aufgabe) | P1 | 06 · 14 Benachrichtigungen & Automatisierungs-Schnittstellen | offen |
| OUT-282 | Inhalt der Abschlussmail | P1 | 06 · 14 Benachrichtigungen & Automatisierungs-Schnittstellen | offen |
| OUT-283 | Exporte als E-Mail-Anhang | P2 | 06 · 14 Benachrichtigungen & Automatisierungs-Schnittstellen | offen |
| OUT-284 | Desktop-Benachrichtigung | P2 | 06 · 14 Benachrichtigungen & Automatisierungs-Schnittstellen | offen |
| OUT-285 | MCP-Server (Steuerung durch KI-Assistenten) | P2 | 06 · 14 Benachrichtigungen & Automatisierungs-Schnittstellen | offen |
| PERF-001 | Speicherobergrenze im Textmodus | P0 | 07 · Stabilität & Performance | offen |
| PERF-002 | Durchsatz | P1 | 07 · Stabilität & Performance | offen |
| PERF-003 | Gegendruck (Backpressure) | P0 | 07 · Stabilität & Performance | offen |
| PERF-004 | Reaktionsfähigkeit der Oberfläche | P0 | 07 · Stabilität & Performance | offen |
| PERF-005 | Absturzsicherheit & Fortsetzen | P0 | 07 · Stabilität & Performance | offen |
| PERF-006 | JavaScript-Rendering unter Kontrolle | P1 | 07 · Stabilität & Performance | offen |
| PERF-007 | Skalierbare Nachlauf-Analysen | P1 | 07 · Stabilität & Performance | offen |
| PERF-008 | Große Exporte | P1 | 07 · Stabilität & Performance | offen |
| PERF-009 | Lasttest-Suite | P0 | 07 · Stabilität & Performance | offen |

## 6 Gesamtbewertung der Spezifikations-Vollständigkeit

**93 / 100**

Herleitung (Abzüge vom Vollwert 100):
- −3: fünf kleine Lücken/Teilabdeckungen gegenüber 24.3 (G-1 … G-5), davon nur G-1 und G-2 echte Funktionslücken.
- −4: 17 inhaltliche Widersprüche zwischen Bereichsdokumenten (Abschnitt 4.1); solange sie bestehen, ist die Spec an diesen Stellen nicht eindeutig implementierbar.
- Kein Abzug für die bewussten, dokumentierten Paritätslücken (Linux-Pakete, Lizenz-Gating entfällt, Headless immer aktiv), da sie als OS-Festlegung gekennzeichnet sind.
- Positiv: jede Funktion hat Priorität, Akzeptanzkriterien und Fixture; offene Punkte sind je Dokument in Q-/Anhang-Listen gesammelt.

**Empfohlene nächste Schritte für den Planer (Opus):** (1) Konflikte K-1 … K-17 auflösen, indem je Thema genau ein Bereichsdokument die Vollspezifikation trägt und die übrigen auf Verweis gekürzt werden; (2) G-1 und G-2 als neue IDs aufnehmen, G-3 … G-5 als Akzeptanzkriterien; (3) Mischprioritäten (Abschnitt 5) in eindeutige Einzelprioritäten oder getrennte IDs auflösen, damit die Meilensteinzählung stabil ist.

# 09 – DataForSEO-Anbindung (D4S)

Bereich: Anbindung der kostenpflichtigen DataForSEO-API v3 als Datenquelle **über die Parität mit dem Referenzprodukt hinaus**: Verbindung/Konto, gemeinsame Kostenkontrolle und Ergebnis-Cache, Module **Labs Ranked Keywords**, **Backlinks**, **SERP API** (inkl. AI-Overview-Präsenz) und **AI Optimization / LLM Mentions**, Verknüpfung mit Crawl-Daten (neue Filter und Issues), Bulk-Exporte, CLI und Scheduling.

Einordnung: Gebaut wird nach Release 1.0 und nach den KI-Funktionen als **Meilenstein M5** (`docs/ARCHITECTURE.md`; Zuordnung in `README.md`, „Meilensteinzuordnung“). Prioritäten innerhalb dieses Bereichs: **P0** = erste Ausbaustufe von M5, **P1** = vollständiger Ausbau, **P2** = später. Der gemeinsame `BudgetLedger` (AI-013) und der gemeinsame Kostenvoranschlag (AI-012) entstehen bereits mit M4; M5 meldet DataForSEO dort an. Umsetzung im Crate `os-integrations` (Modul `dataforseo`); Speicherung über `os-store`, Filter/Issues über `os-analyze`, Exporte über `os-export`, CLI über `os-cli`. Mock-Server in `os-testsite`.

Alle Beschreibungen sind eigenständig formuliert. API-Fakten wurden am **13.10.2026** gegen die öffentliche Doku `https://docs.dataforseo.com/v3/` und die Preisseiten `https://dataforseo.com/pricing…` geprüft (Abschnitt „Verifizierte API-Fakten“). Werte aus eigener Festlegung sind mit **(OS-Festlegung)** markiert, aus Doku-Lücken abgeleitete Annahmen mit **(Annahme, zu verifizieren)**.

Überarbeitung 13.10.2026 nach Fable-Review (Score 76): einheitliche Regel für leere Ergebnisse, eigenes Kostenprotokoll `dfs_cost_log` mit benannter Abhängigkeit zu 08, additive SERP-Zuschlagsformel mit Unter-/Obergrenze, Senderegeln für Query-Strings und Suchoperatoren, Fehlerfälle für Abschnitt F, LLM-Marktliste (Q7 geklärt), Sondierung für „Automatisch“, Abweichung von INTEG-005, Änderungsbedarf an INTEG-001/INTEG-019, Preis-Zuordnung aus `user_data`, AI Overview im Knowledge Graph, URL-Form beim Bulk-Vorfilter, AI-Mode-Endpunkt. Alle Funktions-IDs sind unverändert.

Überarbeitung 13.10.2026 nach der zweiten Fable-Prüfung (Score 91, „approved“): Alle Befunde und Empfehlungen sind umgesetzt; Empfehlungen gelten als Nutzerentscheidung und sind an den Fundstellen als **Entscheidung (Fable-Empfehlung)** gekennzeichnet. Inhalt: Gruppenwerte von `multi_target_metrics` aus `items[]` (D4S-053, Abschnitt F), `historical` ohne `platform` bei zwei verfügbaren Plattformen (D4S-054), Akzeptanzkriterium mit zwölf Einträgen (D4S-001), Schlüsselbund-Schema nach INTEG-002 (D4S-001), Kostenanzeige nach UI-013 (D4S-071), Freigabe nach 40203 über `user_data` (D4S-004), Rückfall ohne `order_by` (D4S-052), Sandbox ohne LLM-Mentions-Daten (D4S-003), AI-Mode-Abrechnung geklärt (D4S-048), Festlegungen zu Q1 – Q12 und V-D1 – V-D11, Release-Gate M5. Alle Funktions-IDs sind unverändert.

---

## Übersicht

| Abschnitt | IDs | Inhalt | P0 | P1 | P2 |
|---|---|---|---|---|---|
| A Verbindung & Infrastruktur | D4S-001 – D4S-010 | Login/Passwort im Schlüsselbund, Kontostand, Sandbox, Fehlercodes, Ratenlimits, Timeouts/Teilfehler, Markt-Auswahl, Task-Modus mit Wiederaufnahme, Mock-Server, Datenschutz | 9 | 1 | 0 |
| B Kostenkontrolle & Cache | D4S-011 – D4S-018 | Preiskatalog mit Stand-Datum, Kostenschätzung, hartes Budget, Kostenprotokoll, Ergebnis-Cache, Cache-Übernahme, Abfrageumfang, Neuanforderung | 6 | 2 | 0 |
| C Labs Ranked Keywords | D4S-020 – D4S-026 | Keywords je URL, Detail-Tab, Optionen, Domain-Abzug, Bulk-Vorfilter, Relevant Pages, Tab „Organic Keywords“ | 3 | 4 | 0 |
| D Backlinks | D4S-030 – D4S-034 | Kennzahlen je URL und Domain, neue/verlorene Links, einheitliches Linkmetrik-Schema, Backlink-Liste | 2 | 2 | 1 |
| E SERP API | D4S-040 – D4S-048 | Haupt-Keyword, Live-Ranking, SERP-Features, AI-Overview-Zitat, Live/Standard/Priority, Gerät/Ort, Detail-Tab, Mehrfach-Keywords, AI Mode | 4 | 3 | 2 |
| F AI Optimization / LLM Mentions | D4S-050 – D4S-055 | Domain- und Seiten-Erwähnungen, Top Domains/Pages je Keyword, Wettbewerbervergleich, Historie, Erwähnungsdetails | 0 | 4 | 2 |
| G Kombinationen & Issues | D4S-060 – D4S-069 | Nicht indexierbar mit Rankings/Backlinks, Weiterleitungen mit Backlinks, Kannibalisierung, Traffic ohne interne Links, Orphans, falsche URL rankt, AI-Overview ohne Zitat, Keyword fehlt im Title, Issue-Katalog | 4 | 5 | 1 |
| H Ausgabe & Betrieb | D4S-070 – D4S-076 | Internal-Tab/APIs-Modus, Fortschritt & Kostenanzeige, Bulk-Exporte/Reports, CLI, Scheduling, Positionsverlauf, Übergabe an KI-Fragen | 2 | 3 | 2 |

Summe: 62 Funktionen (P0: 30, P1: 24, P2: 8).

## Inhaltsverzeichnis

- D4S-001 Verbindung und Zugangsdaten · P0
- D4S-002 Kontostand, Limits und Live-Preise · P0
- D4S-003 Sandbox-Modus · P0
- D4S-004 Antwortauswertung und Fehlercodes · P0
- D4S-005 Ratenbegrenzung und Parallelität · P0
- D4S-006 Timeouts und Teilfehler · P0
- D4S-007 Markt-Auswahl (Location und Language) · P0
- D4S-008 Standard-Task-Ausführung mit Wiederaufnahme · P1
- D4S-009 Mock-Server und Testbarkeit · P0
- D4S-010 Übertragene Daten und Datenschutz-Hinweis · P0
- D4S-011 Preiskatalog mit Stand-Datum · P0
- D4S-012 Kostenschätzung vor dem Lauf · P0
- D4S-013 Hartes Budget je Crawl · P0
- D4S-014 Kostenprotokoll · P0
- D4S-015 Ergebnis-Cache in der Crawl-Datei · P0
- D4S-016 Cache aus früherem Crawl übernehmen · P1
- D4S-017 Abfrageumfang und Priorisierung · P0
- D4S-018 Neuanforderung und Invalidierung · P1
- D4S-020 Ranked Keywords je URL · P0
- D4S-021 Detail-Tab „Ranked Keywords“ · P0
- D4S-022 Labs-Optionen · P1
- D4S-023 Abrufstrategie „Domain-Abzug“ · P1
- D4S-024 Vorfilter per Bulk Traffic Estimation · P1
- D4S-025 Relevant Pages und Orphan-Kandidaten · P1
- D4S-026 Tab „Organic Keywords“ · P0
- D4S-030 Backlink-Kennzahlen je URL · P0
- D4S-031 Subdomain- und Domain-Ebene · P0
- D4S-032 Neue und verlorene Backlinks · P1
- D4S-033 Einheitliches Linkmetrik-Schema · P1
- D4S-034 Detail-Tab „Backlinks“ (auf Abruf) · P2
- D4S-040 Haupt-Keyword je Seite · P0
- D4S-041 Live-Ranking je Seite · P0
- D4S-042 SERP-Features · P0
- D4S-043 AI-Overview-Präsenz und Zitat · P0
- D4S-044 Ausführungsmodus Live, Standard, Priority · P1
- D4S-045 Erweiterte Orts- und Geräteoptionen · P1
- D4S-046 Detail-Tab „SERP“ · P1
- D4S-047 Mehrere Keywords je Seite · P2
- D4S-048 Google AI Mode · P2
- D4S-050 Domain-Erwähnungen · P1
- D4S-051 Seiten-Erwähnungen · P1
- D4S-052 Top Domains und Top Pages je Keyword · P1
- D4S-053 Wettbewerbervergleich · P1
- D4S-054 Historische Entwicklung · P2
- D4S-055 Erwähnungs-Details · P2
- D4S-060 Nicht indexierbar, aber mit Rankings · P0
- D4S-061 Fehlerseiten und nicht indexierbare Seiten mit Backlinks · P0
- D4S-062 Weiterleitungen mit Backlinks · P1
- D4S-063 Keyword-Kannibalisierung · P0
- D4S-064 Traffic-Seiten mit wenigen internen Links · P1
- D4S-065 Orphan-Seiten mit Rankings oder Erwähnungen · P1
- D4S-066 Andere URL rankt für das Haupt-Keyword · P1
- D4S-067 Top-10-Ranking ohne AI-Overview-Zitat · P1
- D4S-068 Haupt-Keyword fehlt in Title oder H1 · P2
- D4S-069 Issue-Katalog des Bereichs · P0
- D4S-070 Internal-Tab und APIs-Modus · P0
- D4S-071 Fortschritt und Kostenanzeige · P0
- D4S-072 Bulk-Exporte und Reports · P1
- D4S-073 CLI-Optionen · P1
- D4S-074 Scheduling · P1
- D4S-075 Positionsverlauf im Crawl-Vergleich · P2
- D4S-076 DataForSEO-Werte als Eingabe für KI-Fragen · P2

---

## Konventionen für diesen Bereich

- **Übernommene Infrastruktur (nicht duplizieren):** Es gelten unverändert die führenden Einträge aus 04: Verbindungsdialog und Statusautomat (INTEG-001), Schlüsselbund (INTEG-002), Backoff/Retry (INTEG-005), Kontingentzähler (INTEG-006), API-Warteschlange und Fortschritt (INTEG-007), nachträgliches bzw. einzelnes Neuanfordern (INTEG-008/009), Invalidierung bei Konfigurationsänderung (INTEG-010), APIs-Modus (INTEG-012), Integrationsspalten im Internal-Tab (INTEG-013), URL-Abgleich (INTEG-014), Orphan-Aufnahme (INTEG-015), Proxy/Truststore (INTEG-016), Datenschutz (INTEG-018), überschreibbare Basis-URLs (INTEG-019), Linkmetrik-Ebenen und Link-Metrics-Tab (INTEG-150/151). Dieses Dokument beschreibt nur, was für DataForSEO **zusätzlich** oder **abweichend** gilt.
- **Abweichungen von INTEG-005:** Für DataForSEO gelten eigene Vorgaben für gleichzeitige Anfragen (Standard 10, Bereich 1–30) und Anfragen je Minute (D4S-005), weil die API selbst höchstens 30 gleichzeitige Anfragen je Konto zulässt; Backoff-Parameter und „maximale Wiederholungen“ (Standard 5) bleiben wie in INTEG-005.
- **Abgrenzung zu den KI-Funktionen:** KI-Anbieter, Prompts, Embeddings (INTEG-160 ff., INTEG-180 ff.) und der KI-Ausbau mit Crate `os-ai` (Anbieter-Abstraktion, Decision-Engine, eigene Kostenkontrolle, führend in 08) sind nicht Teil dieses Dokuments. DataForSEO führt ein **eigenes** Kostenprotokoll (D4S-014, Tabelle `dfs_cost_log`); `os-ai` protokolliert in `ai_requests` (AI-014). Budget und Kostenschätzung sind dagegen **gemeinsam** (Architekten-Entscheidung vom 13.10.2026): ein Crawl-Budget je Crawl für KI und DataForSEO, verwaltet vom `BudgetLedger` in `os-model` (führend AI-013, hier D4S-013), und ein gemeinsamer Dialog „Kostenvoranschlag“ (AI-012, hier D4S-012). DataForSEO-Werte können als Eingabe für KI-Fragen dienen (D4S-076).
- **Spaltennamen:** kanonische englische Export-Header mit Präfix `DFS ` (z. B. `DFS Ranked Keywords`). In den Link-Metrics-Spalten gilt das Schema aus INTEG-150 `<Anbieter> <Metrik> (<Ebene>)` mit dem Anbieter-Kürzel `DFS` (z. B. `DFS Backlinks (Exact URL)`). Tab-, Filter- und Statusbezeichner sind Schnittstellenbezeichner (CLI, Exporte, Zeitpläne) und bleiben sprachunabhängig.
- **Leerwerte:** Hat die API nichts geliefert oder wurde nicht abgefragt, ist die Zelle leer; `0` heißt „API hat 0 geliefert“. Warum eine Zelle leer ist, steht in der Statusspalte des Moduls.
- **Regel „keine Daten“ vs. „null Treffer“ (verbindlich für alle Module):**
  1. Task-`status_code` **40102** („No Search Results“) → Status `No data`, alle Kennzahlspalten **leer**.
  2. Kein Eintrag für ein gesendetes Ziel in einer Bulk-Antwort (Ziel fehlt in `items`) → Status `No data`, Kennzahlspalten leer.
  3. Task-`status_code` **20000** mit leerem `items` bzw. `total_count = 0` oder `count = 0` → Status `Success`; Zähl- und Summenspalten (z. B. `DFS Ranked Keywords`, `DFS Keywords Top 10`, `DFS Est. Traffic`, `DFS LLM Mentions`) = `0`, abgeleitete Spalten ohne Bezugsgröße (Top-Keyword, Positionen, Prozentwerte) leer.
  Golden-Tests und Filter beziehen sich ausschließlich auf diese Regel; die Mock-Szenarien `empty` (Fall 1), `partial` (Fall 2) und `empty-items` (Fall 3) decken sie ab.
- **Statusspalten je Modul:** `DFS Labs Status`, `DFS Backlinks Status`, `DFS SERP Status`, `DFS LLM Status` (P2 zusätzlich `DFS AI Mode Status`, D4S-048). Wertebereich (abschließend):
  `Success`, `No data`, `Pending`, `Not requested`, `Skipped (non-indexable)`, `Skipped (non-200)`, `Skipped (segment)`, `Skipped (scope limit)`, `Skipped (query string)`, `Skipped (no keyword)`, `Skipped (operator)`, `Skipped (invalid keyword)`, `Skipped (market unsupported)`, `Not connected`, `Budget reached`, `Insufficient funds`, `Quota exceeded`, `Subscription required`, `Error: <Code> <Meldung>` (API-Fehler), `Error: <Meldung>` (vor dem Senden erkannte Fehler, z. B. nicht unterstützte URL-Form), `Error: Timeout nach <n> s`.
- **Herkunftsspalten je Modul:** `DFS Labs Source`, `DFS Backlinks Source`, `DFS SERP Source`, `DFS LLM Source` ∈ {`Live API`, `Task API`, `Cache`, `Sandbox`, `Mock`}; Zeitpunkt der Datenerhebung in `DFS <Modul> Fetched` (ISO 8601 mit Offset).
- **Tabs:** `Organic Keywords` (Labs, neu), `Link Metrics` (bestehend, INTEG-151, erweitert), `Rankings` (SERP API, neu), `LLM Mentions` (neu). Issue-Namen folgen UI-062: `<Tab>: <Filter>`.
- **„Eigene Domain“** ist die registrierbare Domain (Public Suffix List) der Start-URL; Option „Nur eigener Host“ (Standard aus) **(OS-Festlegung)**.
- **URL-Vergleich** zwischen API-Antworten und Crawl-URLs folgt INTEG-014; zusätzlich werden Fragmente (inkl. Text-Fragmente `#:~:text=…`) vor dem Vergleich entfernt.
- **Crawl-Analyse:** Kombinationsfilter (Abschnitt G) werden im Nachlauf berechnet (CRAWL-141), sobald auch die API-Warteschlange leer ist (INTEG-007); vorher zeigen sie den Hinweis „Crawl-Analyse erforderlich“.
- **Beträge** werden intern in Mikro-USD (Ganzzahl, 1 USD = 1.000.000) gerechnet, um Rundungsfehler bei Zeilenpreisen wie 0,000036 USD auszuschließen; Anzeige mit 4 Nachkommastellen und dem Dezimaltrenner der UI-Sprache (UI-121), Export als Dezimalzahl mit Punkt; das Statusleistenfeld `Kosten` folgt dem Format aus UI-013 (führend, **Entscheidung (Fable-Empfehlung)**).

## Abhängigkeiten und Änderungsbedarf in 04 und 08

04 ist für die übernommene Infrastruktur führend; 08 ist führend für `os-ai` und den gemeinsamen `BudgetLedger`. Die Tabelle dokumentiert den Änderungsbedarf, den dieses Dokument ausgelöst hat. **Stand 13.10.2026: alle Punkte sind in 04 bzw. 08 übernommen** (Spalte „Status“); die Spalte „Heutiger Stand“ beschreibt den Zustand vor der Übernahme.

| Nr. | Dokument / ID | Heutiger Stand | Erforderliche Änderung | Ausgelöst durch | Status |
|---|---|---|---|---|---|
| X1 | 04 / INTEG-001 | „Der Dialog enthält genau zehn Einträge …“; UI-Test verbindet „alle zehn Einträge“ | ein zusätzlicher Eintrag „DataForSEO“ (ursprünglich als elfter Eintrag formuliert; mit den fünf KI-Anbietern aus M4 der zwölfte) (Gruppe „Daten-APIs“ oder unter den Linkmetriken); Test-Request von DataForSEO = `GET /v3/appendix/user_data`; Status-Automat unverändert | D4S-001 | übernommen: INTEG-001 mit zwölf Einträgen ab M5, DataForSEO unter „Daten-APIs“ |
| X2 | 04 / INTEG-019 | Mock-Liste ohne DataForSEO; Szenario-Routing über Query-Parameter bzw. Header `X-Scenario`; Mock unter `fixtures/integ/fixture-apis/` | DataForSEO in die Mock-Liste aufnehmen (Routen unter `/__mock/dataforseo/v3/…` in `os-testsite`); der DFS-Mock akzeptiert zusätzlich `X-Scenario`; Abbildung der INTEG-019-Pflichtszenarien: Erfolg → `ok`, Leer → `empty`, 4xx → `invalid-field-one`, Drossel → `ratelimit-x2` (DataForSEO drosselt mit HTTP 200 + 40202 statt HTTP 429) | D4S-009 | übernommen in INTEG-019 (Mock-Liste, `X-Scenario`, Abbildung der Pflichtszenarien) |
| X3 | 04 / INTEG-005 | gleichzeitige Anfragen Standard 5, Bereich 1–50 | Hinweis „Integrationen können abweichende Vorgaben festlegen (z. B. DataForSEO: Standard 10, 1–30, D4S-005)“ | D4S-005 | übernommen in INTEG-005 (inkl. Kriterium für die Anzeige im Reiter „Erweitert“) |
| X4 | 08 / AI-013 | KI-Budget je Crawl nur über `ai_requests`; kein anbieterübergreifendes Budget | [P1] Vor jeder KI-Reservierung zusätzlich das Gesamtbudget aller bezahlten APIs prüfen (Summe `ai_requests` + `dfs_cost_log` + offene Reservierungen beider Seiten); gemeinsamer Reservierungsdienst (Ort offen, Q9) | D4S-013 | übernommen und erweitert: AI-013 führt das gemeinsame Crawl-Budget über `BudgetLedger` in `os-model` (Q9 entschieden); kein P1-Vorbehalt mehr |
| X5 | 08 / Datenhaltung | Tabellen `ai_*` | keine Änderung an `ai_requests`; lesender Zugriff von `os-integrations` auf die Summenspalte `Cost (USD)` für das Gesamtbudget | D4S-013, D4S-014 | übernommen: der `BudgetLedger` liest beide Tabellen; neue gemeinsame Tabelle `api_budget` nur für Grenzwerte (08, Datenhaltung) |

Mit der Übernahme von X4 prüfen KI- und DataForSEO-Requests dasselbe Crawl-Budget; die frühere Einschränkung („Gesamtbudget nur auf DataForSEO-Seite wirksam“, P1-Teil erst nach X4 abnahmefähig) entfällt.

---

## Verifizierte API-Fakten (Stand 13.10.2026)

Diese Tabelle ist die Arbeitsgrundlage; die Implementierung liest Preise und Limits aus dem Preiskatalog (D4S-011) bzw. live aus `user_data` (D4S-002) und kodiert sie nicht hart.

| Thema | Fakt | Quelle |
|---|---|---|
| Basis-URL | `https://api.dataforseo.com/v3/`; Sandbox `https://sandbox.dataforseo.com/v3/` (gleiche Pfade, Dummy-Daten, kostenlos) | docs v3, Appendix Sandbox |
| Authentifizierung | HTTP Basic (`Authorization: Basic base64(login:api_password)`); das API-Passwort ist ein eigenes, von DataForSEO erzeugtes Passwort (nicht das Konto-Passwort); Zugangsdaten nie als URL-Parameter | docs v3/auth |
| Kontodaten | `GET /v3/appendix/user_data` (kostenlos): `money.total`, `money.balance`, `money.limits` (Tag/Minute je Funktion), `money.statistics`, `rates.limits`, `rates.statistics`, `price` (Verschachtelung `price.<api_name>.<func_type>.<func_name>.<priority>` mit `priority_normal`/`priority_high`, Blattfelder `cost_type` (`per_request` oder `per_result`) und `cost` in USD; laut Doku nur die **Basiskosten** des Aufrufs, Kosten zusätzlicher kostenpflichtiger Parameter sind nicht enthalten); `backlinks_subscription_expiry_date` und `llm_mentions_subscription_expiry_date` sind als **veraltet** markiert | Appendix User Data |
| Ratenlimits | 2.000 Aufrufe/Minute je Endpunkt, 30 gleichzeitige Anfragen je Nutzer; Header `X-RateLimit-Limit` / `X-RateLimit-Remaining`; `tasks_ready` max. 20 Aufrufe/Minute | docs v3, Errors, tasks_ready |
| Live vs. Standard | Live: 1 Task je Aufruf, Ergebnis sofort, wird nicht gespeichert (SERP-JSON 30 Tage). Standard: `task_post` (max. 100 Tasks je POST, sonst 40006), `tasks_ready` (max. 1.000 je Aufruf, Einträge 3 Tage), `task_get/advanced/{id}`; Ergebnisse 30 Tage | docs v3, SERP task_post/tasks_ready |
| Antwortformat | HTTP 200 mit `status_code` auf oberster Ebene und je Task (`tasks[].status_code`), `cost` je Antwort und je Task, `tasks_error` | docs v3 |
| Fehlercodes | 20000 Ok, 20100 Task Created, 40006 >100 Tasks, 40100 nicht autorisiert, 40102 keine Ergebnisse, 40104 Konto nicht verifiziert, 40200 Payment Required, 40201 Konto pausiert, 40202 Minutenlimit überschritten, 40203 Tageskostenlimit erreicht, 40204 Abo erforderlich, 40209 zu viele gleichzeitige Anfragen, 40210 Guthaben unzureichend, 40400/40401 nicht gefunden / Task nicht gefunden, 40501 ungültiges Feld, 40601/40602 Task übergeben / in Warteschlange, 50000 interner Fehler | Appendix Errors |
| Labs Ranked Keywords | `POST /v3/dataforseo_labs/google/ranked_keywords/live`; `target` (Domain/Subdomain ohne Protokoll; Seiten-URL mit `https://` oder `www.`, sonst wird die ganze Domain ausgewertet), `location_code`/`language_code`, `item_types` (`organic`, `paid`, `featured_snippet`, `local_pack`, `ai_overview_reference`), `limit` ≤ 1.000, `offset`, `filters` ≤ 8, `order_by` ≤ 3, `historical_serp_mode` (`live`/`lost`/`all`), `ignore_synonyms`, `load_rank_absolute`, `include_clickstream_data` (verdoppelt den Preis); Antwort: `total_count`, `metrics.<typ>.{pos_1, pos_2_3, pos_4_10, …, etv, count, estimated_paid_traffic_cost, is_new, is_up, is_down, is_lost}`, `items[].keyword_data.{keyword, keyword_info.search_volume, keyword_info.cpc, keyword_properties.keyword_difficulty, search_intent_info.main_intent, serp_info.serp_item_types}`, `items[].ranked_serp_element.serp_item.{type, rank_group, rank_absolute, url, etv, rank_changes}` | Labs ranked_keywords |
| Labs weitere | `bulk_traffic_estimation/live` (bis 1.000 Ziele je Task; Domains/Subdomains ohne `https://` und `www.`, Seiten als absolute URL **mit `https://` und `www.`**; `item_types` Standard `organic`, `paid`; Antwort je Item `target`, `metrics.<typ>.{etv,count}`), `relevant_pages/live` (Seiten einer Domain mit `page_address` und `metrics`, `limit` ≤ 1.000, wöchentliche Aktualisierung), `serp_competitors/live` (≤ 200 Keywords), `GET /v3/dataforseo_labs/locations_and_languages` (kostenlos, nur Länder) | Labs-Doku |
| Backlinks | `summary/live` (ein Ziel, `include_subdomains`, `include_indirect_links`, `exclude_internal_backlinks`, `backlinks_status_type`, `rank_scale` `one_hundred`/`one_thousand`, Standard 0–1000); `bulk_pages_summary/live` (bis 1.000 Ziele, höchstens 100 verschiedene Domains je Request; Felder `url`, `rank`, `main_domain_rank`, `backlinks`, `backlinks_spam_score`, `broken_backlinks`, `referring_domains`, `referring_domains_nofollow`, `referring_main_domains`, `referring_ips`, `referring_subnets`, `referring_pages`, `first_seen`, `lost_date`, `referring_links_attributes` …); `bulk_new_lost_backlinks/live` und `bulk_new_lost_referring_domains/live` (bis 1.000 Ziele, `date_from` frühestens 1 Jahr zurück, Standard 1 Monat); weitere Bulk-Endpunkte `bulk_ranks`, `bulk_backlinks`, `bulk_referring_domains`, `bulk_spam_score`; Historie seit 2019 | Backlinks-Doku |
| SERP Google Organic | `POST /v3/serp/google/organic/live/advanced` bzw. `task_post` + `task_get/advanced`; `keyword` ≤ 700 Zeichen, `location_code`/`location_name`/`location_coordinate`, `language_code`, `device` (`desktop`/`mobile`), `os`, `depth` (Standard 10, max. 200; abgerechnet je 10 Ergebnisse), `load_async_ai_overview` (Endpunkt-Doku: Aufpreis pauschal 0,002 $, **erstattet**, wenn kein AI Overview vorhanden ist oder `asynchronous_ai_overview: false`), `calculate_rectangles` (Endpunkt-Doku: Aufpreis 0,002 $), `people_also_ask_click_depth` (1–4), `priority` (1 normal, 2 hoch); Antwort `item_types[]`, Items u. a. `organic` (`rank_group`, `rank_absolute`, `url`, `domain`), `ai_overview` (`asynchronous_ai_overview`, `items[]` vom Typ `ai_overview_element`, `references[]` mit `source`, `domain`, `url`, `title`, `text` auf Item- und Element-Ebene), `knowledge_graph` (darin u. a. `knowledge_graph_ai_overview_item` mit `asynchronous_ai_overview`, `items[]` vom Typ `ai_overview_element` mit eigenen `references[]` sowie `references[]` auf Item-Ebene), `featured_snippet`, `people_also_ask`, `video`, `images`, `top_stories`, `local_pack`; Suchoperatoren (`site:`, `inurl:`, `intitle:` usw.) kosten das 5-Fache | SERP-Doku |
| SERP Google AI Mode | `POST /v3/serp/google/ai_mode/live/advanced` (bzw. Task-Variante); Parameter `keyword` (≤ 700 Zeichen), Ort, Sprache, `device`, `os`, `calculate_rectangles`; kein `depth`; Antwort mit `ai_overview`-artigen Elementen und `references[]` (`source`, `domain`, `url`, `title`, `text`) auf mehreren Ebenen; Preis je Task: Standard 0,0012 $, Priority 0,0024 $, Live 0,004 $; abgerechnet wird je Request = eine AI-Mode-Seite (Preisseite AI Mode, FAQ; Q8 geklärt); `calculate_rectangles` +1 Basispreis | SERP-Doku AI Mode, Preisseite AI Mode |
| LLM Mentions | nur Live; Ausführungszeit „derzeit bis 120 s“; Endpunkte unter `/v3/ai_optimization/llm_mentions/`: `search_mentions`, `target_metrics`, `multi_target_metrics`, `top_mentioned_pages`, `top_mentioned_domains`, `top_mentioned_brands`, `top_mentioned_brand_categories`, `historical`, `timeseries_delta`, `timeseries_new_lost` sowie `*_lite`-Varianten; `target` = bis 10 Entitäten (`domain` ≤ 63 Zeichen ohne `https://`/`www.`, `include_subdomains` Standard `false`, `search_scope` `any`/`sources`/`search_results`; oder `keyword` ≤ 250 Zeichen mit `match_type` `word_match`/`partial_match`); `platform` `google` (AI Overview) oder `chat_gpt` – ohne Angabe liefern die Endpunkte beide Plattformen (für `historical` inzwischen belegt); nur bei `multi_target_metrics` nennt die Doku zugleich `google` als Standard; die Regel für OpenSpider steht in der Einleitung von Abschnitt F; **`chat_gpt` nur Location 2840 (USA) und Sprache `en`**; Marktliste kostenlos über `GET /v3/ai_optimization/llm_mentions/locations_and_languages` (je Land `available_languages[]` mit `language_code`, `available_platforms[]`, `responses_count`); `top_mentioned_pages`: `include_pages`/`exclude_pages` (exakte URL-Listen), `filters` ≤ 8 (Operatoren u. a. `like`/`ilike`/`match`), `order_by` ≤ 3, `limit` Standard 100 / max. 1.000, `offset` ≤ 1.000.000, Antwort je Item `page`, `total.{mentions, ai_search_volume}`, Aufschlüsselung `platform[]`/`location[]`/`language[]` mit `key`, `mentions`, `ai_search_volume`; `target_metrics` liefert seine Kennzahlen im Antwortfeld `aggregated_metrics`; `multi_target_metrics` liefert die Werte je Gruppe in `items[]` (je Eintrag `key`, `total.mentions`, `total.ai_search_volume`, Aufschlüsselung `platform[]`) und in `aggregated_metrics` die Gesamtsumme über alle Gruppen; `GET /v3/ai_optimization/llm_mentions/available_filters` ist kostenlos (verifiziert); Kennzahlen `mentions`, `ai_search_volume`; `historical` monatlich ab **2025-08-01**; Datenaktualisierungszyklus 60 Tage | AI-Optimization-Doku, Preisseite LLM Mentions |
| Preise | siehe Preiskatalog D4S-011 | Preisseiten |
| Mindestabnahme | siehe „Klärung 100-$-Mindestabnahme“ unten | |

### Klärung: 100-$-Mindestabnahme bei Backlinks und LLM Mentions

Befund am 13.10.2026:
1. Die Ankündigung „Pricing Update in DataForSEO APIs“ vom **01.07.2026** nennt für Backlinks API und LLM Mentions API die **Abschaffung der monatlichen 100-$-Verpflichtung**; beide laufen seitdem wie alle anderen APIs über Guthaben (pay-as-you-go). Gleichzeitig wurden Backlinks und Labs um ca. 20 % teurer.
2. Die aktuellen Preisseiten für Backlinks und LLM Mentions nennen keine Mindestabnahme und kein Abo; die allgemeine Preisseite nennt nur die Mindest-Aufladung von 50 $.
3. In `user_data` sind die Abo-Ablauffelder als veraltet markiert, weil es das Abo-Format nicht mehr gibt.
4. **Widerspruch:** Die Fehlercode-Tabelle der Doku beschreibt 40204 („Access denied … activate your subscription“) weiterhin mit dem Hinweis, die Backlinks API habe eine Mindestabnahme. Mehrere Drittseiten nennen die 100 $ noch als gültig; sie stammen erkennbar aus der Zeit vor Juli 2026.

Festlegung für OpenSpider: Backlinks und LLM Mentions werden **ohne** Abo-Annahme angebunden. Die App nennt nirgends eine Mindestabnahme. Antwortet die API dennoch mit 40204, wird das Modul mit Status `Subscription required` gestoppt (D4S-004) und ein neutraler Hinweis mit Link auf das DataForSEO-Dashboard gezeigt. Ob 40204 bei diesen APIs nach dem 01.07.2026 überhaupt noch vorkommt, ist **offene Frage Q1**. **Entscheidung (Fable-Empfehlung)**: pay-as-you-go beibehalten; 40204 stoppt nur das betroffene Modul; geprüft wird beim ersten Live-Test mit einem frischen Konto, eine weitere Maßnahme ist nicht vorgesehen.

---

## Testinfrastruktur dieses Bereichs

- **Mock-Server:** `os-testsite` stellt unter `/__mock/dataforseo/v3/…` alle hier genutzten Endpunkte bereit (Antworten aus `tests/fixtures/dfs-mock/<endpunkt>/<szenario>.json`, Platzhalter `{BASE}` wird durch die Basis-URL der Fixture-Site ersetzt). Die App wird über `OPENSPIDER_API_BASE_DATAFORSEO` (INTEG-019) umgeleitet; Sandbox-Modus über `OPENSPIDER_API_BASE_DATAFORSEO_SANDBOX`.
- **Steuerung:** Kontoprofile über den Login: `mock-good` (gültig, Guthaben 25,00 $), `mock-bad` (40100), `mock-unverified` (40104), `mock-low` (Guthaben 0,05 $; ab Überschreiten 40210), `mock-costlimit` (nach 5 kostenpflichtigen Requests 40203), `mock-paused` (40201). Endpunkt-Szenarien über `POST /__control/dfs/scenario {"endpoint": "...", "name": "..."}` oder Header `X-Scenario` (INTEG-019): `ok`, `empty` (Task 40102), `empty-items` (Task 20000, `items: []`, `total_count: 0`), `ratelimit-x2` (2 × 40202, dann ok), `simul-x2` (2 × 40209, dann ok), `remaining-2` (Header `X-RateLimit-Remaining: 2`), `subscription` (40204), `invalid-field-one` (40501, sobald ein bestimmtes Ziel `{BASE}/bad%20url` im Batch ist), `partial` (Bulk-Antwort ohne Eintrag für `{BASE}/j`), `partial-offset` (paginierter Endpunkt: Seite mit `offset` 0 ok, jede Folgeseite 50000), `slow-<s>` (Antwortverzögerung), `hang`, `5xx-always`, `price-changed` (`cost` um 50 % höher als Katalog), `cost-fixed-max` (jede Antwort meldet den Maximalpreis des Requests), `kg-aio` (SERP: AI Overview nur als `knowledge_graph_ai_overview_item` innerhalb `knowledge_graph`), `order-by-rejected` (LLM Mentions: Requests mit `order_by` → Task 40501, ohne `order_by` ok mit unsortierten Items), `sandbox-no-llm` (nur Sandbox-Route: alle LLM-Mentions-Endpunkte → 40400). Das Tageslimit von `mock-costlimit` ist über `POST /__control/dfs/user_data {"limits_day": <usd>, "statistics_day": <usd>}` steuerbar; `user_data` meldet die Werte in `money.limits.day` bzw. `money.statistics` (D4S-004).
- **Pflichtabdeckung:** Jeder genutzte Endpunkt – ausdrücklich auch alle LLM-Mentions-Endpunkte aus Abschnitt F (`target_metrics`, `multi_target_metrics`, `top_mentioned_pages`, `top_mentioned_domains`, `historical`, `search_mentions`, `locations_and_languages`) – unterstützt `ok`, `empty`, `empty-items`, `ratelimit-x2`, `simul-x2`, `subscription`, `5xx-always`, `hang` und `slow-<s>` (u. a. `slow-150` und `slow-190` für die 180-s-Grenze); paginierte Endpunkte (`ranked_keywords` mit Domain-Ziel, `top_mentioned_pages`, `search_mentions`) zusätzlich `partial-offset`. Der Mock bedient außerdem `GET …/llm_mentions/available_filters` in zwei Varianten (mit und ohne String-Feld `page`).
- **Basis-URL der Fixture-Site:** `{BASE}` = `https://www.dfs-site.test:<port>` – `os-testsite` liefert TLS mit einem Testzertifikat aus dem Test-Truststore (INTEG-016), der Testresolver von `os-fetch` löst `*.test` auf 127.0.0.1 auf. Damit erfüllen alle Fixture-URLs die URL-Form-Vorgaben der Labs-Endpunkte (`https://` und `www.`). Für Q4-Tests gibt es die Varianten `{BASE_NOWWW}` = `http://dfs-nowww.test:<port>` und `{BASE_HTTPS_NOWWW}` = `https://dfs-nowww.test:<port>` mit denselben Pfaden.
- **Protokoll:** `GET /__control/dfs/log` liefert alle empfangenen Requests (Zeitstempel, Pfad, Body, Login, gemeldete `cost`). Akzeptanzkriterien mit „Mock-Log“ werten dieses Protokoll aus.
- **Fixture-Site `tests/fixtures/dfs-site/`** (vom `os-testsite` unter `{BASE}` ausgeliefert):

  | Pfad | Crawl-Eigenschaft | Mock-Daten |
  |---|---|---|
  | `/` | 200, indexierbar, verlinkt `/a`–`/f`, `/d2`, `/h`, `/i`, `/j`, `/deep/1`, `/q?session=abc123` (nicht `/g`, nicht `/orphan`) | Labs: Task 40102 (keine Daten) |
  | `/a` | 200, indexierbar, `<meta name="main-keyword" content="widget shop">`; zusätzlich von `/e` und `/f` verlinkt (3 Unique Inlinks) | Labs: 3 Keywords – `widget shop` Pos. 2, Vol. 1.000, ETV 250,5; `buy widgets` Pos. 5, Vol. 500, ETV 40,0; `widgets` Pos. 12, Vol. 8.000, ETV 30,0; `metrics.organic.etv` 320,5, `pos_1`=0, `pos_2_3`=1, `pos_4_10`=1. Backlinks: 120 Backlinks, 30 RD, 6 RD nofollow, Rank 45, neu 7 / verloren 3 |
  | `/b` | 200, `noindex` | Labs: `old widget` Pos. 4, Vol. 300, ETV 25,0. Backlinks: 15 / 5 RD |
  | `/c` | 404, von `/` verlinkt | Backlinks: 40 / 12 RD |
  | `/d` | 301 → `/a` | Backlinks: 8 / 3 RD |
  | `/d2` | 301 → `/c` (404), von `/` verlinkt | Backlinks: 4 / 2 RD |
  | `/e` | 200, indexierbar; zusätzlich von `/i` verlinkt (2 Unique Inlinks) | Labs: `blue widgets` Pos. 3, Vol. 2.000, ETV 180,0 |
  | `/f` | 200, indexierbar | Labs: `blue widgets` Pos. 8, Vol. 2.000, ETV 40,0 |
  | `/g` | 200, indexierbar, genau 1 interner Inlink (von `/deep/3`), Crawl-Tiefe 4 | Labs: `widget guide` Pos. 1, Vol. 5.000, ETV 1.500,0. Backlinks: 0 |
  | `/h` | 200, Canonical auf `/a` | Labs: Task 20000, leere `items`, `total_count` 0 |
  | `/i` | 200, indexierbar | Labs: `what is a widget` Pos. 6, Vol. 900, ETV 30,0. SERP: im AI Overview zitiert. LLM: 9 Erwähnungen |
  | `/j` | 200, indexierbar | Labs: Task 20000, leere `items`, `total_count` 0; Backlinks: Eintrag mit allen Werten 0 (im Szenario `partial` fehlt der Eintrag); kein Haupt-Keyword; LLM: keine Erwähnung |
  | `/q?session=abc123` | 200, indexierbar, Query-String | wird standardmäßig nie an DataForSEO gesendet (D4S-010) |
  | `/deep/1`–`/deep/3` | Kette `/ → /deep/1 → /deep/2 → /deep/3 → /g` | Labs: Task 20000, leere `items`, `total_count` 0; sonst keine Daten |
  | `/orphan` | nicht verlinkt, 200 | Labs Relevant Pages: ETV 75,0; LLM: 2 Erwähnungen |

  Domain-Ebene (Backlinks `summary`): 5.000.000.000 Backlinks (64-Bit-Prüfung), 1.500 RD, Rank 52. SERP-Mock: Keyword `widget shop` → organisch Pos. 1 `https://competitor.test/shop`, Pos. 2 `{BASE}/a`; Features `ai_overview` (Referenzen `https://competitor.test/shop`, `{BASE}/i#:~:text=widget`), `featured_snippet`, `people_also_ask`, `local_pack`. Keyword `blue widgets` → Pos. 3 `{BASE}/e`, Pos. 8 `{BASE}/f`, kein AI Overview. Keyword `what is a widget` → Pos. 6 `{BASE}/i`, AI Overview mit Referenz `{BASE}/i`; im Szenario `kg-aio` fehlt das Top-Level-Item `ai_overview`, stattdessen enthält `knowledge_graph.items[]` ein `knowledge_graph_ai_overview_item` mit Referenz `{BASE}/i` auf Item-Ebene. Keyword `site:competitor.test widgets` (Operator) ist im Mock vorhanden, wird aber standardmäßig nie angefragt (D4S-040). AI-Mode-Mock: Keyword `what is a widget` mit Referenz `{BASE}/i`. LLM-Mock (Erwähnungen Google AI Overview / ChatGPT; AI-Suchvolumen): eigene Domain 42 / 17 (12.000), `competitor.test` 60 / 20 (20.000), `rival.test` 10 / 3 (3.000); `multi_target_metrics` liefert diese Werte je Plattform-Request als `items[]` (`key` = Domain, `total.mentions`) und in `aggregated_metrics` die Gesamtsumme aller Gruppen (Google 112, ChatGPT 40). LLM-Marktliste (`locations_and_languages`): USA (2840) mit `en` [`google`, `chat_gpt`], Deutschland (2276) mit `de` [`google`], Österreich (2040) **nicht** enthalten.
- **Keine echten API-Aufrufe in CI.** Optionale Live-Tests gegen die Sandbox laufen nur mit `OPENSPIDER_LIVE_API_TESTS=1` und Zugangsdaten aus Umgebungsvariablen.

---

## A Verbindung & Infrastruktur

### D4S-001 Verbindung und Zugangsdaten
- **Priorität:** P0
- **Beschreibung:** Neuer Eintrag „DataForSEO“ unter „Daten-APIs“ im API-Zugriffsdialog (INTEG-001; mit den fünf KI-Anbietern aus M4 der zwölfte Eintrag, übernommen gemäß Änderungsbedarf X1) mit den Reitern „Konto“, „Module“, „Kosten“ und „Erweitert“. Der Nutzer gibt API-Login (E-Mail) und API-Passwort ein; beide werden als ein Eintrag im OS-Schlüsselbund abgelegt, nach dem gemeinsamen Schema aus INTEG-002 (führend, **Entscheidung (Fable-Empfehlung)**): Dienst `OpenSpider`, Konto `dataforseo`; das gespeicherte Geheimnis enthält Login und API-Passwort, der Login ist nicht der Kontoname. „Verbinden“ führt genau einen kostenlosen Test-Request an `user_data` aus. Der Dialog weist darauf hin, dass das API-Passwort nicht das Konto-Passwort ist und im DataForSEO-Dashboard unter „API Access“ zu finden ist.
- **Eingaben/Konfiguration:** Felder „API-Login“, „API-Passwort“ (maskiert), „Zugangsdaten merken“ (Standard an), Schaltflächen „Verbinden“/„Trennen“, Umschalter „Sandbox-Modus“ (D4S-003), Checkboxen je Modul (Labs, Backlinks, SERP, LLM Mentions; Standard alle aus).
- **Ausgabe:** Status gemäß INTEG-001: `Verbunden (<login>)`, `Verbunden (<login>, Sandbox)`, `Fehler: <Meldung>`.
- **Akzeptanzkriterien:**
  - [ ] Login `mock-good` → genau 1 Request `GET /v3/appendix/user_data` (Mock-Log) mit Header `Authorization: Basic <base64("mock-good:<passwort>")>`; Status `Verbunden (mock-good)`.
  - [ ] Login `mock-bad` → Status `Fehler: 40100 Zugangsdaten ungültig – API-Login und API-Passwort aus dem DataForSEO-Dashboard verwenden`; kein weiterer Request.
  - [ ] Login `mock-unverified` → `Fehler: 40104 Konto noch nicht verifiziert`; `mock-paused` → `Fehler: 40201 Konto vorübergehend gesperrt – Support kontaktieren`.
  - [ ] Zugangsdaten werden nie als URL-Parameter gesendet (Mock-Log: keine Query-Strings mit Login oder Passwort) und erscheinen in keiner Datei, keinem Log und keiner Crawl-Datei (Volltextsuche nach dem Testpasswort `dfs-TEST-001` → 0 Treffer).
  - [ ] Leeres Login- oder Passwortfeld → „Verbinden“ ist deaktiviert.
  - [ ] Der API-Zugriffsdialog enthält nach Einbau zwölf Einträge (sechs aus Release 1.0, fünf KI-Anbieter aus M4, DataForSEO); der UI-Test aus INTEG-001 verbindet alle zwölf (DataForSEO gegen `mock-good`).
  - [ ] Der Schlüsselbund-Eintrag liegt unter Dienst `OpenSpider`, Konto `dataforseo` (Mock-Keychain); Login und Passwort stehen nur im Geheimnis. Ein Wechsel des Logins überschreibt denselben Eintrag, es entsteht kein zweiter; nach Neustart zeigt der Status `Verbunden (<neuer Login>)`.
  - [ ] Ist beim Crawlstart mindestens ein Modul aktiv, DataForSEO aber nicht verbunden (fehlende Zugangsdaten), startet der Crawl trotzdem; es erscheint eine nicht blockierende Meldung „DataForSEO nicht verbunden – Module werden übersprungen“, und alle betroffenen URLs erhalten `DFS <Modul> Status = Not connected`.
  - [ ] Ein Konfigurationsprofil enthält Modulauswahl und Optionen, aber weder Login-Passwort noch Basic-Auth-Header (grep auf Profil-Datei).
- **Testfixture:** Mock-Kontoprofile `mock-good`, `mock-bad`, `mock-unverified`, `mock-paused`; Mock-Keychain-Backend; `tests/fixtures/dfs-site/`.

### D4S-002 Kontostand, Limits und Live-Preise
- **Priorität:** P0
- **Beschreibung:** Der Reiter „Konto“ zeigt nach dem Verbinden und auf Klick „Aktualisieren“ die Daten aus `user_data`: Guthaben, insgesamt eingezahlter Betrag, im Konto gesetzte Tages- und Minutenlimits (Kosten und Aufrufe) mit heutigem Verbrauch. Die Antwort enthält zusätzlich die aktuelle Preisliste (`price`); sie aktualisiert den Preiskatalog (D4S-011). Der Kontostand wird nach jedem Lauf und höchstens alle 60 s während eines Laufs neu abgefragt (kostenlos).
- **Eingaben/Konfiguration:** Schaltfläche „Aktualisieren“, Link „Guthaben aufladen“ (öffnet das DataForSEO-Dashboard im Systembrowser).
- **Ausgabe:** `Guthaben: 25,0000 $ · Eingezahlt: 50,0000 $ · Tageslimit Kosten: 10,0000 $ (heute verbraucht 1,2300 $)`; Zeitpunkt der letzten Abfrage.
- **Akzeptanzkriterien:**
  - [ ] Mock `money.balance = 25.0`, `money.total = 50.0` → Anzeige `Guthaben: 25,0000 $`, `Eingezahlt: 50,0000 $`.
  - [ ] Sind in `money.limits.day` Werte gesetzt, werden sie mit dem heutigen Verbrauch aus `money.statistics` angezeigt; fehlen sie, steht „kein Tageslimit gesetzt“.
  - [ ] Während eines Laufs mit 200 Requests in 30 s wird `user_data` höchstens einmal je 60 s abgefragt (Mock-Log).
  - [ ] Die Felder `backlinks_subscription_expiry_date` und `llm_mentions_subscription_expiry_date` werden nicht ausgewertet und nicht angezeigt.
  - [ ] Ein Fehler beim Abfragen von `user_data` während eines Laufs (z. B. 50000) stoppt keine Module; die Anzeige behält den letzten Wert mit Zeitstempel und Hinweis „veraltet“.
- **Testfixture:** Mock `user_data` mit gesetzten und ohne Tageslimits.

### D4S-003 Sandbox-Modus
- **Priorität:** P0
- **Beschreibung:** Mit „Sandbox-Modus“ gehen alle Requests an `https://sandbox.dataforseo.com/v3/` statt an die Produktiv-API. Die Sandbox ist laut Anbieter kostenlos und liefert Dummy-Daten mit realer Struktur; der Modus dient dem Test einer Konfiguration (Module, Markt, Budget, Exporte) ohne Kosten. Sandbox-Daten sind überall als solche erkennbar und werden nie mit Echtdaten vermischt. Die Sandbox nutzt dieselben Zugangsdaten wie die Produktiv-API (Q11). Liefert die Sandbox für LLM-Mentions-Endpunkte 40400 oder 40501, zeigt das LLM-Modul im Sandbox-Modus den Banner „Sandbox liefert keine LLM-Mentions-Daten“; betroffene URLs erhalten `Not requested`, es entsteht kein Fehlerstatus, und im laufenden Sandbox-Lauf gehen keine weiteren LLM-Mentions-Requests hinaus (**Entscheidung (Fable-Empfehlung)**, Q11).
- **Eingaben/Konfiguration:** Umschalter „Sandbox-Modus“ im Reiter „Konto“ (Standard aus); CLI `--dfs-sandbox` (D4S-073).
- **Ausgabe:** Herkunftsspalte `DFS <Modul> Source = Sandbox`; Banner in allen DFS-Tabs „Sandbox – Testdaten ohne Aussagekraft“; Kostenprotokoll mit Betrag 0 und Kennzeichen `sandbox`.
- **Akzeptanzkriterien:**
  - [ ] Bei aktivem Sandbox-Modus gehen 100 % der DataForSEO-Requests an die Sandbox-Basis-URL (Mock-Log der Produktiv-Route: 0 Requests).
  - [ ] Budget und Guthabenprüfung blockieren im Sandbox-Modus nicht (Budget 0,00 $ → Lauf führt trotzdem alle Requests aus).
  - [ ] Sandbox-Ergebnisse werden im Cache (D4S-015) unter einem eigenen Umgebungsschlüssel abgelegt; nach Abschalten des Sandbox-Modus wird kein Sandbox-Eintrag als Cache-Treffer verwendet (Mock-Log Produktiv-Route: Requests für alle URLs).
  - [ ] Exporte enthalten die Herkunftsspalte; im Excel-Export trägt jedes Blatt mit DFS-Daten zusätzlich die Kopfzeilen-Notiz „Sandbox-Daten“.
  - [ ] Ein Umschalten während eines Laufs ist gesperrt (Schalter deaktiviert mit Tooltip).
  - [ ] Szenario `sandbox-no-llm` mit aktiven Modulen Labs und LLM Mentions → genau 1 LLM-Mentions-Request an die Sandbox (40400), danach keiner mehr (Mock-Log); Banner „Sandbox liefert keine LLM-Mentions-Daten“; alle URLs `DFS LLM Status = Not requested`, kein `Error: …`, Filter `Errors` im Tab `LLM Mentions` leer; Labs liefert Sandbox-Daten.
- **Testfixture:** zwei Mock-Routen (Produktiv und Sandbox) mit unterschiedlichen Werten; Szenario `sandbox-no-llm`; `tests/fixtures/dfs-site/`.

### D4S-004 Antwortauswertung und Fehlercodes
- **Priorität:** P0
- **Beschreibung:** DataForSEO meldet Fehler überwiegend mit HTTP 200 und einem `status_code` im Body – auf oberster Ebene und je Task. Ein Request gilt nur als erfolgreich, wenn HTTP 2xx, oberster `status_code` 20000 und Task-`status_code` 20000 (Live) bzw. 20100 (Task angelegt) sind. Jeder Code wird einer festen Reaktion zugeordnet:

  | Code | Reaktion | Status je betroffener URL |
  |---|---|---|
  | 40102 | kein Fehler, keine Wiederholung | `No data` (Kennzahlen leer) |
  | 20000 mit leerem `items` / `total_count` 0 | kein Fehler, keine Wiederholung | `Success` (Zählspalten `0`, siehe Regel „keine Daten“ vs. „null Treffer“) |
  | Ziel fehlt in Bulk-Antwort | kein Fehler, keine Wiederholung (D4S-006) | `No data` |
  | 40202, 40209, 50000, HTTP 5xx, Netzwerkfehler | Wiederholung nach INTEG-005; bei 40209 zusätzlich D4S-005 | nach Ausschöpfen `Error: <Code> <Meldung>` |
  | 40100, 40104, 40201 | DataForSEO-Integration komplett stoppen, Verbindungsstatus `Fehler: …` | `Error: <Code> <Meldung>` für bereits gesendete, `Not requested` für offene |
  | 40200, 40210 | alle DFS-Module stoppen, Kontostand neu abfragen | `Insufficient funds` |
  | 40203 | alle DFS-Module stoppen bis zum nächsten Kalendertag (UTC) oder bis `user_data` wieder Luft meldet (siehe „Freigabe nach 40203“; Rücksetzzeitpunkt UTC-Mitternacht beibehalten, Q2) | `Quota exceeded` |
  | 40204 | nur das betroffene Modul stoppen | `Subscription required` |
  | 40501 | keine Wiederholung; bei Bulk-Requests Halbierung (D4S-006) | `Error: 40501 Invalid Field` |
  | 40401 bei `task_get` | Task als abgelaufen markieren | `Error: 40401 Task nicht mehr verfügbar` |
  | 40601, 40602 | Task weiter abwarten (D4S-008) | `Pending` |
  | 40006 | interner Programmfehler (Batchgröße > 100); Batch teilen und erneut senden, Log-Eintrag Level ERROR | – |

  Gestoppte Module unterbrechen nie den Crawl selbst.

  **Freigabe nach 40203 (Entscheidung (Fable-Empfehlung), Q2):** Vor jedem neuen Lauf und bei „DataForSEO-Daten anfordern“ (INTEG-008) fragt OpenSpider zuerst `user_data` ab (kostenlos) und vergleicht `money.limits.day` mit dem heutigen Verbrauch aus `money.statistics`. Liegt der Verbrauch unter dem Limit (z. B. weil der Nutzer das Limit im Dashboard angehoben hat) oder ist kein Tageslimit gesetzt, werden die gestoppten Module unabhängig von der Uhr freigegeben. Sonst bleibt die Sperre bis 00:00 UTC; schlägt die Abfrage fehl, gilt ebenfalls der UTC-Tageswechsel.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Statusspalten je Modul; Meldung im Fortschrittsbereich „DataForSEO gestoppt: <Grund>“.
- **Akzeptanzkriterien:**
  - [ ] Mock liefert HTTP 200 mit `tasks[0].status_code = 40501` → keine Wiederholung (Mock-Log: genau 1 Request), Status `Error: 40501 Invalid Field`.
  - [ ] Login `mock-low`: Nach dem Request, der 40210 liefert, sendet die App keinen weiteren kostenpflichtigen Request (Mock-Log); alle offenen URLs aller DFS-Module erhalten `Insufficient funds`; die Zahl gecrawlter URLs steigt weiter.
  - [ ] Login `mock-costlimit`: Nach dem 40203 folgt kein weiterer kostenpflichtiger Request bis Tageswechsel (Testuhr auf 23:59 UTC → nach Wechsel auf 00:00 UTC ist „DataForSEO-Daten anfordern“ (INTEG-008) wieder aktiv).
  - [ ] Login `mock-costlimit`, Testuhr 14:00 UTC: Nach dem 40203 hebt der Mock `money.limits.day` über dem Verbrauch an → beim nächsten „DataForSEO-Daten anfordern“ geht zuerst genau 1 Request an `user_data`, danach laufen die kostenpflichtigen Requests wieder (Mock-Log). Ohne Anhebung geht nur der `user_data`-Request hinaus, und die Module bleiben bis 00:00 UTC gesperrt.
  - [ ] Szenario `subscription` am Endpunkt Backlinks → nur Backlinks-URLs erhalten `Subscription required`; Labs läuft im selben Lauf weiter (Mock-Log zeigt weitere Labs-Requests).
  - [ ] Szenario `ratelimit-x2` → nach 2 Wiederholungen Erfolg, Status `Success`.
  - [ ] Szenario `empty` (40102) am Labs-Endpunkt → `DFS Labs Status No data`, `DFS Ranked Keywords` leer (nicht `0`); Wiederholung findet nicht statt (Mock-Log: 1 Request je URL).
  - [ ] Szenario `empty-items` (20000, `items: []`, `total_count: 0`) am Labs-Endpunkt → `DFS Labs Status Success`, `DFS Ranked Keywords 0`, `DFS Keywords Top 10 0`, `DFS Est. Traffic 0`, `DFS Top Keyword` leer; keine Wiederholung.
  - [ ] Für jeden Code der Tabelle existiert ein Unit-Test der Zuordnung Code → Reaktion → Status.
- **Testfixture:** Mock-Szenarien und Kontoprofile aus „Testinfrastruktur“; Testuhr.

### D4S-005 Ratenbegrenzung und Parallelität
- **Priorität:** P0
- **Beschreibung:** Die Limits gelten je DataForSEO-Konto, also auch für andere Werkzeuge desselben Kontos. OpenSpider bleibt daher standardmäßig deutlich darunter: höchstens 10 gleichzeitige Requests und höchstens 1.000 Requests je Minute und Endpunkt **(OS-Festlegung)**. Diese Werte **weichen bewusst von INTEG-005 ab** (dort Standard 5, Bereich 1–50): Der Standard 10 nutzt ein Drittel des API-Limits, damit typische Läufe mit Bulk- und Live-Endpunkten zügig fertig werden; die Obergrenze 30 entspricht dem dokumentierten API-Limit von 30 gleichzeitigen Anfragen je Konto – höhere Werte würden nur 40209 auslösen (Änderungsbedarf X3). Meldet die API 40209, setzt OpenSpider die Parallelität auf `max(1, ⌊aktuell / 2⌋)` (Abrunden; 10 → 5 → 2 → 1) und hebt sie nach 60 s ohne weiteren 40209 schrittweise (+1 je 60 s) bis zum eingestellten Wert an. `X-RateLimit-Remaining` wird ausgewertet: Bei Wert < 5 wartet die App bis zum nächsten Minutenfenster.
- **Eingaben/Konfiguration:** Reiter „Erweitert“: „Gleichzeitige Anfragen“ (1–30, Standard 10), „Anfragen je Minute und Endpunkt“ (1–2.000, Standard 1.000).
- **Ausgabe:** Tooltip der Fortschrittsanzeige: aktuelle Parallelität, Rest laut `X-RateLimit-Remaining`.
- **Akzeptanzkriterien:**
  - [ ] Mit Standardwerten hat der Mock nie mehr als 10 offene DFS-Requests gleichzeitig (Mock-Zeitstempel).
  - [ ] Bei Einstellung 60 Anfragen/Minute liegen in keinem 60-s-Fenster mehr als 60 Requests an denselben Endpunkt.
  - [ ] Szenario `simul-x2` mit Parallelität 10 → nach dem ersten 40209 maximal 5, nach dem zweiten maximal 2 offene Requests; 60 s nach dem letzten 40209 wieder 3 (Testuhr).
  - [ ] Parallelität 3 und ein 40209 → maximal 1 offener Request (⌊3/2⌋ = 1); Parallelität 1 und ein 40209 → bleibt 1.
  - [ ] Eingabe 31 bzw. 0 für „Gleichzeitige Anfragen“ wird abgelehnt; 30 wird akzeptiert.
  - [ ] Antwort-Header `X-RateLimit-Remaining: 2` → der nächste Request an diesen Endpunkt geht erst im nächsten Minutenfenster raus.
  - [ ] Werte außerhalb der Bereiche werden im Dialog abgelehnt.
- **Testfixture:** Mock mit Zeitstempel-Log, Szenario `simul-x2`, Header-Szenario `remaining-2`.

### D4S-006 Timeouts und Teilfehler
- **Priorität:** P0
- **Beschreibung:** Request-Timeouts je Modul: Labs, Backlinks und SERP-Live 120 s, LLM Mentions 180 s (laut Anbieter „bis 120 s“ Ausführungszeit), Verbindungsaufbau 10 s **(OS-Festlegung)**; ein Timeout gilt als vorübergehender Fehler (INTEG-005). Teilfehler werden so behandelt, dass möglichst viele URLs Daten erhalten und nichts doppelt bezahlt wird:
  1. **Fehlender Eintrag in einer Bulk-Antwort** (Ziel gesendet, Task 20000, aber kein Item für dieses Ziel zurück) → `No data` für dieses Ziel, keine Wiederholung. Davon zu unterscheiden ist ein vorhandenes Item mit Wert 0 bzw. leeren `items` eines Einzel-Tasks → `Success` mit `0` (Regel in den Konventionen).
  2. **Bulk-Task mit 40501** → der Batch wird halbiert und beide Hälften werden erneut gesendet, bis das ungültige Ziel isoliert ist (höchstens ⌈log₂(Batchgröße)⌉ Stufen); nur dieses erhält `Error: 40501 Invalid Field`.
  3. **Mehrere Tasks in einem Standard-POST, einzelne fehlerhaft** (`tasks_error > 0`) → nur die fehlerhaften Tasks werden nach D4S-004 behandelt.
  4. **Abbruch mitten im Lauf** (Absturz, Beenden) → bereits bezahlte Ergebnisse sind gespeichert (Commit je Antwort), offene Live-Requests gelten als nicht gesendet und werden beim Fortsetzen neu gestellt.
  5. **Paginierte Abrufe mit `offset`** (Domain-Abzug D4S-023, Seiten-Erwähnungen D4S-051): Jede Seite wird einzeln gecacht. Scheitert eine Folgeseite endgültig, bleiben die bereits geladenen Seiten erhalten; Werte, die erst durch die fehlende Seite feststünden, werden nicht als `0` gewertet (Regeln je Funktion). Neuanforderung lädt nur die fehlenden Seiten.
- **Eingaben/Konfiguration:** Reiter „Erweitert“: „Request-Timeout (s)“ je Modul, Bereich 10–600.
- **Ausgabe:** `Error: Timeout nach <n> s`.
- **Akzeptanzkriterien:**
  - [ ] Szenario `slow-130` am Labs-Endpunkt mit Standard-Timeout → Abbruch nach 120 s (±2 s), Wiederholung gemäß INTEG-005, nach 6 Versuchen Status `Error: Timeout nach 120 s`.
  - [ ] Szenario `slow-150` an LLM Mentions gelingt mit Standard-Timeout 180 s; `slow-190` → Abbruch nach 180 s (±2 s), nach Ausschöpfen der Wiederholungen `Error: Timeout nach 180 s`.
  - [ ] Szenario `partial-offset` am Domain-Abzug (D4S-023) mit 2.500 Keywords → Seite `offset` 0 gespeichert, Folgeseiten nach Wiederholungen endgültig fehlgeschlagen; „Fehlgeschlagene neu anfordern“ (bei Szenario `ok`) fragt nur `offset` 1000 und 2000 ab (Mock-Log).
  - [ ] Szenario `partial`: Bulk-Backlinks für 10 URLs, Antwort ohne `{BASE}/j` → 9 URLs `Success`, `/j` `No data`; Mock-Log: genau 1 Request.
  - [ ] Szenario `invalid-field-one` mit 8 Zielen, davon eines `{BASE}/bad%20url` → höchstens 1 + 2·3 = 7 Requests; 7 URLs `Success`, 1 URL `Error: 40501 Invalid Field`.
  - [ ] Prozess wird nach 30 von 60 Labs-Antworten hart beendet; nach „Fortsetzen“ werden nur die fehlenden 30 URLs angefragt (Mock-Log), die 30 gespeicherten bleiben unverändert.
- **Testfixture:** Mock-Szenarien `slow-<s>`, `partial`, `invalid-field-one`; Kill-Test analog PERF-005.

### D4S-007 Markt-Auswahl (Location und Language)
- **Priorität:** P0
- **Beschreibung:** Je Crawl gilt ein **Markt** aus Land (`location_code`) und Sprache (`language_code`) für alle Module. Die Auswahlliste stammt aus den kostenlosen Endpunkten `GET /v3/dataforseo_labs/locations_and_languages` (Länder mit verfügbaren Sprachen) und `GET /v3/serp/google/locations` bzw. `/languages`; sie wird im App-Datenverzeichnis (nicht in der Crawl-Datei) zwischengespeichert und alle 30 Tage erneuert. Ohne Verbindung dient ein mitgelieferter Schnappschuss `resources/dataforseo/locations.json` als Fallback. Labs kennt nur Länder; die SERP API erlaubt zusätzlich feinere Orte (D4S-045). Für LLM Mentions gilt eine eigene Marktliste aus dem kostenlosen Endpunkt `GET /v3/ai_optimization/llm_mentions/locations_and_languages` (je Land und Sprache die verfügbaren Plattformen `available_platforms`); sie wird wie die übrigen Listen zwischengespeichert und mit einem Schnappschuss `resources/dataforseo/llm_locations.json` abgesichert. Vor jedem LLM-Request prüft OpenSpider den Markt gegen diese Liste: Ist die Kombination Land/Sprache nicht enthalten, wird nichts gesendet und das LLM-Modul erhält `Skipped (market unsupported)`; eine Plattform wird nur angefragt, wenn sie in `available_platforms` steht (laut Anbieter derzeit `chat_gpt` nur für USA/Englisch).
- **Eingaben/Konfiguration:** Durchsuchbares Dropdown „Land“ und davon abhängiges Dropdown „Sprache“ (nur Sprachen, die für das Land verfügbar sind). Vorbelegung beim ersten Öffnen aus der Region des Betriebssystems (z. B. `de-DE` → Deutschland/Deutsch) **(OS-Festlegung)**; danach die zuletzt gewählte Kombination.
- **Ausgabe:** Spalten `DFS Location` (Name) und `DFS Language` (ISO 639-1) in jedem DFS-Tab; Tab-Hinweis „Markt: Deutschland / de“.
- **Akzeptanzkriterien:**
  - [ ] Auswahl „Deutschland“ / „German“ → alle Labs-, SERP- und LLM-Mentions-Requests enthalten `location_code: 2276` und `language_code: "de"` (Mock-Log); Backlinks-Requests enthalten keinen Marktparameter.
  - [ ] Eine Sprache, die laut Liste für das Land nicht verfügbar ist, ist nicht wählbar.
  - [ ] Ohne Netzwerk und ohne Cache wird der Schnappschuss verwendet; der Dialog zeigt „Liste vom <Datum des Schnappschusses>“.
  - [ ] Markt Deutschland/de (LLM-Marktliste: nur `google`) → für LLM Mentions wird keine Anfrage mit `platform: "chat_gpt"` gestellt; die ChatGPT-Spalten bleiben leer, `DFS LLM Status` bleibt vom Google-Teil bestimmt, Tooltip „ChatGPT-Daten für diesen Markt nicht verfügbar“.
  - [ ] Markt Österreich/de (nicht in der LLM-Marktliste) → 0 LLM-Requests (Mock-Log), alle LLM-Funktionen melden `Skipped (market unsupported)`; Labs und SERP laufen normal mit `location_code: 2040`.
  - [ ] Die LLM-Marktliste wird mit genau 1 `GET`-Request an `…/llm_mentions/locations_and_languages` geladen, verursacht `cost` 0 und keinen Eintrag mit Betrag > 0 im Kostenprotokoll.
  - [ ] Eine Änderung des Markts nach einem Abruf löst den Dialog aus INTEG-010 aus.
- **Testfixture:** Mock der Location-Endpunkte von Labs und SERP (3 Länder: USA mit `en`, Deutschland mit `de` und `en`, Österreich mit `de`) und des LLM-Location-Endpunkts (siehe Testinfrastruktur); Schnappschussdateien.

### D4S-008 Standard-Task-Ausführung mit Wiederaufnahme
- **Priorität:** P1
- **Beschreibung:** Für Endpunkte mit Standard-Modus (in diesem Bereich: SERP API) stellt OpenSpider Tasks gebündelt (bis 100 je POST) und holt Ergebnisse durch Abfrage von `tasks_ready` (höchstens 20 Aufrufe/Minute; OpenSpider fragt alle 30 s) und anschließend `task_get/advanced/{id}`. Postback- und Pingback-URLs werden nicht verwendet, weil eine Desktop-App keine öffentlich erreichbare Adresse hat. Jede gestellte Task-ID wird vor dem Absenden in der Crawl-Datei (Tabelle `dfs_tasks`: ID, Endpunkt, Ziel, Parameter-Hash, gestellt um, Status) gespeichert, damit nach Neustart nichts doppelt bezahlt wird.
- **Eingaben/Konfiguration:** „Maximale Wartezeit auf Task-Ergebnisse“ (Standard 60 min, Bereich 5–1.440).
- **Ausgabe:** Status `Pending` bis zum Eintreffen; Fortschritt „SERP: 120/400 Tasks fertig“.
- **Akzeptanzkriterien:**
  - [ ] 250 Keywords im Standard-Modus → genau 3 `task_post`-Requests (100/100/50) (Mock-Log).
  - [ ] `tasks_ready` wird in keinem 60-s-Fenster öfter als 2-mal aufgerufen.
  - [ ] App wird nach dem Stellen von 50 Tasks beendet; nach Neustart und „Fortsetzen“ folgen 0 neue `task_post` für diese 50 Keywords, die Ergebnisse werden über `tasks_ready`/`task_get` abgeholt.
  - [ ] Sind Tasks älter als 3 Tage und nicht in `tasks_ready`, werden sie direkt per `task_get/{id}` abgefragt; liefert das 40401, ist der Status `Error: 40401 Task nicht mehr verfügbar`, und „Neu anfordern“ stellt einen neuen Task.
  - [ ] Nach Ablauf der maximalen Wartezeit gilt der Crawl als vollständig (Exporte laufen), offene Tasks behalten `Pending`; „DataForSEO-Daten anfordern“ (INTEG-008) holt sie später ohne Neukauf ab.
- **Testfixture:** Mock `task_post`/`tasks_ready`/`task_get` mit verzögerter Fertigstellung (Task wird nach 2 `tasks_ready`-Abfragen fertig); Testuhr für die 3-Tage-Grenze.

### D4S-009 Mock-Server und Testbarkeit
- **Priorität:** P0
- **Beschreibung:** Für jeden in diesem Dokument genutzten Endpunkt stellt `os-testsite` eine Mock-Route bereit (siehe „Testinfrastruktur“), inklusive Kontoprofilen, Szenarien und Request-Protokoll. Die Basis-URLs von Produktiv- und Sandbox-API sind über `OPENSPIDER_API_BASE_DATAFORSEO` und `OPENSPIDER_API_BASE_DATAFORSEO_SANDBOX` überschreibbar – wie bei INTEG-019 nur in Debug-/Testbuilds oder mit `--allow-api-override`. Die Mock-Antworten sind aus der öffentlichen Doku abgeleitete, selbst erstellte Beispiele (keine kopierten Kundendaten).
- **Eingaben/Konfiguration:** Umgebungsvariablen; Steuer-Endpunkte `/__control/dfs/…`.
- **Ausgabe:** keine.
- **Akzeptanzkriterien:**
  - [ ] Für jeden genutzten Endpunkt existieren die Szenarien der „Pflichtabdeckung“ (Testinfrastruktur), mindestens `ok`, `empty`, `empty-items`, ein 4xx-Szenario (40501 oder 40204), `ratelimit-x2`, `hang` und `slow-<s>`; ein Test iteriert über alle Endpunkte aus Anhang B und schlägt fehl, wenn eines fehlt.
  - [ ] Die INTEG-019-Pflichtszenarien (Erfolg, Leer, 4xx, Drossel) sind über den Header `X-Scenario` mit `ok`, `empty`, `invalid-field-one`, `ratelimit-x2` erreichbar (Änderungsbedarf X2).
  - [ ] In Release-Builds ohne Flag werden die Variablen ignoriert.
  - [ ] Alle Akzeptanztests dieses Dokuments laufen offline; ein Netzwerk-Allowlist-Test (INTEG-018) schlägt fehl, sobald ein Request an `*.dataforseo.com` geht.
  - [ ] Ein Schema-Test prüft jede Mock-Antwort gegen die in `os-integrations` definierten Antworttypen (Deserialisierung ohne unbekannte Pflichtfelder).
- **Testfixture:** `tests/fixtures/dfs-mock/`, `tests/fixtures/dfs-site/`.

### D4S-010 Übertragene Daten und Datenschutz-Hinweis
- **Priorität:** P0
- **Beschreibung:** An DataForSEO werden ausschließlich URLs, Domains, Keywords und Marktparameter gesendet, nie Seiteninhalte, HTML oder Crawl-Daten. Auch URLs und Keywords können personenbezogene oder geheime Daten enthalten; daher gelten vor dem Senden folgende Regeln **(OS-Festlegung)**:
  1. **Query-Strings:** URLs mit Query-String (z. B. Session-IDs, Tokens, E-Mail-Adressen in Parametern) werden standardmäßig **nicht** gesendet und erhalten `Skipped (query string)`. Ein Entfernen der Parameter vor dem Senden findet bewusst nicht statt, weil die Ergebnisse dann einer anderen URL gehören würden. Optional kann eine Allowlist unbedenklicher Parameternamen gepflegt werden (z. B. `page`, `lang`); URLs, deren Query ausschließlich aus erlaubten Parametern besteht, werden unverändert gesendet. Ein ausdrücklicher Schalter „Alle URLs mit Query-String senden“ ist möglich und zeigt eine Warnung.
  2. **Zugangsdaten in URLs:** URLs mit Userinfo (`user:pass@host`) werden nie gesendet (`Error: URL enthält Zugangsdaten`).
  3. **Keywords** aus Custom Extraction (D4S-040) sind beliebiger Seitentext; Keywords mit E-Mail-Adresse oder URL mit Query-String werden verworfen (`Skipped (invalid keyword)`).
  4. **Gespeicherte Fremdinhalte:** Antworten werden als Rohdaten in `dfs_cache` und in den Modultabellen der Crawl-Datei gespeichert. Erwähnungs-Details (D4S-055) enthalten vollständige LLM-Fragen und -Antworten, also Inhalte Dritter; sie bleiben dauerhaft in der Crawl-Datei, bis der Nutzer sie löscht („Erwähnungs-Details löschen“ im Reiter „Kosten“ → Cache), und reisen mit, wenn die Crawl-Datei weitergegeben wird.
  Vor dem ersten kostenpflichtigen Request je Installation erscheint einmalig ein Hinweis, dass URLs, Domains und Keywords (mit ihren Quellen: Crawl, Liste, Custom Extraction, Labs) an DataForSEO übertragen werden, Kosten verursachen und Antworten einschließlich Fremdinhalten in der Crawl-Datei gespeichert werden (INTEG-018 gilt sinngemäß).
- **Eingaben/Konfiguration:** einmalige Bestätigung (im Sandbox-Modus ebenfalls erforderlich); Reiter „Erweitert“: „URLs mit Query-String“ ∈ {`Nicht senden` (Standard), `Nur erlaubte Parameter`, `Alle senden`}, Liste „Erlaubte Parameter“ (Standard leer).
- **Ausgabe:** Status `Skipped (query string)`, `Skipped (invalid keyword)`, `Error: URL enthält Zugangsdaten`.
- **Akzeptanzkriterien:**
  - [ ] Ohne Bestätigung wird kein Request außer `user_data` und den Location-Listen gesendet.
  - [ ] Kein Request-Body im Mock-Log enthält Text aus den Fixture-Seiten (Prüfung auf den eindeutigen Seitentext `DFS-CONTENT-MARKER`).
  - [ ] Standard-Crawl der Fixture-Site mit allen Modulen → kein Request-Body im Mock-Log enthält `?session=` bzw. `abc123`; `/q?session=abc123` hat in allen URL-bezogenen Modulen `Skipped (query string)`, und `dfs_cache` enthält keinen Schlüssel mit `session=` (SQL-Prüfung).
  - [ ] Einstellung `Nur erlaubte Parameter` mit Allowlist `page` → `{BASE}/a?page=2` wird gesendet, `{BASE}/q?session=abc123` nicht.
  - [ ] URL `https://user:pw@www.dfs-site.test/a` (Listenmodus) → kein Request, Status `Error: URL enthält Zugangsdaten`.
  - [ ] Extraktor liefert für eine Seite `kontakt max@example.com` → `Skipped (invalid keyword)`, kein SERP-Request mit `@` im Body.
  - [ ] Der Einwilligungshinweis nennt Keyword-Quellen und die Speicherung von Fremdinhalten (UI-Text-Test); „Erwähnungs-Details löschen“ entfernt alle `search_mentions`-Einträge aus `dfs_cache` und `dfs_llm` (SQL-Zählung 0).
- **Testfixture:** `tests/fixtures/dfs-site/` (jede Seite enthält `DFS-CONTENT-MARKER`; Seite `/q?session=abc123`; Variante mit `?page=2`), Extraktor-Variante mit E-Mail-Adresse, Mock-Log.

---

## B Kostenkontrolle & Cache

### D4S-011 Preiskatalog mit Stand-Datum
- **Priorität:** P0
- **Beschreibung:** Alle Kostenrechnungen nutzen einen versionierten Preiskatalog `resources/dataforseo/pricing.json` (Felder je Endpunkt: `path`, `mode` live/standard/priority, `cost_per_task`, `cost_per_row`, `rows_per_task_max`, `surcharges[]` mit `name`, `per_task_low`, `per_task_high`, `refundable_if_absent`, Multiplikatoren (z. B. `operator_factor`), `user_data_key` = [`api_name`, `func_type`, `func_name`], `valid_from`, `source_url`, `checked_at`). Beim Verbinden werden die Preise aus `user_data.price` gelesen und überschreiben den mitgelieferten Katalog für dieses Konto **teilweise** (kontospezifische Rabatte werden so berücksichtigt). Weil `user_data.price` je Funktion und Priorität nur **einen** Basiswert mit `cost_type` liefert und Zuschläge ausdrücklich nicht enthält, gilt folgende Zuordnung:

  | `user_data.price…` | überschreibt im Katalog | bleibt aus dem mitgelieferten Katalog |
  |---|---|---|
  | `cost_type = per_request` | `cost_per_task` des Eintrags mit passendem `user_data_key` und Modus (`priority_normal` → live bzw. standard laut `func_type`, `priority_high` → priority) | `cost_per_row`, `surcharges[]`, Multiplikatoren, `rows_per_task_max` |
  | `cost_type = per_result` | `cost_per_row` | `cost_per_task`, `surcharges[]`, Multiplikatoren, `rows_per_task_max` |
  | Eintrag ohne passenden `user_data_key` bzw. unbekannte Schlüssel | – (ignoriert, Log-Eintrag DEBUG) | alles |

  Die genaue Belegung von `func_type`/`func_name` je Endpunkt wird bei der Umsetzung anhand einer echten `user_data`-Antwort in `user_data_key` eingetragen **(Annahme, zu verifizieren)**. Je Katalogfeld wird die Herkunft (`account` | `catalog` | `observed`) geführt. Mitgelieferter Stand (geprüft am **13.10.2026**, Preise gültig seit der Preisänderung vom 01.07.2026):

  | Endpunkt-Gruppe | Preis je Task | Preis je Zeile/Item | Anmerkung |
  |---|---|---|---|
  | DataForSEO Labs (alle Endpunkte, nur Live) | 0,012 $ | 0,00012 $ | `include_clickstream_data` verdoppelt den Preis |
  | Backlinks `/backlinks/*` (nur Live) | 0,024 $ | 0,000036 $ | bis 1.000 Zeilen je Request |
  | SERP Google Organic Standard | 0,0006 $ je SERP-Seite (10 Ergebnisse) | – | Priority 0,0012 $, Live 0,002 $ |
  | SERP-Zuschläge (je Task, unabhängig von `depth`) | `load_async_ai_overview`: `per_task_low` = 1 Basispreis (Preisseite), `per_task_high` = max(1 Basispreis; 0,002 $) (Endpunkt-Doku), `refundable_if_absent` = ja; `calculate_rectangles`: dieselbe Doppel-Lesart (Endpunkt-Doku 0,002 $, Preisseiten +1 Basispreis), nicht erstattbar; PAA-Klick +0,00015 $ je Klick; Suchoperatoren `operator_factor` = 5 auf den gesamten Task-Preis | – | Live: beide Lesarten ergeben 0,002 $; Standard/Priority weichen ab (offene Frage Q3; Golden-Test als Release-Gate M5) |
  | SERP Google AI Mode (P2, D4S-048) | Standard 0,0012 $, Priority 0,0024 $, Live 0,004 $ je Task | – | `calculate_rectangles` +1 Basispreis |
  | LLM Mentions `/ai_optimization/llm_mentions/*` (nur Live, inkl. `*_lite`) | 0,10 $ | 0,001 $ | |
  | `user_data`, Location-Listen, `available_filters`, `tasks_ready`, `task_get`, Sandbox | 0 $ | 0 $ | |

  Die Mindest-Aufladung beträgt 50 $; eine Mindestabnahme gibt es nach aktuellem Stand nicht (siehe Klärung oben).
- **Eingaben/Konfiguration:** keine; Anzeige des Katalogstands im Reiter „Kosten“.
- **Ausgabe:** „Preise: live vom Konto (13.10.2026 14:02)“ bzw. „Preise: mitgelieferter Katalog, Stand 13.10.2026“.
- **Akzeptanzkriterien:**
  - [ ] Ein CI-Test validiert `pricing.json` gegen ein JSON-Schema (Pflichtfelder, `checked_at` vorhanden, keine negativen Preise).
  - [ ] Liefert `user_data.price` für Labs `ranked_keywords` `cost_type: per_request`, `cost: 0.010`, rechnen Schätzung und Budget mit 0,010 $ je Task und weiterhin 0,00012 $ je Zeile aus dem Katalog (10 URLs, `limit` 100 → `Cost Min 0,1000 $`, `Cost Max 0,2200 $`); der Reiter „Kosten“ zeigt die Herkunft `account` für den Task- und `catalog` für den Zeilenpreis.
  - [ ] Liefert `user_data.price` für Backlinks `bulk_pages_summary` `cost_type: per_result`, `cost: 0.00003`, wird nur `cost_per_row` ersetzt (250 URLs → 0,024 + 250 · 0,00003 = 0,0315 $).
  - [ ] SERP-Zuschläge werden nie aus `user_data.price` überschrieben (Mock `user_data` mit SERP-Basispreis 0,0015 $ Live → `per_task_high` des AI-Overview-Zuschlags bleibt 0,002 $; `per_task_low` folgt als „1 Basispreis“ dem neuen Basispreis 0,0015 $).
  - [ ] Weicht die in einer Antwort gemeldete `cost` um mehr als 10 % von der Katalogrechnung ab (Szenario `price-changed`), wird einmal je Lauf und Endpunkt ein Hinweis „Preise haben sich geändert – Schätzung ungenau“ gezeigt und der Katalog für die laufende Sitzung mit dem beobachteten Wert korrigiert.
  - [ ] Ist der mitgelieferte Katalog älter als 180 Tage und keine Live-Preisliste verfügbar, zeigt der Reiter „Kosten“ einen Warnhinweis.
- **Testfixture:** Mock `user_data` mit abweichenden Preisen; Szenario `price-changed`.

### D4S-012 Kostenschätzung vor dem Lauf
- **Priorität:** P0
- **Beschreibung:** Vor jedem Lauf mit kostenpflichtigen DFS-Modulen (Crawlstart, „DataForSEO-Daten anfordern“, Neuanforderung, geplante Aufgabe) berechnet OpenSpider je Modul eine Unter- und Obergrenze der Kosten und zeigt sie zusammen mit Guthaben und Budget an. Grundlage ist die erwartete Zahl abzufragender URLs bzw. Keywords nach Abzug von Cache-Treffern (D4S-015) und Scope-Ausschlüssen (D4S-017); während eines Spider-Crawls ist die URL-Zahl noch unbekannt, daher wird zunächst mit der Obergrenze „max. URLs je Modul“ gerechnet und die Schätzung live nachgeführt. Formeln (Preise aus D4S-011):
  - Labs je URL: untere Grenze `n · Task`, obere `n · (Task + limit · Zeile)`; mit Vorfilter (D4S-024) `Σ (Task + min(count_i, limit) · Zeile)`.
  - Backlinks: `⌈n / 1.000⌉ · Task + n · Zeile` (zuzüglich Aufteilung wegen ≤ 100 Domains je Request), Domain-Ebene `Hosts · (Task + 1 · Zeile)`.
  - SERP (Zuschläge additiv je Task, nicht mit `depth` multipliziert): je eindeutiger Kombination i aus (Keyword, Markt, Gerät) `Task_i = m_i · (Basis(Modus) · ⌈depth / 10⌉ + Z_AIO + Z_rect)` mit `m_i` = 5 für Keywords mit Suchoperator (nur wenn erlaubt, D4S-040), sonst 1. **Obergrenze** `Cost Max = Σ Task_i` mit `Z = per_task_high`; **Untergrenze** `Cost Min = Σ m_i · (Basis · ⌈depth / 10⌉ + Z_rect,low)` ohne AI-Overview-Zuschlag, weil dieser erstattet wird, wenn kein AI Overview vorliegt (D4S-011). Die Budget-Reservierung (D4S-013) nutzt `Task_i` mit `per_task_high`.
  - LLM Mentions: `Requests · Task + erwartete Zeilen · Zeile` (obere Grenze mit `limit`).
  - **Gemeinsamer Dialog (Architekten-Entscheidung vom 13.10.2026, führend AI-012):** Die Tabelle dieses Eintrags ist der Abschnitt „DataForSEO“ des gemeinsamen Dialogs „Kostenvoranschlag“; sind zugleich KI-Aufgaben aktiv, zeigt derselbe Dialog den Abschnitt „KI“, die gemeinsame Summenzeile und den Rest des Crawl-Budgets. Es gibt keinen getrennten DataForSEO-Dialog.
- **Eingaben/Konfiguration:** Dialog „Kostenvoranschlag“ (Abschnitt „DataForSEO“) mit „Starten“, „Budget anpassen“, „Umfang reduzieren“, „Abbrechen“; Option „Schätzung vor jedem Lauf anzeigen“ (Standard an; aus = nur anzeigen, wenn die Obergrenze das Budget übersteigt).
- **Ausgabe:** Tabelle `Module`, `Requests`, `Rows (max)`, `Cost Min (USD)`, `Cost Max (USD)`, `Cached`; Summenzeile des Abschnitts; Guthaben; Crawl-Budget und ggf. Teilbudget DataForSEO (Rest laut `BudgetLedger`).
- **Akzeptanzkriterien:**
  - [ ] Backlinks für 250 URLs auf einer Domain (keine Cache-Treffer) → `Requests 1`, `Cost Min = Cost Max = 0,0330 $` (0,024 + 250 · 0,000036); tatsächlich wird genau 1 Request gesendet.
  - [ ] Labs für 10 URLs mit `limit` 100 → `Cost Min 0,1200 $`, `Cost Max 0,2400 $`.
  - [ ] SERP Live, 2 eindeutige Keywords, depth 20, AI-Overview-Zuschlag an → `Cost Max 0,0120 $` (2 · (0,002 · 2 + 0,002)), `Cost Min 0,0080 $` (2 · 0,002 · 2).
  - [ ] SERP Standard, 2 Keywords, depth 20, AI-Overview-Zuschlag an → `Cost Max 0,0064 $` (2 · (0,0006 · 2 + 0,002)), `Cost Min 0,0024 $`.
  - [ ] SERP Live, 1 Keyword mit Operator bei erlaubten Operatoren, depth 10, ohne Zuschläge → `Cost Min = Cost Max = 0,0100 $` (5 · 0,002).
  - [ ] 6 von 10 URLs mit gültigem Cache-Eintrag → Schätzung rechnet nur mit 4 URLs, Spalte `Cached = 6`.
  - [ ] Übersteigt `Cost Min` das Guthaben, ist „Starten“ deaktiviert mit Hinweis „Guthaben reicht nicht“; übersteigt `Cost Max` das Budget, wird der Dialog unabhängig von der Option angezeigt.
  - [ ] Die tatsächlichen Kosten eines Laufs liegen gegen den Mock zwischen `Cost Min` und `Cost Max` (Golden-Test über alle Module; der SERP-Mock meldet für Keywords ohne AI Overview Kosten ohne Zuschlag, für Keywords mit AI Overview mit Zuschlag).
  - [ ] Fehlerfälle: Nicht verbunden → Dialog zeigt nur Cache-Treffer und den Hinweis „DataForSEO nicht verbunden“, „Starten“ startet den Crawl ohne DFS-Requests; Preiskatalog ohne Eintrag für einen benötigten Endpunkt → Zeile „Preis unbekannt“, das betroffene Modul sendet keine kostenpflichtigen Requests (nur Cache und Sandbox), weil keine Reservierung berechnet werden kann.
- **Testfixture:** `tests/fixtures/dfs-site/` + Mock; vorbereitete Crawl-Datei mit 6 Cache-Einträgen.

### D4S-013 Hartes Budget je Crawl
- **Priorität:** P0
- **Beschreibung:** **Gemeinsames Crawl-Budget (Architekten-Entscheidung vom 13.10.2026; führend AI-013, `BudgetLedger` in `os-model`):** DataForSEO verbraucht dasselbe Crawl-Budget wie die KI-Funktionen aus 08 (Standard 5,00 $, 0 = keine kostenpflichtigen Requests, nur Cache/Sandbox) **(OS-Festlegung)**. Optional gibt es darunter ein Teilbudget „DataForSEO gesamt“ und Teilbudgets je Modul. Vor jedem Request wird über den `BudgetLedger` der **maximal mögliche** Preis dieses Requests (Task + `limit` · Zeile; bei SERP inklusive `per_task_high`-Zuschlägen und Operator-Faktor, D4S-012) reserviert; gesendet wird nur, wenn `verbraucht + reserviert + Maximalpreis ≤ Grenze` für das Crawl-Budget (verbraucht = Summe `dfs_cost_log` + Summe `Cost (USD)` aus `ai_requests`, reserviert = offene Reservierungen beider Bereiche) **und** für jedes betroffene DataForSEO-Teilbudget (nur DataForSEO-Verbrauch) gilt. Nach der Antwort ersetzt die gemeldete `cost` die Reservierung. Damit kann das Budget bei korrektem Preiskatalog nicht überschritten werden. Ist das Budget erreicht, werden keine weiteren Requests gestellt; der Crawl läuft weiter. Reicht das Budget nicht für alle URLs, werden URLs priorisiert (D4S-017). Eine gemeinsame Protokolltabelle gibt es nicht (D4S-014). In den Kriterien meint „Budget“ das Crawl-Budget ohne KI-Verbrauch, sofern nicht ausdrücklich ein Teilbudget oder KI-Verbrauch genannt ist.
- **Eingaben/Konfiguration:** Reiter „Kosten“: „Budget bezahlter APIs je Crawl (USD)“ (Crawl-Budget, dieselbe Einstellung wie AI-013; 0–10.000, zwei Nachkommastellen), „Teilbudget DataForSEO“ (leer = kein Teilbudget), „Teilbudget“ je Modul (leer = kein Teilbudget). CLI: `--api-budget` bzw. `--dfs-budget` (Teilbudget DataForSEO), OUT-201/D4S-073.
- **Ausgabe:** Status `Budget reached` für nicht mehr abgefragte URLs; Fortschritt im gemeinsamen Statusleistenfeld `Kosten` (UI-013, Tooltip mit DataForSEO-Anteil); Meldung „Budget erreicht – <n> DataForSEO-URLs ohne Daten“ (bei erreichtem Teilbudget „DataForSEO-Teilbudget erreicht – …“).
- **Akzeptanzkriterien:**
  - [ ] Budget 0,05 $, Labs mit `limit` 100 (Maximalpreis 0,024 $ je Request), Mock-Szenario `cost-fixed-max` (jede Antwort meldet 0,024 $), 10 URLs → genau 2 Requests (2 · 0,024 = 0,048 ≤ 0,05; ein 3. ergäbe 0,072 > 0,05); 8 URLs erhalten `Budget reached`.
  - [ ] Passt eine Reservierung nur wegen noch offener Reservierungen nicht, wartet die URL auf deren Abrechnung; `Budget reached` wird erst vergeben, wenn sie auch ohne offene Reservierungen nicht passt (Szenario mit tatsächlicher `cost` 0,012 $ je Request und Budget 0,05 $ → 3 Requests: 0,036 + Reservierung 0,024 > 0,05 erst beim 4.).
  - [ ] Die Summe der gemeldeten `cost` aller Requests eines Laufs ist ≤ Budget (Mock-Log-Summe), auch bei 10 parallelen Requests.
  - [ ] Teilbudget Backlinks 0,00 $ bei Crawl-Budget 5 $ → Backlinks-URLs `Budget reached`, Labs läuft normal.
  - [ ] Teilbudget „DataForSEO gesamt“ 0,05 $ bei Crawl-Budget 5 $ und Szenario wie im ersten Kriterium → genau 2 Requests; KI-Aufgaben desselben Crawls laufen bis zum Crawl-Budget weiter.
  - [ ] Budget 0 → kein kostenpflichtiger Request, Cache-Treffer werden trotzdem angezeigt (`Source = Cache`).
  - [ ] Erhöhen des Budgets nach `Budget reached` und „DataForSEO-Daten anfordern“ fragt genau die URLs mit `Budget reached` ab.
  - [ ] Szenario `price-changed` (gemeldete Kosten höher als reserviert): die App stoppt, sobald die nächste Reservierung nicht mehr passt; die Überschreitung ist höchstens die Differenz der bereits laufenden Requests und wird im Kostenprotokoll als „Budget überschritten um <x> $ (Preisabweichung)“ vermerkt.
  - [ ] SERP mit erlaubten Operatoren: Budget 0,009 $, Live, depth 10, 1 Operator-Keyword (Reservierung 5 · 0,002 = 0,010 $) → kein Request, Status `Budget reached`; dasselbe Keyword ohne Operator (0,002 $) wird gesendet.
  - [ ] Crawl-Budget 1,00 $, `dfs_cost_log` summiert 0,70 $ und `ai_requests` 0,25 $ → nächster DFS-Request mit Maximalpreis 0,10 $ wird nicht gesendet; ein DFS-Request mit Maximalpreis 0,05 $ wird gesendet.
  - [ ] In derselben Lage reserviert `os-ai` für einen KI-Request mit Reservierung 0,06 $ nichts und vergibt `Budget exceeded` (Gegenstück in AI-013; beide Bereiche nutzen denselben `BudgetLedger`).
- **Testfixture:** Mock mit festen `cost`-Werten; Szenario `price-changed`; Crawl-Datei mit vorbelegten Zeilen in `dfs_cost_log` und `ai_requests` sowie ein `ai-mock` aus 08.

### D4S-014 Kostenprotokoll
- **Priorität:** P0
- **Beschreibung:** Jeder Request an DataForSEO wird in der Crawl-Datei in der eigenen Tabelle `dfs_cost_log` protokolliert (KI-Anfragen stehen getrennt in `ai_requests`, AI-014; eine Zusammenführung erfolgt nur rechnerisch im `BudgetLedger` für das Crawl-Budget, AI-013/D4S-013): Zeitpunkt, Modul, Endpunkt, Modus, Anzahl Ziele, Anzahl Zeilen, reservierter Betrag, gemeldete `cost`, Status-Code, Herkunft (`api`/`sandbox`), Task-ID, Zweck (`data` | `probe` für Sondierungen nach D4S-023). Spaltennamen und Einheiten (Mikro-USD) sind so gewählt, dass die Summe mit der von `ai_requests` direkt addierbar ist. Gecachte Ergebnisse erzeugen keinen Eintrag. Zusätzlich führt die App einen installationsweiten Monatszähler je Konto (überlebt Neustart).
- **Eingaben/Konfiguration:** Reiter „Kosten“ → „Kostenprotokoll anzeigen“.
- **Ausgabe:** Tabelle `Time`, `Module`, `Endpoint`, `Mode`, `Purpose`, `Targets`, `Rows`, `Cost (USD)`, `Status Code`, `Source`; Summen je Modul und gesamt; Seitenleiste „API“ (INTEG-007) zeigt zusätzlich die Spalte `Cost (USD)` für DataForSEO. Export siehe D4S-072.
- **Akzeptanzkriterien:**
  - [ ] Nach einem Lauf gegen den Mock stimmt die Summe der Spalte `Cost (USD)` exakt (Mikro-USD) mit der Summe der vom Mock gemeldeten `cost` überein.
  - [ ] Fehlgeschlagene Requests werden mit ihrer gemeldeten `cost` (auch 0) protokolliert.
  - [ ] Das Protokoll enthält weder Login noch Passwort noch vollständige Request-Bodies.
  - [ ] Der Monatszähler „DataForSEO diesen Monat: <x> $“ summiert über alle Crawls; Löschen einer Crawl-Datei ändert ihn nicht.
- **Testfixture:** Mock mit `cost`-Werten je Endpunkt, inkl. eines Fehlerszenarios mit `cost` 0.

### D4S-015 Ergebnis-Cache in der Crawl-Datei
- **Priorität:** P0
- **Beschreibung:** Jede erfolgreiche Antwort (inkl. `No data`) wird in der Crawl-Datei (Tabelle `dfs_cache`) unter einem Schlüssel aus Umgebung (Produktiv/Sandbox), Endpunkt, normalisiertem Ziel (URL/Domain/Keyword), Markt, Gerät und dem Hash aller ergebnisrelevanten Parameter gespeichert (komprimiertes JSON, Erhebungszeitpunkt, gemeldete Kosten). Vor jedem Request wird der Cache geprüft; ein Treffer innerhalb der maximalen Gültigkeit ersetzt den Request. Bulk-Antworten werden je Ziel einzeln abgelegt, damit spätere Teilmengen treffen. Die Gültigkeit ist je Modul einstellbar (Standard Labs 30 Tage, Backlinks 7 Tage, SERP 1 Tag, LLM Mentions 30 Tage) **(OS-Festlegung)**.
- **Eingaben/Konfiguration:** Reiter „Kosten“: „Max. Cache-Alter“ je Modul (0 = Cache nie verwenden), „Cache leeren“ je Modul.
- **Ausgabe:** `DFS <Modul> Source = Cache`, `DFS <Modul> Fetched` = ursprünglicher Erhebungszeitpunkt.
- **Akzeptanzkriterien:**
  - [ ] Zweimaliges „DataForSEO-Daten anfordern“ mit identischer Konfiguration innerhalb der Gültigkeit → beim zweiten Mal 0 kostenpflichtige Requests (Mock-Log), alle Werte identisch, `Source = Cache`.
  - [ ] Änderung eines ergebnisrelevanten Parameters (z. B. Labs `limit` 100 → 200, Gerät, Markt) führt zu einem Cache-Fehlschlag; eine rein darstellende Änderung (Spaltenreihenfolge) nicht.
  - [ ] Bulk-Backlinks für 10 URLs, danach Abfrage für 4 dieser URLs → 0 Requests.
  - [ ] Max. Cache-Alter 0 → jeder Lauf fragt neu ab.
  - [ ] Ein Cache-Eintrag älter als die Gültigkeit wird nicht verwendet (Testuhr +8 Tage bei Backlinks).
  - [ ] Die Crawl-Datei bleibt bei 100.000 gecachten Labs-Antworten mit je 100 Keywords unter 1,5 GB Zuwachs (komprimierte Ablage).
- **Testfixture:** Mock + Testuhr; synthetischer Lasttest für die Größenprüfung.

### D4S-016 Cache aus früherem Crawl übernehmen
- **Priorität:** P1
- **Beschreibung:** Beim Start eines Crawls (oder in der Crawl-Verwaltung) kann der DFS-Cache eines früheren Crawls desselben Projekts übernommen werden; nur Einträge innerhalb der Gültigkeit werden kopiert. So zahlen wiederholte Crawls (z. B. geplante Läufe) nur für Neues oder Abgelaufenes.
- **Eingaben/Konfiguration:** Auswahl „DataForSEO-Cache übernehmen aus: <Crawl>“ (Standard: letzter Crawl desselben Projekts, wenn vorhanden; Option „aus“).
- **Ausgabe:** Hinweis „<n> DataForSEO-Ergebnisse aus Crawl <Name> übernommen“.
- **Akzeptanzkriterien:**
  - [ ] Zweiter Crawl derselben Fixture-Site am selben Tag mit Übernahme → 0 Labs- und Backlinks-Requests; SERP nur, wenn dessen Gültigkeit (1 Tag) abgelaufen ist.
  - [ ] Sandbox-Einträge werden nie in einen Produktiv-Crawl übernommen.
  - [ ] Übernommene Werte tragen den ursprünglichen Erhebungszeitpunkt.
- **Testfixture:** zwei aufeinanderfolgende Crawls derselben Fixture; Testuhr.

### D4S-017 Abfrageumfang und Priorisierung
- **Priorität:** P0
- **Beschreibung:** Standardmäßig werden je URL nur **interne HTML-Seiten mit Status 200 und `Indexability = Indexable`** abgefragt. Das gilt ausdrücklich auch für das Backlinks-Modul (Architekten-Entscheidung vom 13.10.2026, Q10): standardmäßig nur indexierbare 200er, umschaltbar über die Umfangsoptionen; der Kostenvoranschlag weist bei aktivem Backlinks-Modul auf die Optionen hin, weil D4S-061/062 sie voraussetzen. Je Modul kann der Umfang erweitert (nicht indexierbare bzw. Nicht-200-URLs, z. B. für D4S-060/061) oder eingeschränkt werden (Segment nach CRAWL-134, maximale URL-Zahl). Reichen Budget oder Maximalzahl nicht, werden URLs in fester Reihenfolge ausgewählt: Crawl-Tiefe aufsteigend, dann `Unique Inlinks` absteigend, dann Adresse alphabetisch **(OS-Festlegung)**. Unabhängig davon gelten die Senderegeln aus D4S-010 (Query-Strings, Zugangsdaten in URLs). Im APIs-Modus (INTEG-012) ist die Indexierbarkeit unbekannt; dort gelten alle Listen-URLs als im Umfang.
- **Eingaben/Konfiguration:** Je Modul: „Auch nicht indexierbare URLs“ (Standard aus), „Auch Nicht-200-URLs (3xx/4xx/5xx)“ (Standard aus), „Segment“ (Standard: keins), „Max. URLs“ (Standard 10.000 **(OS-Festlegung)**).
- **Ausgabe:** Status `Skipped (non-indexable)`, `Skipped (non-200)`, `Skipped (segment)`, `Skipped (scope limit)`.
- **Akzeptanzkriterien:**
  - [ ] Standard-Crawl der Fixture-Site → Labs fragt genau `/`, `/a`, `/e`, `/f`, `/g`, `/i`, `/j`, `/deep/1`–`/deep/3` ab (10 URLs); `/b` und `/h` erhalten `Skipped (non-indexable)`, `/c`, `/d` und `/d2` `Skipped (non-200)`, `/q?session=abc123` `Skipped (query string)` (D4S-010; die Query-Regel wird vor Umfangsgrenze und Priorisierung angewendet).
  - [ ] Mit „Auch nicht indexierbare URLs“ und „Auch Nicht-200-URLs“ für Backlinks werden zusätzlich `/b`, `/h`, `/c`, `/d`, `/d2` abgefragt.
  - [ ] „Max. URLs“ 3 → abgefragt werden `/` (Tiefe 0) sowie `/a` (3 Unique Inlinks) und `/e` (2) als Tiefe-1-URLs mit den meisten `Unique Inlinks`; übrige `Skipped (scope limit)`.
  - [ ] Im APIs-Modus mit 5 Listen-URLs (1 Duplikat) werden 4 URLs abgefragt.
- **Testfixture:** `tests/fixtures/dfs-site/`; `tests/fixtures/dfs-apis-list.txt`.

### D4S-018 Neuanforderung und Invalidierung
- **Priorität:** P1
- **Beschreibung:** Es gelten INTEG-008 (nachträglich anfordern), INTEG-009 (einzelne URLs neu anfordern) und INTEG-010 (Invalidierung). Zusätzlich für DataForSEO: „Neu anfordern“ umgeht den Cache für die markierten URLs ausdrücklich und zeigt vorher die Kostenschätzung (D4S-012); „Fehlgeschlagene neu anfordern“ betrifft nur URLs mit `Error: …`, `Budget reached`, `Insufficient funds`, `Quota exceeded`, `Pending` (abgelaufen). Ergebnisrelevante Optionsänderungen (Markt, Gerät, Labs-Optionen, Rank-Skala, Tiefe) lösen den INTEG-010-Dialog aus.
- **Eingaben/Konfiguration:** Kontextmenü „API → DataForSEO → <Modul | Alle> neu anfordern“, „Fehlgeschlagene neu anfordern“.
- **Ausgabe:** aktualisierte Zellen, `Source = Live API`.
- **Akzeptanzkriterien:**
  - [ ] 3 markierte URLs mit gültigem Cache, „Labs neu anfordern“ → genau 3 Labs-Requests nach Bestätigung der Schätzung (Mock-Log).
  - [ ] „Fehlgeschlagene neu anfordern“ nach einem Lauf mit 2 × `Error`, 3 × `Budget reached`, 5 × `Success` → genau 5 Requests.
  - [ ] Änderung von `rank_scale` nach dem Abruf → INTEG-010-Dialog; „Löschen und neu laden“ fragt alle Backlinks-Werte neu ab.
- **Testfixture:** gespeicherter Crawl `tests/fixtures/dfs-crawls/site-mixed-status.ospider` + Mock.

---

## C Labs Ranked Keywords

### D4S-020 Ranked Keywords je URL
- **Priorität:** P0
- **Beschreibung:** Für jede URL im Umfang (D4S-017) wird `POST /v3/dataforseo_labs/google/ranked_keywords/live` mit der vollständigen URL als `target`, dem Markt (D4S-007), `item_types: ["organic"]`, `limit` (Standard 100), `order_by: ["ranked_serp_element.serp_item.etv,desc"]` und `historical_serp_mode: "live"` aufgerufen **(OS-Festlegung: Sortierung nach Traffic, damit das Top-Keyword immer in der ersten Seite liegt)**. Aus der Antwort entstehen die Kennzahlen je URL; die Keyword-Liste wird in `dfs_ranked_keywords` (URL, Keyword, Kennzahlen) gespeichert. Seiten-URLs werden mit Protokoll gesendet; da die API eine Seiten-URL nur mit `https://` oder `www.` als Seite erkennt, werden `http://`-URLs ohne `www.` nicht gesendet und erhalten `Error: URL-Form von Labs nicht unterstützt` **(OS-Festlegung, siehe offene Frage Q4)**.
  Ableitungen:
  - `DFS Ranked Keywords` = `result.total_count`
  - `DFS Keywords Top 3` = `metrics.organic.pos_1 + metrics.organic.pos_2_3`; `DFS Keywords Top 10` = zusätzlich `+ pos_4_10`
  - `DFS Est. Traffic` = `metrics.organic.etv` (geschätzte monatliche Besuche, gerundet auf 1 Nachkommastelle)
  - `DFS Est. Traffic Cost` = `metrics.organic.estimated_paid_traffic_cost` (USD)
  - **Top-Keyword** = Keyword mit dem höchsten `serp_item.etv`; bei Gleichstand höheres `search_volume`, dann bessere `rank_group`, dann alphabetisch **(OS-Festlegung)**; daraus `DFS Top Keyword`, `DFS Top Keyword Position` (`rank_group`), `DFS Top Keyword Volume` (`keyword_info.search_volume`), `DFS Top Keyword Traffic` (`serp_item.etv`).
- **Eingaben/Konfiguration:** Modul „Labs“ aktivieren; Optionen in D4S-022.
- **Ausgabe:** Spalten im Tab `Organic Keywords` (D4S-026) und im Internal-Tab: `DFS Ranked Keywords`, `DFS Keywords Top 3`, `DFS Keywords Top 10`, `DFS Est. Traffic`, `DFS Est. Traffic Cost`, `DFS Top Keyword`, `DFS Top Keyword Position`, `DFS Top Keyword Volume`, `DFS Top Keyword Traffic`, `DFS Location`, `DFS Language`, `DFS Labs Source`, `DFS Labs Fetched`, `DFS Labs Status`.
- **Akzeptanzkriterien:**
  - [ ] `/a` → `DFS Ranked Keywords 3`, `DFS Keywords Top 3 1`, `DFS Keywords Top 10 2`, `DFS Est. Traffic 320.5`, `DFS Top Keyword widget shop`, `DFS Top Keyword Position 2`, `DFS Top Keyword Volume 1000`, `DFS Top Keyword Traffic 250.5`, `DFS Labs Status Success`.
  - [ ] `/` (Mock 40102) → alle Kennzahlspalten leer, `DFS Labs Status No data`.
  - [ ] `/j` (Task 20000, leere `items`, `total_count` 0) → `DFS Ranked Keywords 0`, `DFS Keywords Top 3 0`, `DFS Keywords Top 10 0`, `DFS Est. Traffic 0`, `DFS Est. Traffic Cost 0`, `DFS Top Keyword`/`… Position`/`… Volume`/`… Traffic` leer, Status `Success` (Regel „keine Daten“ vs. „null Treffer“).
  - [ ] Crawl von `{BASE_NOWWW}` (http, ohne `www.`) → 0 Labs-Requests, alle URLs `Error: URL-Form von Labs nicht unterstützt`; ein Crawl derselben Pfade unter `{BASE}` sendet je URL einen Request.
  - [ ] Request-Body für `/a` enthält `target` = `{BASE}/a`, `limit: 100`, `item_types: ["organic"]`, die `order_by`-Regel und den Markt (Mock-Log).
  - [ ] Pro URL genau 1 Request (ohne Wiederholungen); Ergebnisse erscheinen live während des Crawls.
  - [ ] Fehlerfälle: nicht verbunden → `Not connected`; Budget erreicht → `Budget reached`; 40210 → `Insufficient funds`; Timeout → `Error: Timeout nach 120 s` (jeweils über die gemeinsamen Mock-Szenarien geprüft).
- **Testfixture:** `tests/fixtures/dfs-site/` + Mock `ranked_keywords` (Datensätze wie in der Tabelle).

### D4S-021 Detail-Tab „Ranked Keywords“
- **Priorität:** P0
- **Beschreibung:** Unterer Detail-Tab für die markierte URL mit allen gespeicherten Keywords (bis `limit`). Mehrfachauswahl zeigt die Keywords aller markierten URLs mit Spalte `Address`.
- **Eingaben/Konfiguration:** Spaltenauswahl, Sortierung, Textfilter auf `Keyword`.
- **Ausgabe:** Spalten `Address`, `Keyword`, `Position` (`rank_group`), `Absolute Position` (`rank_absolute`), `Search Volume`, `Est. Traffic` (`serp_item.etv`), `CPC`, `Keyword Difficulty`, `Search Intent` (`main_intent`), `SERP Item Type`, `SERP Features` (`serp_item_types`, mit `; ` getrennt), `Rank Change` (`New` / `Up` / `Down` / leer aus `rank_changes`), `Previous Position`, `Cannibalized` (`Yes`/`No`, D4S-063).
- **Akzeptanzkriterien:**
  - [ ] Markierung `/a` → 3 Zeilen, standardmäßig nach `Est. Traffic` absteigend sortiert.
  - [ ] Markierung `/e` und `/f` → 2 Zeilen `blue widgets` mit `Cannibalized = Yes` (nach Crawl-Analyse).
  - [ ] Export des Detail-Tabs (OUT-021) enthält genau diese Spalten.
- **Testfixture:** `tests/fixtures/dfs-site/` + Mock.

### D4S-022 Labs-Optionen
- **Priorität:** P1
- **Beschreibung:** Weitere ergebnisrelevante Parameter für Ranked Keywords:
  - **Keywords je URL** (`limit`, 1–1.000, Standard 100) – bestimmt Zeilenkosten und Länge der Detailliste; Kennzahlen aus `metrics` bleiben vollständig.
  - **Maximale Position** (Filter `ranked_serp_element.serp_item.rank_group <= N`, Standard: keiner) – reduziert Zeilen; `DFS Ranked Keywords` zählt dann nur Keywords bis Position N (Spaltentooltip nennt den Filter).
  - **Mindest-Suchvolumen** (Filter `keyword_data.keyword_info.search_volume >= N`, Standard: keiner).
  - **Synonyme zusammenfassen** (`ignore_synonyms`, Standard aus).
  - **AI-Overview-Referenzen zählen** (`item_types` zusätzlich `ai_overview_reference`; neue Spalte `DFS AI Overview Keywords` = `metrics.ai_overview_reference.count`; Detail-Tab zeigt diese Zeilen mit `SERP Item Type = ai_overview_reference`), Standard aus.
  - **Verlorene Keywords** (`historical_serp_mode: "lost"` als zweiter Request je URL; Spalte `DFS Lost Keywords` = `total_count`), Standard aus.
  - **Clickstream-Daten** (`include_clickstream_data`, verdoppelt den Preis; Spalte `DFS Clickstream Traffic` = `metrics.organic.clickstream_etv`), Standard aus.
- **Eingaben/Konfiguration:** Reiter „Module“ → „Labs“.
- **Ausgabe:** wie D4S-020 plus optionale Spalten.
- **Akzeptanzkriterien:**
  - [ ] Maximale Position 10 → Request enthält genau einen Filter `["ranked_serp_element.serp_item.rank_group","<=",10]`; zusammen mit Mindest-Suchvolumen werden die Filter mit `"and"` verbunden (Mock-Log).
  - [ ] Mehr als 8 Filterbedingungen sind in der UI nicht konfigurierbar (API-Grenze).
  - [ ] AI-Overview-Referenzen an → `item_types: ["organic","ai_overview_reference"]`; `/i` zeigt `DFS AI Overview Keywords 1`.
  - [ ] Verlorene Keywords an → 2 Requests je URL; die Kostenschätzung verdoppelt den Task-Anteil.
  - [ ] Clickstream an → Kostenschätzung verdoppelt sich.
  - [ ] Jede Optionsänderung nach einem Abruf löst INTEG-010 aus und erzeugt neue Cache-Schlüssel.
- **Testfixture:** Mock mit Request-Protokoll und Antworten je `item_types`/`historical_serp_mode`.

### D4S-023 Abrufstrategie „Domain-Abzug“
- **Priorität:** P1
- **Beschreibung:** Alternativ zur Abfrage je URL lädt OpenSpider alle Ranked Keywords der **eigenen Domain** seitenweise (`target` = Domain ohne Protokoll, `limit` 1.000, `offset` in 1.000er-Schritten, Sortierung nach `etv` absteigend, optional Positions-/Volumenfilter aus D4S-022) und ordnet jede Zeile über `ranked_serp_element.serp_item.url` den Crawl-URLs zu (INTEG-014-Abgleich). Das ist bei großen Sites deutlich günstiger (z. B. 50.000 Keywords: 50 Requests ≈ 6,60 $ statt 0,012 $ je URL) und liefert zugleich Keywords für URLs, die nicht gecrawlt wurden (D4S-065). Kennzahlen je URL werden aus den zugeordneten Zeilen berechnet (Anzahl, Top 3/Top 10, Summe `etv`); sie können von D4S-020 abweichen, wenn der Abzug durch eine Höchstzahl begrenzt ist. In der Strategie „Automatisch“ wählt OpenSpider anhand der Kostenschätzung die günstigere Variante; dafür muss die Zahl der Domain-Keywords bekannt sein, die nur eine **Sondierung** liefert **(OS-Festlegung)**:
  1. **Auslöser:** Sobald `n` = Zahl der URLs im Labs-Umfang (D4S-017) ≥ 50 ist (Spider-Crawl: sobald 50 URLs im Umfang gecrawlt sind; Listen-/APIs-Modus und „DataForSEO-Daten anfordern“: sofort) oder der Crawl mit `n` ≥ 50 endet. Bei `n` < 50 wird ohne Sondierung `Je URL` gewählt, weil die Sondierung dann einen spürbaren Anteil der Kosten ausmacht. Bis zur Entscheidung warten Labs-URLs mit Status `Pending`.
  2. **Sondierungs-Request:** `ranked_keywords/live` mit `target` = eigene Domain, Markt, denselben Filtern wie der spätere Abzug, `limit: 1`; ausgewertet wird nur `total_count`. Kosten laut Katalog 0,012 + 1 · 0,00012 = 0,01212 $; sie werden vor dem Senden wie jeder Request reserviert (D4S-013), im Kostenprotokoll mit `Purpose = probe` vermerkt und im Schätzdialog als eigene Zeile „Labs – Sondierung“ ausgewiesen. Das Ergebnis wird gecacht (Gültigkeit wie Labs, D4S-015); ein gültiger Cache-Eintrag ersetzt die Sondierung.
  3. **Entscheidung:** Verglichen werden `Cost Min` von `Je URL` für `n` URLs (`n · Task`) und die Kosten des Domain-Abzugs `⌈K / 1.000⌉ · Task + K · Zeile` mit `K = min(total_count, Max. Keywords)`; gewählt wird der kleinere Betrag, bei Gleichstand `Je URL`. Im Spider-Crawl wächst `n` noch: Da die Kosten von `Je URL` mit `n` steigen, die des Domain-Abzugs aber fest sind, fällt die Entscheidung für `Domain-Abzug` endgültig, sobald `n · Task` die Domain-Kosten übersteigt; bis dahin bleiben die Labs-URLs `Pending`, und endet der Crawl vorher, wird `Je URL` mit dem tatsächlichen `n` ausgeführt. Die Entscheidung gilt für den ganzen Lauf.
  4. **Fehler der Sondierung** (Timeout, 5xx nach Wiederholungen, 40102) → Rückfall auf `Je URL`, Hinweis im Schätzdialog; Budget erreicht → keine Sondierung, alle Labs-URLs `Budget reached`.
- **Eingaben/Konfiguration:** „Abrufstrategie“: `Je URL` | `Domain-Abzug` | `Automatisch` (Standard `Je URL` in P0, `Automatisch` ab P1 **(OS-Festlegung)**); „Max. Keywords beim Domain-Abzug“ (Standard 100.000).
- **Ausgabe:** wie D4S-020; `DFS Labs Source = Live API (Domain)`; Tab-Hinweis „Kennzahlen aus Domain-Abzug (max. <n> Keywords)“.
- **Akzeptanzkriterien:**
  - [ ] Mock-Domain mit 2.500 Keywords → genau 3 Requests mit `offset` 0, 1000, 2000 (Mock-Log); Kosten 3 · 0,012 + 2.500 · 0,00012 = 0,3360 $.
  - [ ] Zeile mit `serp_item.url = http://<host>/a/` wird der gecrawlten `https://<host>/a/` zugeordnet.
  - [ ] Zeilen mit URLs, die nicht im Crawl sind, werden gespeichert und erscheinen in D4S-065.
  - [ ] „Automatisch“ mit 10 URLs (Listenmodus) → keine Sondierung, `Je URL` (Mock-Log: kein Request mit `limit: 1`).
  - [ ] „Automatisch“ mit 5.000 URLs (Listenmodus), Mock-Domain mit `total_count` 2.500 → genau 1 Sondierungs-Request (`limit: 1`, Domain-Ziel) vor allen anderen Labs-Requests, Kostenprotokoll-Zeile mit `Purpose probe` und `Cost 0,0121 $` (gerundet; intern 12.120 µUSD); Vergleich 60,0000 $ (`Je URL`, Cost Min) gegen 0,3360 $ → `Domain-Abzug`; Gesamtkosten Labs 0,3481 $; beide Beträge im Schätzdialog sichtbar.
  - [ ] Zweiter Lauf innerhalb der Cache-Gültigkeit → keine erneute Sondierung (Mock-Log).
  - [ ] Spider-Crawl mit 60 URLs im Umfang, Sondierung `total_count` 2.500 (Domain-Kosten 0,3360 $) → Sondierung, sobald die 50. URL im Umfang ist; da 50 · 0,012 = 0,60 $ > 0,336 $, sofort `Domain-Abzug` (Mock-Log: Sondierung und Abzug-Requests vor Crawl-Ende, keine Einzel-Requests).
  - [ ] Spider-Crawl mit 60 URLs im Umfang, Sondierung `total_count` 250.000 (Domain-Kosten bei „Max. Keywords“ 100.000: 100 · 0,012 + 100.000 · 0,00012 = 13,2000 $) → Labs-URLs bleiben bis Crawl-Ende `Pending`, danach 60 Einzel-Requests (`Je URL`, 0,72 $).
  - [ ] Szenario `5xx-always` nur für den Sondierungs-Request → Rückfall auf `Je URL`, Hinweis „Sondierung fehlgeschlagen – Abfrage je URL“.
  - [ ] Wird „Max. Keywords“ erreicht, erscheint der Hinweis, dass Kennzahlen unvollständig sein können.
  - [ ] Teilfehler (Szenario `partial-offset`): Kennzahlen je URL werden nur aus vollständig geladenen Seiten berechnet und tragen den Tab-Hinweis „Domain-Abzug unvollständig (<geladen>/<gesamt> Keywords)“; URLs ohne zugeordnete Zeile erhalten **nicht** `0`, sondern `Error: 50000 Domain-Abzug unvollständig`.
- **Testfixture:** Mock `ranked_keywords` für Domain-Ziel mit 2.500 synthetischen Zeilen (deterministisch generiert), davon 2 auf `/orphan`, inklusive Antwort auf `limit: 1` mit `total_count` 2.500; Listendatei mit 5.000 URLs.

### D4S-024 Vorfilter per Bulk Traffic Estimation
- **Priorität:** P1
- **Beschreibung:** Optional wird vor den teureren Einzelabfragen `POST /v3/dataforseo_labs/google/bulk_traffic_estimation/live` mit bis zu 1.000 URLs je Request aufgerufen. URLs mit `metrics.organic.count = 0` erhalten sofort `DFS Ranked Keywords 0` und werden nicht einzeln abgefragt; für die übrigen wird die Kostenschätzung mit den tatsächlichen Keyword-Zahlen präzisiert. Die Preisbasis des Bulk-Endpunkts (je Task und je Item wie Labs allgemein) ist **(Annahme, zu verifizieren)**.
  **URL-Form:** Der Endpunkt verlangt Seiten-URLs als absolute URL mit `https://` **und** `www.`; wie eine andere Form ausgewertet wird (womöglich als Domain), ist nicht dokumentiert (Q4). Ein fälschlich als Domain gewertetes Ziel lieferte `count > 0` (nur verschenkte Ersparnis) oder – schlimmer – eine falsche `0`, die die Einzelabfrage unterdrückt. Deshalb **(OS-Festlegung)**: Nur URLs, die mit `https://www.` beginnen, laufen über den Vorfilter; alle anderen URLs werden ohne Vorfilter direkt nach D4S-020 abgefragt (sofern dort zulässig). Zusätzlich wird ein Bulk-Item verworfen (URL behandelt wie ohne Vorfilter), wenn sein `target` nicht der gesendeten URL entspricht.
- **Eingaben/Konfiguration:** Checkbox „Vorab per Bulk-Schätzung filtern“ (Standard an, sobald P1 verfügbar).
- **Ausgabe:** Spalte `DFS Bulk Est. Traffic` (`metrics.organic.etv`); `DFS Labs Status = Success` für URLs mit 0 Keywords ohne Einzelabfrage.
- **Akzeptanzkriterien:**
  - [ ] 10 URLs der Fixture-Site → 1 Bulk-Request mit 10 `targets`; danach Einzel-Requests nur für die URLs mit `count > 0` (Mock-Log: `/a`, `/e`, `/f`, `/g`, `/i`).
  - [ ] 2.300 URLs → 3 Bulk-Requests (1.000/1.000/300).
  - [ ] Liefert der Bulk-Request einen Fehler, werden alle URLs einzeln abgefragt (kein Datenverlust), mit Hinweis im Kostenprotokoll.
  - [ ] URLs mit `count = 0` im Bulk-Ergebnis → `DFS Labs Status Success`, `DFS Ranked Keywords 0`, `DFS Keywords Top 10 0`, `DFS Est. Traffic 0` (gleiche Regel wie D4S-020 für leere `items`); fehlt eine URL in der Bulk-Antwort, wird sie einzeln abgefragt statt `No data` zu erhalten.
  - [ ] Crawl von `{BASE_HTTPS_NOWWW}` (https ohne `www.`) → 0 Bulk-Requests (Mock-Log), alle URLs werden direkt einzeln abgefragt; Tooltip der Spalte `DFS Bulk Est. Traffic`: „Vorfilter nur für https://www.-URLs“.
  - [ ] Mock-Item mit `target` = `www.dfs-site.test` (Domain statt gesendeter URL `{BASE}/a`) → Item verworfen, `/a` wird einzeln abgefragt.
- **Testfixture:** Mock `bulk_traffic_estimation` inkl. Variante mit Domain-`target`; Fixture-Variante `{BASE_HTTPS_NOWWW}`.

### D4S-025 Relevant Pages und Orphan-Kandidaten
- **Priorität:** P1
- **Beschreibung:** `POST /v3/dataforseo_labs/google/relevant_pages/live` mit der eigenen Domain liefert die rankenden Seiten der Domain mit Kennzahlen (`page_address`, `metrics.organic.etv`, `count`, Positionsverteilung), sortiert nach Keyword-Anzahl, bis 1.000 je Request. Seiten, die nicht im Crawl enthalten sind, sind Orphan-Kandidaten (D4S-065) und können wie bei INTEG-015 in die Crawl-Warteschlange aufgenommen werden.
- **Eingaben/Konfiguration:** Checkbox „Rankende Seiten der Domain abrufen“ (Standard aus), „Max. Seiten“ (Standard 1.000), „In DataForSEO entdeckte neue URLs crawlen“ (Standard aus).
- **Ausgabe:** Report „DataForSEO Relevant Pages“ mit `Page`, `In Crawl` (`Yes`/`No`), `DFS Ranked Keywords`, `DFS Est. Traffic`, `DFS Keywords Top 10`; Quelle `DataForSEO` im Orphan-Pages-Report (OUT-099).
- **Akzeptanzkriterien:**
  - [ ] Mock liefert `/a`, `/g`, `/orphan` → Report enthält 3 Zeilen, `/orphan` mit `In Crawl = No` und `DFS Est. Traffic 75`.
  - [ ] Mit „neue URLs crawlen“ wird `/orphan` gecrawlt (Fixture-Server-Log) und gilt als Orphan der Quelle `DataForSEO`; URLs fremder Hosts nie.
  - [ ] Include/Exclude und robots.txt gelten für so entdeckte URLs.
- **Testfixture:** Mock `relevant_pages`; `tests/fixtures/dfs-site/`.

### D4S-026 Tab „Organic Keywords“
- **Priorität:** P0
- **Beschreibung:** Neuer Haupt-Tab mit den Labs-Kennzahlen je URL und Filtern, die Kombinationen mit Crawl-Daten sichtbar machen. Filter, die Crawl-Analyse benötigen, sind entsprechend markiert.
- **Eingaben/Konfiguration:** wie alle Tabs (Spaltenauswahl, Suche, Segment).
- **Ausgabe:** Spalten `Address`, `Status Code`, `Indexability`, `Indexability Status`, `Unique Inlinks`, Spalten aus D4S-020/022, `DFS Cannibalized Keywords`. Filter: `All`, `With Rankings`, `No Rankings`, `No Data`, `Top 3 Rankings`, `Non-Indexable With Rankings` (D4S-060), `Cannibalization` (D4S-063), `High Traffic Low Inlinks` (D4S-064), `Orphan URLs` (D4S-065), `Main Keyword Not In Title` (D4S-068, P2), `Errors`.
- **Akzeptanzkriterien:**
  - [ ] `With Rankings` = URLs mit `DFS Labs Status Success` und `DFS Ranked Keywords ≥ 1`; `No Rankings` = `DFS Labs Status Success` und `DFS Ranked Keywords = 0`; `No Data` = `DFS Labs Status No data`. Die drei Filter sind disjunkt; URLs ohne Abfrage (Skipped/Fehler/Pending) sind in keinem von ihnen.
  - [ ] Standard-Crawl der Fixture-Site → `No Rankings` enthält genau `/j` und `/deep/1`–`/deep/3` (Mock: leere `items`), `No Data` genau `/` (40102), `With Rankings` genau `/a`, `/e`, `/f`, `/g`, `/i`.
  - [ ] `Top 3 Rankings` = `DFS Keywords Top 3 ≥ 1`.
  - [ ] `Errors` enthält alle URLs mit `DFS Labs Status` beginnend mit `Error:` oder gleich `Insufficient funds`, `Quota exceeded`, `Subscription required`.
  - [ ] Ohne aktiviertes Labs-Modul ist der Tab ausgeblendet.
- **Testfixture:** `tests/fixtures/dfs-site/` + Mock; erwartete Filterzählungen in `tests/golden/dfs-site/organic-keywords.json`.

---

## D Backlinks

### D4S-030 Backlink-Kennzahlen je URL
- **Priorität:** P0
- **Beschreibung:** Für URLs im Umfang wird `POST /v3/backlinks/bulk_pages_summary/live` mit bis zu 1.000 Zielen je Request aufgerufen; da ein Request höchstens 100 verschiedene Domains enthalten darf, gruppiert OpenSpider die Ziele nach registrierbarer Domain (relevant bei externen Zielen und im APIs-Modus). Parameter: `rank_scale: "one_hundred"` **(OS-Festlegung, Vergleichbarkeit mit anderen 0–100-Autoritätswerten; umstellbar auf `one_thousand`)**, `include_subdomains: true`. Die Spalten folgen dem Linkmetrik-Schema aus INTEG-150 mit Ebene `Exact URL` und erscheinen im Tab `Link Metrics` (INTEG-151). Ableitungen:
  - `DFS Dofollow Referring Domains %` = (`referring_domains` − `referring_domains_nofollow`) / `referring_domains` · 100, zwei Nachkommastellen; leer bei `referring_domains = 0`.
  - `DFS Dofollow Backlinks %` = (`backlinks` − `referring_links_attributes.nofollow`) / `backlinks` · 100; fehlt der Schlüssel `nofollow`, gilt 0 **(abgeleitet)**; leer bei `backlinks = 0`.
- **Eingaben/Konfiguration:** Modul „Backlinks“ aktivieren; Ebenen-Checkboxen (INTEG-150; `Exact URL (HTTP + HTTPS)` ist deaktiviert, da DataForSEO keine Summierung beider Protokolle anbietet); „Rank-Skala“ (`0–100` | `0–1000`).
- **Ausgabe:** `DFS Backlinks (Exact URL)`, `DFS Referring Domains (Exact URL)`, `DFS Referring Main Domains (Exact URL)`, `DFS Nofollow Referring Domains (Exact URL)`, `DFS Dofollow Referring Domains % (Exact URL)`, `DFS Dofollow Backlinks % (Exact URL)`, `DFS Rank (Exact URL)`, `DFS Main Domain Rank (Exact URL)`, `DFS Backlink Spam Score (Exact URL)`, `DFS Broken Backlinks (Exact URL)`, `DFS Referring IPs (Exact URL)`, `DFS Referring Subnets (Exact URL)`, `DFS First Seen (Exact URL)`, `DFS Backlinks Source`, `DFS Backlinks Fetched`, `DFS Backlinks Status`.
- **Akzeptanzkriterien:**
  - [ ] `/a` → `DFS Backlinks (Exact URL) 120`, `DFS Referring Domains (Exact URL) 30`, `DFS Nofollow Referring Domains (Exact URL) 6`, `DFS Dofollow Referring Domains % (Exact URL) 80.00`, `DFS Rank (Exact URL) 45`.
  - [ ] `/g` mit 0 Backlinks → `DFS Backlinks 0`, `DFS Dofollow … %` leer.
  - [ ] 10 URLs eines Hosts → genau 1 Request mit 10 `targets` und `rank_scale: "one_hundred"` (Mock-Log).
  - [ ] 150 externe Ziel-URLs auf 150 verschiedenen Domains (Option „Linkmetriken für externe URLs“, INTEG-151) → 2 Requests (100 + 50 Domains).
  - [ ] Rank-Skala `0–1000` → Request mit `one_thousand`; geänderte Werte lösen INTEG-010 aus.
  - [ ] Fehlerfälle: Szenario `partial` → fehlende URL `No data`; `subscription` → `Subscription required` (nur Backlinks); Budget/Guthaben/Timeout wie D4S-004/006/013.
- **Testfixture:** Mock `bulk_pages_summary`; `tests/fixtures/dfs-site/`; Liste mit 150 externen Domains `tests/fixtures/dfs-external-150.txt`.

### D4S-031 Subdomain- und Domain-Ebene
- **Priorität:** P0
- **Beschreibung:** Für die Ebenen `Subdomain` und `Domain` (INTEG-150) wird je eindeutigem Host bzw. je registrierbarer Domain genau ein Request `POST /v3/backlinks/summary/live` gestellt (`target` ohne Protokoll und ohne `www.`; Subdomain-Ebene mit `include_subdomains: false`, Domain-Ebene mit `true`) und der Wert allen URLs zugeordnet. Zusätzlich zeigt eine Seitenleisten-Übersicht „Backlinks (Domain)“ die Domain-Kennzahlen inklusive Top-TLDs und Linktypen (`referring_links_tld`, `referring_links_types`, je Top 10).
- **Eingaben/Konfiguration:** Ebenen-Checkboxen; „Interne Backlinks ausschließen“ (`exclude_internal_backlinks`, Standard an), „Indirekte Links einbeziehen“ (`include_indirect_links`, Standard an).
- **Ausgabe:** Spalten wie D4S-030 mit Ebene `(Subdomain)` bzw. `(Domain)`; Seitenleiste mit `Backlinks`, `Referring Domains`, `Rank`, `Spam Score`, `Broken Backlinks`, Top-TLDs, Linktypen.
- **Akzeptanzkriterien:**
  - [ ] 10 URLs auf einem Host, Ebene Domain → genau 1 `summary`-Request; alle 10 URLs zeigen `DFS Backlinks (Domain) 5000000000` (64-Bit, keine Überläufe in Anzeige und CSV).
  - [ ] `blog.example.co.uk/x` → Domain-Ziel `example.co.uk`.
  - [ ] Kostenschätzung: 2 Hosts, Ebenen Subdomain + Domain auf derselben registrierbaren Domain → 3 Requests (2 Subdomain, 1 Domain).
- **Testfixture:** Mock `summary` mit 64-Bit-Wert; URL-Liste mit gemischten Hosts.

### D4S-032 Neue und verlorene Backlinks
- **Priorität:** P1
- **Beschreibung:** Für URLs im Umfang werden `bulk_new_lost_backlinks/live` und `bulk_new_lost_referring_domains/live` (je bis 1.000 Ziele) mit `date_from` = heute − Zeitraum aufgerufen. Der Zeitraum ist 7–365 Tage (Standard 30) – die API erlaubt höchstens ein Jahr Rückblick.
- **Eingaben/Konfiguration:** Checkbox „Neue/verlorene Links“ (Standard aus), „Zeitraum (Tage)“; Schwellen für den Filter `Lost Backlinks`: „mindestens verlorene Backlinks“ (Standard 5) und „mehr verloren als gewonnen“ (Standard an).
- **Ausgabe:** `DFS New Backlinks`, `DFS Lost Backlinks`, `DFS New Referring Domains`, `DFS Lost Referring Domains`, `DFS New/Lost Since` (Datum); Filter `Link Metrics: Lost Backlinks`.
- **Akzeptanzkriterien:**
  - [ ] Zeitraum 30 Tage, Testuhr 13.10.2026 → `date_from: "2026-09-13"` in beiden Requests (Mock-Log).
  - [ ] Zeitraum 400 Tage wird im Dialog abgelehnt.
  - [ ] `/a` (neu 7, verloren 3) ist bei Standardschwellen **nicht** im Filter; Mock-URL mit neu 1, verloren 6 ist enthalten.
- **Testfixture:** Mock `bulk_new_lost_backlinks` und `bulk_new_lost_referring_domains`; Testuhr.

### D4S-033 Einheitliches Linkmetrik-Schema
- **Priorität:** P1
- **Beschreibung:** Damit Filter, Issues, Segmente und Exporte unabhängig vom Linkdaten-Anbieter funktionieren, führt OpenSpider anbieterneutrale Spalten ein, die aus einem wählbaren **primären Linkanbieter** gefüllt werden. Die anbieterspezifischen Spalten (INTEG-152 – 154, D4S-030) bleiben daneben bestehen. Zuordnung (jeweils Ebene `Exact URL`, für Domain-Werte Ebene `Domain`):

  | Neutrale Spalte | DFS | Majestic | Ahrefs | Moz |
  |---|---|---|---|---|
  | `Link Backlinks` | `backlinks` | External Backlinks | Backlinks | External Pages Linking |
  | `Link Referring Domains` | `referring_domains` | Referring Domains | Referring Domains | Root Domains Linking |
  | `Link Dofollow Referring Domains %` | D4S-030 | – | aus Dofollow-/Gesamt-Referring-Domains, falls geliefert | (Root Domains Linking − Nofollow Root Domains) / Root Domains Linking |
  | `Link Authority` | `rank` (0–100) | Trust Flow | URL Rating bzw. Domain Rating | Page Authority bzw. Domain Authority |
  | `Link Source` | `DataForSEO` | `Majestic` | `Ahrefs` | `Moz` |

  `Link Authority` ist ausdrücklich **nicht** zwischen Anbietern vergleichbar; der Spaltentooltip nennt die zugrunde liegende Metrik (`Link Authority Metric`, z. B. `DFS Rank`). Ist der primäre Anbieter für eine URL ohne Wert, wird **nicht** auf einen anderen Anbieter ausgewichen (keine Vermischung) **(OS-Festlegung)**.
- **Eingaben/Konfiguration:** Einstellung „Primärer Linkanbieter“ (Standard: der erste verbundene in der Reihenfolge DataForSEO, Ahrefs, Majestic, Moz).
- **Ausgabe:** neutrale Spalten im Tab `Link Metrics` und im Internal-Tab; alle Link-Filter aus Abschnitt G verwenden ausschließlich die neutralen Spalten.
- **Akzeptanzkriterien:**
  - [ ] Primär DataForSEO → `Link Backlinks` = `DFS Backlinks (Exact URL)` für alle URLs; Umstellung auf Ahrefs → Werte aus `Ahrefs Backlinks (Exact URL)`, ohne neuen API-Request.
  - [ ] Filter `Non-200 With Backlinks` liefert mit Ahrefs-Mockdaten und mit DFS-Mockdaten dieselbe URL-Menge, wenn beide Mocks für `/c` Backlinks > 0 melden.
  - [ ] Fehlt bei Primär DataForSEO der Wert für eine URL, bleibt `Link Backlinks` leer, auch wenn Majestic einen Wert hat.
- **Testfixture:** Mocks DFS und Ahrefs mit überlappenden Daten.

### D4S-034 Detail-Tab „Backlinks“ (auf Abruf)
- **Priorität:** P2
- **Beschreibung:** Für eine markierte URL lädt „Backlinks laden“ die einzelnen Backlinks (`POST /v3/backlinks/backlinks/live`, `mode: "as_is"`, `limit` Standard 100, max. 1.000, sortiert nach Rank absteigend; Parameternamen `mode`/`order_by` **(abgeleitet, bei Umsetzung gegen die Endpunkt-Doku prüfen)**) und verweisende Domains (`referring_domains/live`). Erst nach Bestätigung der Kostenschätzung; Ergebnis wird gecacht.
- **Eingaben/Konfiguration:** Schaltfläche im Detail-Tab, Anzahl.
- **Ausgabe:** Spalten `Source URL`, `Source Domain`, `Source Rank`, `Anchor`, `Dofollow` (`Yes`/`No`), `Link Type`, `First Seen`, `Last Seen`, `Is Lost`, `Is Broken`.
- **Akzeptanzkriterien:**
  - [ ] Ohne Klick wird kein `backlinks/live`-Request gestellt (Mock-Log).
  - [ ] Markierung `/a`, Anzahl 100 → 1 Request, Kostenschätzung 0,024 + 100 · 0,000036 = 0,0276 $.
  - [ ] Zweites Öffnen innerhalb der Cache-Gültigkeit → kein Request.
- **Testfixture:** Mock `backlinks/live` mit 120 Zeilen für `/a`.

---

## E SERP API

### D4S-040 Haupt-Keyword je Seite
- **Priorität:** P0
- **Beschreibung:** Für die Ranking-Prüfung braucht jede Seite ein **Haupt-Keyword**. Quellen in einstellbarer Reihenfolge (erste nicht leere gewinnt):
  1. **Liste** – CSV/TSV mit den Spalten `Address` und `Keyword` (Import im Dialog oder per CLI; URL-Abgleich nach INTEG-014),
  2. **Custom Extraction** – erster Wert eines gewählten Extraktors (INTEG-040 ff.), z. B. ein Meta-Tag,
  3. **Labs** – `DFS Top Keyword` aus D4S-020,
  4. [P2] **Search Console** – Suchanfrage mit den meisten Klicks für die URL im GSC-Zeitraum (INTEG-110 ff.).
  Keywords werden getrimmt, Mehrfach-Leerzeichen zusammengefasst und in Kleinbuchstaben verglichen. Seiten ohne Keyword werden nicht abgefragt. Jedes Keyword wird vor der Übernahme geprüft; die Prüfung gilt für alle Quellen gleichermaßen (Liste und Custom Extraction werden ungeprüft geliefert):
  - **Ungültig** → `Skipped (invalid keyword)`: über 700 Zeichen (API-Grenze), nach Normalisierung leer, enthält eine E-Mail-Adresse oder eine URL mit Query-String (D4S-010).
  - **Suchoperator** → standardmäßig `Skipped (operator)`, kein Request: Das Keyword enthält ein Token, das ohne Groß-/Kleinschreibung mit einem der Präfixe `site:`, `inurl:`, `allinurl:`, `intitle:`, `allintitle:`, `intext:`, `allintext:`, `inanchor:`, `allinanchor:`, `filetype:`, `ext:`, `related:`, `cache:`, `info:`, `before:`, `after:`, `around(` beginnt, oder ein in Anführungszeichen gesetztes Wort mit vorangestelltem `-` bzw. ein Token `OR`/`AND` zwischen zwei Begriffen **(OS-Festlegung; Liste in `resources/dataforseo/search_operators.json`, erweiterbar)**. Hintergrund: DataForSEO berechnet solche Anfragen mit dem 5-Fachen des Task-Preises. Mit der Option „Suchoperatoren erlauben“ werden sie gesendet; Kostenschätzung und Budget-Reservierung rechnen dann mit Faktor 5 (D4S-012, D4S-013), und der Schätzdialog weist die Zahl der Operator-Keywords gesondert aus.
  Wird ein Keyword einer höher priorisierten Quelle verworfen, greift **nicht** automatisch die nächste Quelle (das Ergebnis wäre für den Nutzer nicht nachvollziehbar); die Seite erhält den Skip-Status mit der verwerfenden Quelle im Tooltip.
- **Eingaben/Konfiguration:** Modul „SERP“ → „Keyword-Quellen“ (Reihenfolge per Drag & Drop, je Quelle an/aus; Standard: Liste, Custom Extraction, Labs); Extraktor-Auswahl; Listen-Import; „Suchoperatoren erlauben“ (Standard aus).
- **Ausgabe:** `DFS Main Keyword`, `DFS Main Keyword Source` ∈ {`List`, `Extraction`, `Labs`, `GSC`}; Status `Skipped (no keyword)`, `Skipped (operator)`, `Skipped (invalid keyword)`.
- **Akzeptanzkriterien:**
  - [ ] Extraktor auf `meta[name=main-keyword]/@content` → `/a` erhält `widget shop` mit Quelle `Extraction`.
  - [ ] Liste mit Zeile `{BASE}/e;blue widgets` und Labs-Top-Keyword für `/e` → Quelle `List` (höhere Priorität).
  - [ ] `/j` (keine Quelle liefert) → `Skipped (no keyword)`, kein SERP-Request.
  - [ ] Liste mit 3 Zeilen, davon 1 URL nicht im Crawl → Hinweis „1 Listen-URL nicht im Crawl“; im APIs-Modus wird sie trotzdem abgefragt.
  - [ ] Ist die Quelle Labs gewählt, aber das Labs-Modul aus, zeigt der Dialog eine Warnung mit Direktlink zur Labs-Aktivierung.
  - [ ] Liste mit Zeile `{BASE}/g;site:competitor.test widgets` → `/g` `DFS SERP Status Skipped (operator)`, Mock-Log ohne Request mit `site:`; Schätzdialog zeigt „1 Keyword mit Suchoperator übersprungen“.
  - [ ] Dieselbe Liste mit „Suchoperatoren erlauben“ → 1 Request mit dem Keyword; Kostenschätzung und Reservierung dieses Keywords = 5 · Task-Preis (Live, depth 20, ohne Zuschläge: 5 · 0,004 = 0,0200 $).
  - [ ] Keywords `Site:example.com`, `INURL:shop` und `widgets -"cheap"` werden als Operator erkannt; `website builder` und `info desk` nicht (Unit-Test der Erkennung).
  - [ ] Keyword mit 701 Zeichen → `Skipped (invalid keyword)`.
- **Testfixture:** `tests/fixtures/dfs-site/` (Meta-Tag auf `/a`), `tests/fixtures/dfs-site/keywords.csv`, `tests/fixtures/dfs-site/keywords-operators.csv`.

### D4S-041 Live-Ranking je Seite
- **Priorität:** P0
- **Beschreibung:** Für jede eindeutige Kombination (Keyword, Markt, Gerät) wird genau ein Request `POST /v3/serp/google/organic/live/advanced` gestellt (Parameter: `keyword`, Markt, `device`, `depth`, `load_async_ai_overview` gemäß D4S-043); Seiten mit demselben Keyword teilen sich das Ergebnis. Die vollständige Ergebnisliste wird je Keyword in `dfs_serp_results` gespeichert. Auswertung je Seite: Unter allen `organic`-Items wird das erste gesucht, dessen normalisierte URL der Seiten-URL entspricht (`DFS SERP Position` = `rank_group`, `DFS SERP Absolute Position` = `rank_absolute`); unabhängig davon wird das bestplatzierte organische Ergebnis der eigenen Domain als `DFS Ranking URL` ausgewiesen. Nicht gefunden innerhalb `depth` → Position leer, Status `Success`.
- **Eingaben/Konfiguration:** Modul „SERP“ aktivieren; „Gerät“ (`desktop` | `mobile`, Standard `desktop`); „Ergebnistiefe“ (10–200 in Zehnerschritten, Standard 20) **(OS-Festlegung)**.
- **Ausgabe:** Tab `Rankings` (D4S-046 für Details) und Internal-Tab: `DFS Main Keyword`, `DFS Main Keyword Source`, `DFS SERP Position`, `DFS SERP Absolute Position`, `DFS Ranking URL`, `DFS Ranking URL Match` (`Yes`, wenn `DFS Ranking URL` = Seiten-URL; `No` sonst; leer, wenn die eigene Domain nicht rankt), `DFS SERP Depth`, `DFS SERP Device`, `DFS SERP Location`, `DFS SERP Language`, `DFS SERP Mode`, `DFS SERP Source`, `DFS SERP Fetched`, `DFS SERP Status`. Filter im Tab `Rankings`: `All`, `Top 3`, `Top 10`, `Not Ranking`, `Different URL Ranking` (D4S-066), `No Main Keyword`, `Pending`, `Errors`.
- **Akzeptanzkriterien:**
  - [ ] `/a` mit `widget shop` → `DFS SERP Position 2`, `DFS Ranking URL {BASE}/a`, `DFS Ranking URL Match Yes`.
  - [ ] `/f` mit `blue widgets` → `DFS SERP Position 8`, `DFS Ranking URL {BASE}/e`, `DFS Ranking URL Match No`.
  - [ ] `/e` und `/f` mit demselben Keyword → genau 1 SERP-Request (Mock-Log).
  - [ ] Keyword ohne eigenes Ergebnis in Tiefe 20 → Position leer, `DFS Ranking URL` leer, im Filter `Not Ranking`.
  - [ ] Request-Body enthält `depth: 20`, `device: "desktop"` und den Markt; Kosten laut Kostenprotokoll = 2 · 0,002 $ je Keyword, zuzüglich 0,002 $ AI-Overview-Zuschlag nur bei aktiver Option **und** vorhandenem AI Overview (sonst erstattet, D4S-011).
  - [ ] Fehlerfälle über die gemeinsamen Szenarien: `Not connected`, `Budget reached`, `Insufficient funds`, `Error: Timeout nach 120 s`; ein Fehler bei einem Keyword betrifft alle Seiten mit diesem Keyword, keine anderen.
- **Testfixture:** Mock `serp/google/organic/live/advanced` mit den drei Keyword-Antworten und einem Keyword ohne Treffer (`widget repair`).

### D4S-042 SERP-Features
- **Priorität:** P0
- **Beschreibung:** Aus `item_types` des Ergebnisses werden die vorhandenen SERP-Features je Keyword übernommen (z. B. `ai_overview`, `knowledge_graph`, `featured_snippet`, `people_also_ask`, `local_pack`, `video`, `images`, `top_stories`; `organic` und `paid` zählen nicht als Feature). Ein AI Overview innerhalb des Knowledge Graph (`knowledge_graph_ai_overview_item`, D4S-043) ergänzt die Liste um `ai_overview`, auch wenn `item_types` es nicht enthält. Zusätzlich wird festgehalten, ob die Seite selbst in einem Feature-Element erscheint (z. B. als Quelle des Featured Snippets).
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** `DFS SERP Features` (alphabetisch, mit `; ` getrennt), `DFS SERP Feature Count`, `DFS Owned Features` (Features, in denen die Seiten-URL vorkommt); Filter `Has Featured Snippet`, `Has Local Pack`, `Has People Also Ask` sowie je weiterem beobachteten Typ ein dynamischer Filter `SERP Feature: <type>`.
- **Akzeptanzkriterien:**
  - [ ] `widget shop` → `DFS SERP Features = ai_overview; featured_snippet; local_pack; people_also_ask`, `DFS SERP Feature Count 4`.
  - [ ] `blue widgets` → `DFS SERP Features` leer, `DFS SERP Feature Count 0`.
  - [ ] Ein unbekannter neuer `item_types`-Wert (Mock `ai_mode_teaser`) erscheint unverändert in der Liste und erzeugt einen dynamischen Filter, ohne Fehler.
- **Testfixture:** Mock-Antworten wie D4S-041 plus Variante mit unbekanntem Typ.

### D4S-043 AI-Overview-Präsenz und Zitat
- **Priorität:** P0
- **Beschreibung:** Ein AI Overview kann an zwei Stellen der Antwort stehen; beide werden gleich behandelt:
  1. als Top-Level-Item vom Typ `ai_overview`,
  2. als `knowledge_graph_ai_overview_item` innerhalb von `items[]` eines `knowledge_graph`-Items.
  Ist mindestens eine der beiden Varianten vorhanden, gilt das AI Overview als vorhanden; bei beiden werden die Referenzen zusammengeführt (Duplikate nach normalisierter URL einmal gezählt). Mit „Asynchrone AI Overviews laden“ (`load_async_ai_overview: true`, Zuschlag laut D4S-011) werden auch nachgeladene Overviews mit Inhalt geliefert; ohne die Option kann ein Item (beider Varianten) mit `asynchronous_ai_overview: true` ohne Referenzen ankommen. Die Seite gilt als **zitiert**, wenn eine Referenz-URL (`references[].url` auf Item-Ebene oder in den `ai_overview_element`-Einträgen) nach Entfernen des Fragments (`#:~:text=…`) und Normalisierung (INTEG-014) der Seiten-URL entspricht. Die Zahl der Referenzen auf die eigene Domain wird zusätzlich gezählt. Die Fundstelle steht in `DFS AI Overview Placement`.
- **Eingaben/Konfiguration:** „Asynchrone AI Overviews laden“ (Standard an **(OS-Festlegung)**; Hinweis auf Mehrkosten).
- **Ausgabe:** `DFS AI Overview` ∈ {`Yes`, `No`, `Yes (not loaded)`}, `DFS AI Overview Cited` (`Yes`/`No`; leer, wenn kein Overview oder nicht geladen), `DFS AI Overview Own Domain References` (Anzahl), `DFS AI Overview References` (Anzahl gesamt), `DFS AI Overview Placement` ∈ {`Top-level`, `Knowledge Graph`, `Both`} (leer ohne Overview); Filter `AI Overview Present`, `Cited In AI Overview`, `AI Overview Not Cited` (D4S-067); Bulk-Export „AI Overview References“ (D4S-072).
- **Akzeptanzkriterien:**
  - [ ] `/i` mit `what is a widget` → `DFS AI Overview Yes`, `DFS AI Overview Cited Yes`.
  - [ ] `/a` mit `widget shop` → `DFS AI Overview Yes`, `DFS AI Overview Cited No`, `DFS AI Overview Own Domain References 1` (Referenz `{BASE}/i#:~:text=widget` gehört zur eigenen Domain, ist aber nicht `/a`).
  - [ ] Für `/i` würde eine Referenz `{BASE}/i#:~:text=widget` als Zitat gezählt (Fragment entfernt).
  - [ ] Option aus und Mock-Item mit `asynchronous_ai_overview: true` ohne Referenzen → `DFS AI Overview Yes (not loaded)`, `Cited` leer; Request ohne `load_async_ai_overview` (Mock-Log).
  - [ ] Option an → Request mit `load_async_ai_overview: true`; die Kostenschätzung erhöht `Cost Max` je Keyword um `per_task_high` (Live 0,002 $) und lässt `Cost Min` unverändert (D4S-012).
  - [ ] Szenario `kg-aio` für `what is a widget` → `/i` `DFS AI Overview Yes`, `DFS AI Overview Cited Yes`, `DFS AI Overview Placement Knowledge Graph`; `DFS SERP Features` enthält `ai_overview` und `knowledge_graph`.
  - [ ] Antwort mit beiden Varianten und derselben Referenz `{BASE}/i` in beiden → `DFS AI Overview References` zählt sie einmal, `Placement Both`.
  - [ ] `knowledge_graph_ai_overview_item` mit `asynchronous_ai_overview: true` ohne Referenzen bei ausgeschalteter Option → `DFS AI Overview Yes (not loaded)`.
- **Testfixture:** Mock-Antworten mit AI Overview (Referenzen auf Item- und Element-Ebene, mit und ohne Text-Fragment), Szenario `kg-aio`, Variante mit beiden Fundstellen sowie asynchrone Items ohne Inhalt (beide Varianten).

### D4S-044 Ausführungsmodus Live, Standard, Priority
- **Priorität:** P1
- **Beschreibung:** Neben Live (P0) unterstützt das SERP-Modul den günstigeren Standard-Modus (Task-Warteschlange, laut Anbieter im Mittel ca. 5 min, Ziel bis 45 min) und Priority (`priority: 2`, bis ca. 1 min) über D4S-008. Im Modus „Automatisch“ nutzt OpenSpider Live für bis zu 20 Keywords und Standard darüber **(OS-Festlegung)**; Prompt-Tester-artige Einzelabfragen (Neuanforderung einzelner URLs, D4S-018) laufen immer Live.
- **Eingaben/Konfiguration:** „SERP-Modus“: `Automatisch` | `Live` | `Standard` | `Priority` (Standard `Automatisch`, sobald P1 verfügbar; in P0 nur `Live`).
- **Ausgabe:** `DFS SERP Mode`; Status `Pending` bis zum Ergebnis.
- **Akzeptanzkriterien:**
  - [ ] `Automatisch` mit 15 Keywords → 15 Live-Requests; mit 25 Keywords → 1 `task_post` mit 25 Tasks.
  - [ ] Kostenschätzung je Modus: 100 Keywords, depth 10, ohne Zuschlag → Standard 0,06 $, Priority 0,12 $, Live 0,20 $.
  - [ ] Priority setzt `priority: 2` in jedem Task (Mock-Log).
- **Testfixture:** Mock `task_post`/`tasks_ready`/`task_get` (siehe D4S-008).

### D4S-045 Erweiterte Orts- und Geräteoptionen
- **Priorität:** P1
- **Beschreibung:** Das SERP-Modul kann den Markt verfeinern: Ort aus der SERP-Location-Liste (Stadt/Region, `location_code`) oder Koordinate (`location_coordinate` „Breite,Länge,Radius“), Betriebssystem (`os`: desktop `windows`/`macos`, mobile `android`/`ios`) und Google-Domain (`se_domain`, Standard automatisch). Labs und LLM Mentions bleiben beim Land des Markts.
- **Eingaben/Konfiguration:** „SERP-Ort überschreiben“, „Koordinate“, „Betriebssystem“, „Google-Domain“.
- **Ausgabe:** `DFS SERP Location` zeigt den verfeinerten Ort; Tab-Hinweis.
- **Akzeptanzkriterien:**
  - [ ] Ort „Berlin, Germany“ → SERP-Requests mit dessen `location_code`, Labs-Requests weiterhin mit 2276.
  - [ ] Koordinate `52.52,13.40,10` wird als `location_coordinate` gesendet; ungültiges Format wird im Dialog abgelehnt.
  - [ ] Gerät `mobile` mit OS `macos` ist nicht wählbar.
- **Testfixture:** Mock-Location-Liste mit Städten; Mock-Log.

### D4S-046 Detail-Tab „SERP“
- **Priorität:** P1
- **Beschreibung:** Unterer Detail-Tab für die markierte URL: die gespeicherte Ergebnisliste ihres Haupt-Keywords in Reihenfolge, mit Hervorhebung eigener Ergebnisse; darunter die AI-Overview-Referenzen.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Spalten `Rank Absolute`, `Rank Group`, `Type`, `Domain`, `URL`, `Title`, `Is Own Domain`, `Is This Page`; Abschnitt AI Overview mit `Source`, `Domain`, `URL`, `Title`, `Is Own Domain`.
- **Akzeptanzkriterien:**
  - [ ] Markierung `/a` → Zeilen in `rank_absolute`-Reihenfolge, Zeile `{BASE}/a` mit `Is This Page Yes`.
  - [ ] Markierung einer Seite mit `Skipped (no keyword)` → Hinweis „Kein Haupt-Keyword“ statt Tabelle.
- **Testfixture:** Mock-Antworten wie D4S-041.

### D4S-047 Mehrere Keywords je Seite
- **Priorität:** P2
- **Beschreibung:** Statt nur des Haupt-Keywords werden je Seite bis zu N Keywords geprüft (aus Liste mit mehreren Zeilen je URL oder die Top-N-Keywords aus Labs). Die Tab-Spalten zeigen weiterhin das Haupt-Keyword; zusätzlich `DFS Tracked Keywords`, `DFS Avg SERP Position`, `DFS Keywords Ranking Top 10`. Alle Ergebnisse stehen im Bulk-Export „All SERP Rankings“.
- **Eingaben/Konfiguration:** „Keywords je Seite“ (1–20, Standard 1).
- **Ausgabe:** zusätzliche Spalten wie oben.
- **Akzeptanzkriterien:**
  - [ ] N = 3 mit Labs-Quelle für `/a` → SERP-Requests für 3 Keywords; `DFS Tracked Keywords 3`.
  - [ ] `DFS Avg SERP Position` mittelt nur gefundene Positionen; nicht gefundene zählen nicht (Anzahl in Tooltip).
- **Testfixture:** Mock mit SERP-Antworten für alle drei `/a`-Keywords.

### D4S-048 Google AI Mode
- **Priorität:** P2
- **Beschreibung:** Für das Haupt-Keyword wird `POST /v3/serp/google/ai_mode/live/advanced` (bzw. in den Modi Standard/Priority die Task-Variante über D4S-008) mit Keyword, Markt, `device` und `os` aufgerufen. Analog zu D4S-043 wird geprüft, ob die Seite bzw. die eigene Domain in den Referenzen der AI-Mode-Antwort vorkommt; ausgewertet werden alle `references[]` auf allen Ebenen (Item und Elemente). Der Endpunkt kennt kein `depth`. Abrechnung je Task (eine AI-Mode-Antwort): Standard 0,0012 $, Priority 0,0024 $, Live 0,004 $ (Preisseite, 13.10.2026). Die Preisseite AI Mode (FAQ) belegt die Abrechnung je Request = eine AI-Mode-Seite; eine Abrechnung je 20 (Desktop) bzw. 10 (Mobile) Ergebnisse gibt es nicht (geklärt 13.10.2026, Q8). Abweichungen fallen über die gemeldete `cost` auf (D4S-011).
- **Eingaben/Konfiguration:** Checkbox „AI Mode prüfen“ (Standard aus).
- **Ausgabe:** `DFS AI Mode Cited` (`Yes`/`No`), `DFS AI Mode Own Domain References`; eigene Statusspalte `DFS AI Mode Status` mit dem Wertebereich der Modul-Statusspalten (Konventionen).
- **Akzeptanzkriterien:**
  - [ ] Mock-Antwort für `what is a widget` mit Referenz `{BASE}/i` → `/i` `DFS AI Mode Cited Yes`; Request an `…/serp/google/ai_mode/live/advanced` ohne `depth` (Mock-Log).
  - [ ] Ohne aktivierte Option kein Request an den AI-Mode-Endpunkt.
  - [ ] Kostenschätzung Live, 3 Keywords → `Cost Min = Cost Max = 0,0120 $`.
  - [ ] Fehlerfälle über die gemeinsamen Szenarien (nicht verbunden, Budget, 40210, Timeout): betroffen sind nur die AI-Mode-Spalten; die Organic-Spalten aus D4S-041 bleiben unverändert.
- **Testfixture:** Mock `serp/google/ai_mode/live/advanced` (selbst erstellte Antwort nach der dokumentierten Struktur).

---

## F AI Optimization / LLM Mentions

Gemeinsam für diesen Abschnitt: Alle Endpunkte sind Live-only und kosten laut Preiskatalog 0,10 $ je Task plus 0,001 $ je Zeile; deshalb ist jede Funktion einzeln zuschaltbar, und die Kostenschätzung nennt die Anzahl Requests ausdrücklich. Plattformen: `google` (AI Overview) und `chat_gpt`; welche Plattform für den Markt verfügbar ist, bestimmt die LLM-Marktliste (D4S-007). Regel für `platform` **(OS-Festlegung)**: Ist für den Markt nur eine Plattform verfügbar, wird sie immer explizit gesetzt. Sind beide verfügbar, wird `platform` bei Endpunkten, deren Antwort eine Aufschlüsselung `platform[]` je Item enthält und deren Doku „ohne Angabe beide“ eindeutig nennt (`target_metrics`, `top_mentioned_pages`, `top_mentioned_domains`, `search_mentions`), weggelassen – ein Request liefert dann beide Plattformen. `historical` wird ebenso behandelt, weil der Standard „ohne Angabe beide Plattformen“ dort inzwischen belegt ist: bei zwei verfügbaren Plattformen 1 Request je Ziel ohne `platform`, sonst `platform` explizit (**Entscheidung (Fable-Empfehlung)**). Nur bei `multi_target_metrics`, dessen Doku zugleich `google` als Standard und „ohne Angabe beide“ nennt, setzt OpenSpider `platform` immer explizit und stellt je verfügbarer Plattform einen Request. Zuordnung zum Soll-Umfang „Target/Aggregated Metrics“: Endpunkte `target_metrics` (eine Domain) und `multi_target_metrics` (Vergleich). `aggregated_metrics` ist **kein** Endpunkt, sondern ein Antwortfeld: Bei `target_metrics` stehen dort die Kennzahlen der Domain; bei `multi_target_metrics` ist es die Gesamtsumme über alle Gruppen, die Werte je Gruppe stehen in `items[]` (D4S-053, **Entscheidung (Fable-Empfehlung)**).

**Priorität:** Der Soll-Umfang führt LLM Mentions gleichrangig mit Labs, Backlinks und SERP. Abschnitt F ist dennoch vollständig P1, weil (1) ein Request rund das Achtfache eines Labs-Requests kostet (0,10 $ gegenüber 0,012 $) und die Kostenkontrolle aus Abschnitt B daher vorher erprobt sein soll, (2) die Daten überwiegend Domain-Kennzahlen sind und keine P0-Kombination mit Crawl-Daten (Abschnitt G) von ihnen abhängt und (3) die erste Ausbaustufe sich auf die je URL abgefragten Module konzentriert. Die Infrastruktur (Markt, Budget, Cache, Fehlerbehandlung, Mock) ist P0 und deckt F bereits ab, sodass F ohne Umbau nachgezogen werden kann.

**Gemeinsame Fehlerfälle (gelten für D4S-050 – D4S-055; jede Funktion prüft sie mit den Mock-Szenarien der Pflichtabdeckung):**

| Fall | Verhalten | Anzeige |
|---|---|---|
| Nicht verbunden / Zugangsdaten fehlen | kein Request | Seitenleiste/Report: „DataForSEO nicht verbunden“; URL-Spalten (D4S-051) `Not connected` |
| Markt nicht in der LLM-Marktliste | kein Request | `Skipped (market unsupported)` bzw. Hinweis in Seitenleiste/Report |
| Budget erreicht (D4S-013) | kein Request; bei paginierten Abrufen keine weiteren Seiten | `Budget reached`; Report-Kopf „unvollständig (Budget)“ |
| 40210 / 40200 | alle DFS-Module stoppen (D4S-004) | `Insufficient funds` |
| 40204 | nur das LLM-Modul stoppt | `Subscription required` |
| 40202 / 40209 / 5xx / Timeout 180 s | Wiederholung nach INTEG-005 bzw. D4S-005; danach endgültiger Fehler | `Error: <Code> <Meldung>` bzw. `Error: Timeout nach 180 s` |
| Teilfehler bei Paginierung (`offset`) | geladene Seiten bleiben gültig und gecacht; nur fehlende Seiten werden bei Neuanforderung geholt (D4S-006 Nr. 5) | Report-Kopf „unvollständig (<geladen>/<gesamt>)“; URL-Werte nach D4S-051 |
| Teilfehler je Plattform (ein Request je Plattform, D4S-053) | erfolgreiche Plattform wird übernommen | Spalte der fehlgeschlagenen Plattform leer, Status `Error: …` mit Plattform im Tooltip |

### D4S-050 Domain-Erwähnungen
- **Priorität:** P1
- **Beschreibung:** `POST /v3/ai_optimization/llm_mentions/target_metrics/live` mit der eigenen Domain als Entität (`search_scope: "any"`, `include_subdomains: true`) liefert, wie oft die Domain in LLM-Antworten erwähnt oder als Quelle genannt wird, und das zugehörige AI-Suchvolumen – gesamt sowie je Plattform, Land und Sprache, außerdem die häufigsten Quell-Domains. Ein Request je Crawl (Plattform-Regel siehe Abschnittseinleitung); die Kennzahlen stammen aus dem Antwortfeld `aggregated_metrics`. Optional eine zweite Abfrage mit `search_scope: "sources"` für „nur als Quelle zitiert“.
- **Eingaben/Konfiguration:** Modul „LLM Mentions“ → „Domain-Kennzahlen“ (Standard an, wenn Modul aktiv); „Nur Quellenzitate zusätzlich“ (Standard aus).
- **Ausgabe:** Seitenleiste „LLM Mentions“: `Mentions`, `AI Search Volume`, je Plattform `Mentions Google AI Overview`, `Mentions ChatGPT`, Top-10 `Sources Domain` mit `Mentions`; im Crawl-Overview-Export (OUT-080) die Zeilen `DFS LLM Domain Mentions`, `DFS LLM Domain AI Search Volume`.
- **Akzeptanzkriterien:**
  - [ ] Markt USA/en (LLM-Marktliste: `google`, `chat_gpt`) → 1 Request ohne `platform` (beide Plattformen), Anzeige `Mentions Google AI Overview 42`, `Mentions ChatGPT 17`, `Mentions 59`.
  - [ ] Markt Deutschland/de → Request mit `platform: "google"`, `location_code: 2276`, `language_code: "de"`; ChatGPT-Zeile zeigt „nicht verfügbar für diesen Markt“ statt `0`.
  - [ ] Die eigene Domain wird ohne Protokoll und ohne `www.` gesendet, höchstens 63 Zeichen; längere Domains → Status `Error: Domain zu lang für LLM Mentions`.
  - [ ] Fehler 40204 → `Subscription required` nur für das LLM-Modul (siehe Klärung Mindestabnahme).
  - [ ] Gemeinsame Fehlerfälle von Abschnitt F: nicht verbunden → 0 Requests, Seitenleiste „DataForSEO nicht verbunden“; Budget 0,05 $ (< 0,10 $ Reservierung) → 0 Requests, Seitenleiste „Budget erreicht“; Login `mock-low` → `Insufficient funds`; `slow-190` → `Error: Timeout nach 180 s` nach Ausschöpfen der Wiederholungen; `empty` → Seitenleiste „Keine Daten“ (Werte leer), `empty-items` → `Mentions 0`.
- **Testfixture:** Mock `target_metrics` für USA/en und DE/de mit den Szenarien der Pflichtabdeckung.

### D4S-051 Seiten-Erwähnungen
- **Priorität:** P1
- **Beschreibung:** `POST /v3/ai_optimization/llm_mentions/top_mentioned_pages/live` mit der eigenen Domain als Entität (`search_scope: "sources"`, `include_subdomains: true`), `limit: 1000` (API-Standard wäre 100, Maximum 1.000) und `offset` in 1.000er-Schritten (API-Grenze 1.000.000) liefert die meistgenannten Seiten (`items[].page`, `total.mentions`, `total.ai_search_volume`, Aufschlüsselung `platform[]`) in den passenden Antworten. Nur Seiten der eigenen Domain werden den Crawl-URLs zugeordnet (INTEG-014); Seiten der eigenen Domain, die nicht im Crawl sind, sind Orphan-Kandidaten (D4S-065).
  **Zeilenkosten sparen (Q5, teilweise geklärt):** Die Antwortmenge enthält auch fremde, mitzitierte Seiten, die je Zeile kosten. OpenSpider schränkt sie serverseitig ein, sofern der Endpunkt das Feld `page` als String-Filter anbietet: `filters: [["page","like","%://<domain>/%"],"or",["page","like","%.<domain>/%"]]` (zweiter Ausdruck für Subdomains). Ob `page` filterbar ist, liest OpenSpider beim ersten Gebrauch aus `GET /v3/ai_optimization/llm_mentions/available_filters` und speichert es im App-Datenverzeichnis (`available_filters` ist laut Doku kostenlos – verifiziert; ob `page` filterbar ist, zeigt erst die Antwort, Q5); ist es nicht filterbar, filtert OpenSpider clientseitig. `include_pages` (exakte URL-Liste) wird nicht genutzt, weil es keine Orphan-Kandidaten liefert. Der Report „LLM Co-Cited Pages“ (fremde Seiten derselben Antwortmenge) erfordert die ungefilterte Variante und ist deshalb eine eigene Option mit eigener Kostenschätzung.
- **Eingaben/Konfiguration:** „Seiten-Erwähnungen“ (Standard aus), „Max. Seiten“ (100–10.000, Standard 1.000), „Mitzitierte fremde Seiten laden (Report LLM Co-Cited Pages)“ (Standard aus).
- **Ausgabe:** Tab `LLM Mentions`: `Address`, `Indexability`, `DFS LLM Mentions`, `DFS LLM Mentions Google AI Overview`, `DFS LLM Mentions ChatGPT`, `DFS LLM AI Search Volume`, `DFS LLM Source`, `DFS LLM Fetched`, `DFS LLM Status`. Filter `All`, `Mentioned`, `Not Mentioned`, `Mentioned But Non-Indexable`, `Orphan URLs`, `Errors`.
- **Akzeptanzkriterien:**
  - [ ] Mock (`available_filters` meldet `page` als String-Feld) → Request enthält den `page`-Filter mit der eigenen Domain (Mock-Log); ohne `page` in `available_filters` → Request ohne Filter, clientseitige Filterung, gleiches Ergebnis.
  - [ ] Option „Mitzitierte fremde Seiten laden“ an, Mock liefert `{BASE}/i` (9), `{BASE}/a` (3), `{BASE}/orphan` (2), `https://competitor.test/x` (20) → `/i` `DFS LLM Mentions 9`, `/a` `3`; `competitor.test/x` erscheint nur im Report „LLM Co-Cited Pages“; `/orphan` erscheint mit der Option „In DataForSEO entdeckte neue URLs crawlen“ (D4S-025/065) nach Crawl-Analyse im Filter `Orphan URLs`, ohne sie nur im Orphan-Pages-Report.
  - [ ] Gecrawlte URLs ohne Eintrag nach vollständiger Abfrage → `DFS LLM Mentions 0`, Status `Success`; wurde wegen „Max. Seiten“ abgebrochen, bleibt der Wert leer mit Status `Skipped (scope limit)`.
  - [ ] „Max. Seiten“ 2.500 → 3 Requests (`offset` 0/1000/2000), Kostenschätzung obere Grenze 3 · 0,10 + 2.500 · 0,001 = 2,80 $.
  - [ ] Plattformaufteilung je Seite entspricht den `platform`-Einträgen des Items.
  - [ ] Teilfehler (Szenario `partial-offset`, „Max. Seiten“ 2.500, Gesamtzahl laut `total_count` > 1.000): Seiten aus `offset` 0 erhalten ihre Werte und `Success`; gecrawlte URLs ohne Eintrag erhalten **nicht** `0`, sondern `Error: 50000 Seiten-Erwähnungen unvollständig`, weil sie auf einer fehlenden Seite stehen könnten; nach „Fehlgeschlagene neu anfordern“ werden nur `offset` 1000 und 2000 abgefragt.
  - [ ] Weitere Fehlerfälle (gemeinsame Tabelle): nicht verbunden → alle URLs `Not connected`; Budget erreicht vor der ersten Seite → `Budget reached`, zwischen zwei Seiten → wie Teilfehler, aber Status `Budget reached` für URLs ohne Eintrag; `mock-low` → `Insufficient funds`; `slow-190` → `Error: Timeout nach 180 s`; `subscription` → `Subscription required`; `empty` (40102) → alle gecrawlten URLs `No data`; `empty-items` → alle `DFS LLM Mentions 0`, `Success`.
- **Testfixture:** Mock `top_mentioned_pages` mit gemischten Domains und den Szenarien der Pflichtabdeckung inkl. `partial-offset`; Mock `available_filters` in zwei Varianten (mit und ohne `page`).

### D4S-052 Top Domains und Top Pages je Keyword
- **Priorität:** P1
- **Beschreibung:** Für eine Keyword-Menge (Liste oder die Top-N-Keywords der Crawl-Daten nach Suchvolumen aus Labs, Standard N = 20) werden je Keyword `top_mentioned_domains/live` und `top_mentioned_pages/live` mit einer Keyword-Entität (`match_type: "word_match"`) abgefragt (`limit` Standard 10, `order_by: ["mentions,desc"]`; der Feldname für die Sortierung wird wie in D4S-051 aus `available_filters` bestätigt **(Annahme, zu verifizieren)**). **Rückfall ohne `order_by` (Entscheidung (Fable-Empfehlung), Q5):** Antwortet einer der beiden Endpunkte auf `order_by` mit 40501, sendet OpenSpider denselben Request einmal ohne `order_by` erneut und sortiert die Items clientseitig nach `total.mentions` absteigend (bei Gleichstand nach `total.ai_search_volume` absteigend, dann nach Domain bzw. Seite aufsteigend), damit `Rank` deterministisch bleibt. Diese Ausnahme von der Regel „40501 ohne Wiederholung“ (D4S-004) gilt nur für diesen Fall; die Erkenntnis wird je Endpunkt im App-Datenverzeichnis gespeichert, sodass nur der erste Request doppelt anfällt. `Rank` ist keine API-Spalte, sondern die 1-basierte Position eines Items in der so sortierten Antwort (`offset` + Index + 1). Ergebnis: Wer wird bei diesem Thema in LLM-Antworten genannt – und ist die eigene Domain bzw. Seite dabei?
- **Eingaben/Konfiguration:** „Keyword-Analyse“ (Standard aus), „Keyword-Quelle“ (Liste | Labs Top-N), N (1–500), „Ergebnisse je Keyword“ (1–100).
- **Ausgabe:** Reports „LLM Top Domains per Keyword“ (`Keyword`, `Rank`, `Domain`, `Mentions`, `AI Search Volume`, `Is Own Domain`) und „LLM Top Pages per Keyword“ (`Keyword`, `Rank`, `Page`, `Mentions`, `AI Search Volume`, `Is Own Domain`, `In Crawl`); im Tab `LLM Mentions` Spalte `DFS LLM Keyword Top Page Count` (für wie viele analysierte Keywords die URL unter den Top-Seiten ist).
- **Akzeptanzkriterien:**
  - [ ] 20 Keywords → 40 Requests; Kostenschätzung obere Grenze 40 · 0,10 + 40 · 10 · 0,001 = 4,40 $ (Dialog zeigt das vor dem Start).
  - [ ] Keyword `what is a widget` mit Mock-Top-Pages `{BASE}/i` an Rang 2 → Report-Zeile `Is Own Domain Yes`, `In Crawl Yes`; `/i` `DFS LLM Keyword Top Page Count 1`.
  - [ ] Keywords über 250 Zeichen werden übersprungen und im Report-Kopf gezählt.
  - [ ] Jeder Request enthält `order_by` mit absteigender Sortierung nach Erwähnungen (Mock-Log); Mock-Antwort mit 3 Items → Report-`Rank` 1, 2, 3 in Antwortreihenfolge (Unit-Test der Ableitung).
  - [ ] Szenario `order-by-rejected`: Der erste Request mit `order_by` erhält 40501, derselbe Request geht genau einmal ohne `order_by` erneut hinaus, alle weiteren Requests dieses Endpunkts gehen ohne `order_by` (Mock-Log); Items mit `total.mentions` 3, 9, 5 → Report-`Rank` 1 = 9, 2 = 5, 3 = 3; kein `Error: 40501` im Report; das Kostenprotokoll enthält beide Requests.
  - [ ] Teilfehler: Keyword 2 von 2 liefert nach Wiederholungen 5xx → Report enthält die Zeilen für Keyword 1, Kopfzeile „1 von 2 Keywords fehlgeschlagen“; `DFS LLM Keyword Top Page Count` zählt nur erfolgreiche Keywords (Tooltip nennt die Zahl der fehlgeschlagenen); „Fehlgeschlagene neu anfordern“ sendet nur die 2 Requests für Keyword 2.
  - [ ] Budget reicht nur für 10 von 40 Requests → genau 10 Requests (Keywords in Reihenfolge der Quelle, je Keyword beide Endpunkte zusammen), Report-Kopf „unvollständig (Budget)“; nicht verbunden bzw. Markt nicht unterstützt → 0 Requests und Hinweis.
- **Testfixture:** Mock `top_mentioned_domains`/`top_mentioned_pages` für 2 Keywords mit den Szenarien der Pflichtabdeckung.

### D4S-053 Wettbewerbervergleich
- **Priorität:** P1
- **Beschreibung:** `POST /v3/ai_optimization/llm_mentions/multi_target_metrics/live` vergleicht die eigene Domain mit 1–9 Wettbewerbern (die API verlangt 2–10 Gruppen; jede Gruppe = eine Domain, `key` = Domainname) in einem Request je Plattform; `platform` wird immer explizit gesetzt (Abschnittseinleitung), d. h. 2 Requests (`google`, `chat_gpt`), wenn der Markt beide Plattformen anbietet, sonst 1. Die Gruppenwerte stammen aus `items[]` der Antwort (je Eintrag `key` = Domain, `total.mentions`, `total.ai_search_volume`, Aufschlüsselung `platform[]`); `aggregated_metrics` enthält nur die Gesamtsumme des Vergleichs über alle Gruppen und speist ausschließlich die Summenzeile `All Targets` (**Entscheidung (Fable-Empfehlung)**). `Mentions` je Gruppe = Summe der Plattform-Requests; `Share of Mentions %` = `Mentions` der Gruppe / Σ `Mentions` aller Gruppen aus `items[]` · 100 (**OS-Festlegung**, damit sich die Anteile zu 100 % addieren). Optional wird die Menge per Keyword-Entität auf ein Thema eingeschränkt.
- **Eingaben/Konfiguration:** Liste „Wettbewerber-Domains“ (1–9), optional „Thema (Keyword)“.
- **Ausgabe:** Seitenleiste/Report „LLM Share of Voice“: `Target`, `Mentions`, `Mentions Google AI Overview`, `Mentions ChatGPT`, `AI Search Volume`, `Share of Mentions %` (Anteil an der Summe aller Gruppen aus `items[]`, zwei Nachkommastellen); Summenzeile `All Targets` mit `Mentions`, `Mentions Google AI Overview`, `Mentions ChatGPT`, `AI Search Volume` aus `aggregated_metrics`.
- **Akzeptanzkriterien:**
  - [ ] Markt USA/en: eigene Domain + `competitor.test` + `rival.test` → genau 2 Requests mit je 3 Gruppen, einer mit `platform: "google"`, einer mit `platform: "chat_gpt"` (Mock-Log); jede Antwort enthält `items[]` mit drei Einträgen (`key` = Domain; `total.mentions` Google 42 / 60 / 10, ChatGPT 17 / 20 / 3) und `aggregated_metrics` mit der Gesamtsumme (Google 112, ChatGPT 40); Anteil eigene Domain = (42 + 17) / (112 + 40) · 100 = 59 / 152 · 100 = 38.82; Summenzeile `All Targets` `Mentions 152`.
  - [ ] Liefert der Mock in `aggregated_metrics` abweichende Summen (Google 100 statt 112), ändern sich weder die Gruppenwerte noch `Share of Mentions %` (weiter 38.82), nur die Summenzeile `All Targets` (`Mentions 140`).
  - [ ] Markt Deutschland/de → genau 1 Request mit `platform: "google"`; `Mentions ChatGPT` zeigt „nicht verfügbar für diesen Markt“, `Share of Mentions %` wird nur aus Google-Werten berechnet (Hinweis im Report-Kopf).
  - [ ] 0 Wettbewerber → Funktion deaktiviert mit Hinweis „mindestens 1 Wettbewerber“; 10 Wettbewerber werden abgelehnt.
  - [ ] Teilfehler je Plattform: `chat_gpt`-Request mit `5xx-always` → Report zeigt Google-Werte, ChatGPT-Spalte leer, Report-Kopf „ChatGPT: Error: 50000 …“, `Share of Mentions %` aus Google-Werten; Kostenprotokoll enthält beide Requests.
  - [ ] Gemeinsame Fehlerfälle: nicht verbunden, Budget < 0,10 $, `mock-low`, `slow-190`, `subscription` → kein Report-Inhalt, Report-Kopf mit dem jeweiligen Status.
- **Testfixture:** Mock `multi_target_metrics` je Plattform mit den Szenarien der Pflichtabdeckung.

### D4S-054 Historische Entwicklung
- **Priorität:** P2
- **Beschreibung:** `POST /v3/ai_optimization/llm_mentions/historical/live` liefert für die eigene Domain (und die Wettbewerber aus D4S-053, je ein Request) monatliche Werte `mentions` und `ai_search_volume` ab frühestens 2025-08-01. `platform` folgt der Regel der Abschnittseinleitung: Bietet der Markt beide Plattformen an, wird `platform` weggelassen (1 Request je Ziel), sonst explizit gesetzt; die Kostenschätzung rechnet entsprechend mit 1 statt 2 Requests je Ziel (**Entscheidung (Fable-Empfehlung)**). Optional zusätzlich `timeseries_new_lost` für neu hinzugekommene bzw. verlorene Erwähnungen **(Parameter und Felder vor Umsetzung per Probeaufruf zu prüfen, Q8/V-D7)**.
- **Eingaben/Konfiguration:** „Verlauf abrufen“, Zeitraum (`date_from` ≥ 2025-08-01, `date_to`).
- **Ausgabe:** Liniendiagramm in der Seitenleiste „LLM Mentions“; Report „LLM Mentions History“ (`Target`, `Year`, `Month`, `Mentions`, `AI Search Volume`).
- **Akzeptanzkriterien:**
  - [ ] `date_from` vor 2025-08-01 wird im Dialog auf 2025-08-01 korrigiert und so gesendet.
  - [ ] Markt USA/en, eigene Domain + 2 Wettbewerber → genau 3 Requests ohne `platform` (Mock-Log), Kostenvoranschlag `Requests 3` (nicht 6); Markt Deutschland/de → 3 Requests mit `platform: "google"`.
  - [ ] Mock mit 14 Monaten → Report mit 14 Zeilen je Target in chronologischer Reihenfolge.
  - [ ] Teilfehler: eigene Domain ok, ein Wettbewerber `slow-190` → Report enthält die Zeilen der erfolgreichen Targets; für den Wettbewerber eine Kopfzeile `Error: Timeout nach 180 s`; Neuanforderung sendet nur dessen Request.
  - [ ] Gemeinsame Fehlerfälle: nicht verbunden, Markt nicht unterstützt, Budget erreicht, `mock-low`, `subscription` → 0 bzw. keine weiteren Requests, Diagramm zeigt den Status statt einer leeren Linie.
- **Testfixture:** Mock `historical` mit den Szenarien der Pflichtabdeckung.

### D4S-055 Erwähnungs-Details
- **Priorität:** P2
- **Beschreibung:** Für eine markierte URL lädt „Erwähnungen anzeigen“ per `search_mentions/live` die konkreten Fragen und Antworten, in denen die Seite als Quelle vorkommt (Domain-Entität mit `search_scope: "sources"`, clientseitig auf Quellen mit der Seiten-URL gefiltert; `limit` Standard 50), inklusive Plattform, Modell, AI-Suchvolumen und Zeitraum der Beobachtung. Die Fragen und Antworten sind **Inhalte Dritter**; sie werden in `dfs_llm` (und roh in `dfs_cache`) dauerhaft in der Crawl-Datei gespeichert, bis der Nutzer sie löscht (D4S-010 Nr. 4). Vor dem ersten Laden je Crawl weist ein Hinweis darauf hin.
- **Eingaben/Konfiguration:** Schaltfläche im Detail-Tab „LLM Mentions“, Anzahl; „Erwähnungs-Details löschen“ (D4S-010).
- **Ausgabe:** Detail-Tab mit `Platform`, `Model`, `Question`, `Answer` (gekürzt, vollständig im Export), `Source Rank`, `AI Search Volume`, `First Seen`, `Last Seen`.
- **Akzeptanzkriterien:**
  - [ ] Ohne Klick kein `search_mentions`-Request.
  - [ ] Mock mit 3 Erwähnungen, davon 2 mit Quelle `{BASE}/i` → Detail-Tab für `/i` zeigt 2 Zeilen.
  - [ ] Antworten im Markdown-Format werden als Text gespeichert; der Export bewahrt Zeilenumbrüche (korrekt gequotet).
  - [ ] Hinweis auf gespeicherte Fremdinhalte erscheint vor dem ersten Laden je Crawl genau einmal.
  - [ ] Fehlerfälle: nicht verbunden → Schaltfläche deaktiviert mit Tooltip; Kostenschätzung über Budget → kein Request, Hinweis „Budget reicht nicht“; `mock-low` → `Insufficient funds`; `slow-190` → Detail-Tab zeigt `Error: Timeout nach 180 s` und „Erneut versuchen“; `empty` → „Keine Erwähnungen gefunden“; Teilfehler bei Folgeseiten (`partial-offset`, Anzahl > 1.000) → geladene Zeilen bleiben sichtbar, Hinweis „unvollständig“.
- **Testfixture:** Mock `search_mentions` mit den Szenarien der Pflichtabdeckung.

---

## G Kombinationen mit Crawl-Daten & Issues

Alle Filter dieses Abschnitts werden in der Crawl-Analyse berechnet (siehe Konventionen). Fehlt eine Voraussetzung (Modul aus, nötige Umfangsoption aus), zeigt der Filter den Hinweis „Voraussetzung fehlt: <Modul/Option>“ statt `0`, und das zugehörige Issue erscheint nicht im Issues-Tab (UI-062).

### D4S-060 Nicht indexierbar, aber mit Rankings
- **Priorität:** P0
- **Beschreibung:** Seiten, die laut Crawl nicht indexierbar sind (noindex, Canonical auf andere URL, robots-gesperrt mit Antwort usw.), laut DataForSEO aber für Keywords ranken – ein Hinweis auf eine ungewollte Sperre oder auf Rankings, die bei Umsetzung der Sperre verloren gehen. Datenquelle: Labs je URL (erfordert die Umfangsoption „Auch nicht indexierbare URLs“, D4S-017) oder Domain-Abzug (D4S-023, liefert die Daten ohne Zusatzoption).
- **Eingaben/Konfiguration:** „Mindest-Traffic für den Filter“ (Standard 0 = jedes Ranking zählt).
- **Ausgabe:** Filter `Organic Keywords: Non-Indexable With Rankings` (Bedingung `Indexability = Non-Indexable` und `DFS Ranked Keywords ≥ 1` und `DFS Est. Traffic ≥ Schwelle`); Spalten zusätzlich `Indexability Status`, `DFS Est. Traffic`, `DFS Top Keyword`.
- **Akzeptanzkriterien:**
  - [ ] Mit Umfangsoption an: `/b` (noindex, 1 Keyword, ETV 25) ist enthalten; `/h` (Canonical auf `/a`, 0 Keywords) nicht.
  - [ ] Mit Umfangsoption aus und ohne Domain-Abzug: Hinweis „Voraussetzung fehlt: Labs – Auch nicht indexierbare URLs“; kein Issue.
  - [ ] Mindest-Traffic 30 → `/b` nicht enthalten.
- **Testfixture:** `tests/fixtures/dfs-site/` + Mock.

### D4S-061 Fehlerseiten und nicht indexierbare Seiten mit Backlinks
- **Priorität:** P0
- **Beschreibung:** Zwei Filter im Tab `Link Metrics` auf Basis der neutralen Linkspalten (D4S-033; bis zu deren Umsetzung auf Basis von `DFS Backlinks (Exact URL)`): Seiten mit Fehlerstatus und externen Backlinks verschenken Linkwirkung; nicht indexierbare Seiten mit Backlinks ebenso. Erfordert für das Backlinks-Modul die Umfangsoptionen „Auch Nicht-200-URLs“ bzw. „Auch nicht indexierbare URLs“.
- **Eingaben/Konfiguration:** „Mindestzahl verweisender Domains“ (Standard 1).
- **Ausgabe:** Filter `Link Metrics: Non-200 With Backlinks` (Status 4xx, 5xx oder keine Antwort, und `Link Referring Domains ≥ Schwelle`) und `Link Metrics: Non-Indexable With Backlinks` (Status 200, `Indexability = Non-Indexable`, `Link Referring Domains ≥ Schwelle`); Spalten zusätzlich `Status Code`, `Indexability Status`, `Inlinks`. Bulk-Export „Backlinks To Error Pages“ (D4S-072).
- **Akzeptanzkriterien:**
  - [ ] `/c` (404, 12 RD) ist in `Non-200 With Backlinks`; `/d` (301) nicht (siehe D4S-062).
  - [ ] `/b` (noindex, 5 RD) ist in `Non-Indexable With Backlinks`; mit Schwelle 6 nicht.
  - [ ] Ohne die Umfangsoptionen zeigen beide Filter den Voraussetzungs-Hinweis.
- **Testfixture:** `tests/fixtures/dfs-site/` + Mock `bulk_pages_summary`.

### D4S-062 Weiterleitungen mit Backlinks
- **Priorität:** P1
- **Beschreibung:** 3xx-URLs mit externen Backlinks. Endet die Weiterleitungskette (Kettenauswertung aus 01/OUT-082) nicht auf einer indexierbaren 200-Seite oder hat sie mehr als einen Sprung, ist die Linkwirkung gefährdet.
- **Eingaben/Konfiguration:** Schwelle wie D4S-061.
- **Ausgabe:** Filter `Link Metrics: Redirect With Backlinks` (alle 3xx mit Backlinks) und `Link Metrics: Redirect To Error With Backlinks` (Teilmenge: Ziel nicht 200/indexierbar oder Kette > 1 Sprung); Spalten zusätzlich `Redirect URL`, `Final Address`, `Final Status Code`, `Chain Length`.
- **Akzeptanzkriterien:**
  - [ ] `/d` (301 → `/a`, 3 RD) ist in `Redirect With Backlinks`, nicht in `Redirect To Error With Backlinks`.
  - [ ] `/d2` (301 → `/c`, 404) mit 2 RD ist in beiden Filtern.
- **Testfixture:** `tests/fixtures/dfs-site/`; Mock.

### D4S-063 Keyword-Kannibalisierung
- **Priorität:** P0
- **Beschreibung:** Mehrere indexierbare URLs derselben Site ranken für dasselbe Keyword im selben Markt. Grundlage sind die gespeicherten Keyword-Zeilen (D4S-020 bzw. Domain-Abzug D4S-023, dann vollständiger). Eine **Kannibalisierungsgruppe** ist ein Keyword mit mindestens 2 verschiedenen indexierbaren Crawl-URLs (nach INTEG-014 normalisiert), die jeweils mit Position ≤ Maximalposition ranken, sofern das Suchvolumen ≥ Mindestvolumen ist. In jeder Gruppe ist die URL mit dem höchsten `etv` `Primary` (Gleichstand: bessere Position, dann alphabetisch), alle anderen `Secondary`. Optional muss die zweitstärkste URL einen Mindestanteil am Traffic der Gruppe haben, um Rauschen auszublenden.
- **Eingaben/Konfiguration:** „Maximalposition“ (1–100, Standard 20), „Mindest-Suchvolumen“ (Standard 10), „Mindestanteil der zweitstärksten URL in %“ (0–50, Standard 0) **(OS-Festlegung)**.
- **Ausgabe:** Spalte `DFS Cannibalized Keywords` (Anzahl Gruppen, an denen die URL beteiligt ist); Filter `Organic Keywords: Cannibalization` (`DFS Cannibalized Keywords ≥ 1`); Detail-Tab-Spalte `Cannibalized` (D4S-021); Report „Keyword Cannibalization“ mit einer Zeile je (Keyword, URL): `Keyword`, `Search Volume`, `URLs In Group`, `Address`, `Role`, `Position`, `Est. Traffic`, `Traffic Share %`, `Indexability`, `Title 1`.
- **Akzeptanzkriterien:**
  - [ ] `blue widgets` (Vol. 2.000): `/e` Pos. 3, ETV 180 und `/f` Pos. 8, ETV 40 → eine Gruppe; `/e` `Primary` mit `Traffic Share % 81.82`, `/f` `Secondary` mit `18.18`; beide `DFS Cannibalized Keywords 1`.
  - [ ] Maximalposition 5 → keine Gruppe (Pos. 8 fällt heraus).
  - [ ] Mindestanteil 20 % → keine Gruppe (18,18 % < 20 %).
  - [ ] Nicht indexierbare URLs (z. B. `/b` mit gleichem Keyword in einer Fixture-Variante) bilden keine Gruppe mit indexierbaren URLs.
  - [ ] Die Berechnung über 1.000.000 Keyword-Zeilen dauert < 60 s (Lasttest, synthetische Zeilen).
- **Testfixture:** `tests/fixtures/dfs-site/` + Mock; synthetischer Datensatz für den Lasttest.

### D4S-064 Traffic-Seiten mit wenigen internen Links
- **Priorität:** P1
- **Beschreibung:** Seiten, die laut DataForSEO nennenswerten organischen Traffic haben, intern aber kaum verlinkt oder tief vergraben sind – Kandidaten für eine stärkere interne Verlinkung.
- **Eingaben/Konfiguration:** „Mindest-Traffic“ (`DFS Est. Traffic`, Standard 10), „Max. Unique Inlinks“ (Standard 1), optional „Min. Crawl-Tiefe“ (Standard aus).
- **Ausgabe:** Filter `Organic Keywords: High Traffic Low Inlinks`; Spalten zusätzlich `Unique Inlinks`, `Crawl Depth`, `Link Score` (falls berechnet).
- **Akzeptanzkriterien:**
  - [ ] `/g` (ETV 1.500, 1 Unique Inlink, Tiefe 4) ist enthalten; `/a` (3 Unique Inlinks) nicht.
  - [ ] „Min. Crawl-Tiefe“ 5 → `/g` nicht enthalten.
- **Testfixture:** `tests/fixtures/dfs-site/` + Mock.

### D4S-065 Orphan-Seiten mit Rankings oder Erwähnungen
- **Priorität:** P1
- **Beschreibung:** URLs der eigenen Domain, die DataForSEO als rankend (Domain-Abzug D4S-023, Relevant Pages D4S-025, Ranking-URL aus SERP D4S-041) oder als LLM-Quelle (D4S-051) kennt, die der Crawl aber nicht über interne Links gefunden hat. Verhalten analog INTEG-015: Mit „In DataForSEO entdeckte neue URLs crawlen“ werden sie gecrawlt und erscheinen im Filter; ohne die Option nur im Orphan-Pages-Report (OUT-099) mit Quelle.
- **Eingaben/Konfiguration:** Option aus D4S-025 (gilt für alle DFS-Quellen).
- **Ausgabe:** Filter `Organic Keywords: Orphan URLs` und `LLM Mentions: Orphan URLs`; Quellen im OUT-099-Report `DataForSEO Labs`, `DataForSEO SERP`, `DataForSEO LLM Mentions`.
- **Akzeptanzkriterien:**
  - [ ] Mit Option: `/orphan` wird gecrawlt und ist nach Crawl-Analyse in beiden Orphan-Filtern (Labs-Quelle Relevant Pages, LLM-Quelle Top Pages).
  - [ ] Ohne Option: `/orphan` nicht gecrawlt (Fixture-Server-Log), aber im Orphan-Pages-Report mit beiden Quellen.
  - [ ] Fremde Hosts werden nie gecrawlt.
- **Testfixture:** `tests/fixtures/dfs-site/` + Mocks `relevant_pages`, `top_mentioned_pages`.

### D4S-066 Andere URL rankt für das Haupt-Keyword
- **Priorität:** P1
- **Beschreibung:** Für das Haupt-Keyword einer Seite rankt in der Live-SERP eine andere URL der eigenen Domain besser (oder ausschließlich) – das gewünschte Ziel wird verfehlt. Besonders kritisch, wenn die rankende URL nicht indexierbar oder eine Weiterleitung ist.
- **Eingaben/Konfiguration:** „Nur wenn die Seite selbst nicht in den Top N rankt“ (Standard aus, N = 10).
- **Ausgabe:** Filter `Rankings: Different URL Ranking` (`DFS Ranking URL Match = No`); Spalten zusätzlich `DFS Ranking URL Indexability`, `DFS Ranking URL Status Code` (aus Crawl-Daten der rankenden URL, leer, wenn nicht gecrawlt).
- **Akzeptanzkriterien:**
  - [ ] `/f` (`blue widgets`, rankende URL `/e`) ist enthalten mit `DFS Ranking URL Indexability = Indexable`.
  - [ ] `/a` (`widget shop`, rankt selbst) ist nicht enthalten.
  - [ ] Option „nur wenn nicht in Top 10“ an → `/f` (selbst Pos. 8) nicht enthalten.
- **Testfixture:** `tests/fixtures/dfs-site/` + SERP-Mock.

### D4S-067 Top-10-Ranking ohne AI-Overview-Zitat
- **Priorität:** P1
- **Beschreibung:** Die Seite rankt für ihr Haupt-Keyword in den Top 10, die SERP zeigt ein AI Overview, die Seite wird darin aber nicht zitiert.
- **Eingaben/Konfiguration:** „Maximalposition“ (Standard 10).
- **Ausgabe:** Filter `Rankings: AI Overview Not Cited` (`DFS SERP Position ≤ N`, `DFS AI Overview = Yes`, `DFS AI Overview Cited = No`); nicht wertender Filter `Rankings: Cited In AI Overview`.
- **Akzeptanzkriterien:**
  - [ ] `/a` (Pos. 2, Overview vorhanden, nicht zitiert) ist in `AI Overview Not Cited`; `/i` ist in `Cited In AI Overview`.
  - [ ] Seiten mit `DFS AI Overview = Yes (not loaded)` sind in keinem der beiden Filter.
- **Testfixture:** SERP-Mock wie D4S-043.

### D4S-068 Haupt-Keyword fehlt in Title oder H1
- **Priorität:** P2
- **Beschreibung:** Alle Wörter des Haupt-Keywords (Unicode-NFC, Groß-/Kleinschreibung ignoriert, Reihenfolge egal) kommen weder in `Title 1` noch in `H1-1` vor.
- **Eingaben/Konfiguration:** „Prüfen in“: Title | H1 | beide (Standard beide = Befund, wenn in keinem der beiden enthalten).
- **Ausgabe:** Filter `Organic Keywords: Main Keyword Not In Title`.
- **Akzeptanzkriterien:**
  - [ ] Title „Widget Shop – Example“ und Keyword `widget shop` → nicht im Filter; Title „Startseite“, H1 „Willkommen“ → im Filter.
  - [ ] Keyword `größe widget` gegen Title „Grösse Widget“ → im Filter (keine Umlaut-Faltung) **(OS-Festlegung)**.
- **Testfixture:** Fixture-Variante mit angepassten Titles.

### D4S-069 Issue-Katalog des Bereichs
- **Priorität:** P0
- **Beschreibung:** Die Filter aus Abschnitt G (und `Lost Backlinks` aus D4S-032) sind Befunde im Sinne von UI-063 und erscheinen im Issues-Tab mit Typ und Priorität nach den dort festgelegten Regeln. Für Befunde, deren Definition nicht indexierbare URLs voraussetzt (D4S-060, D4S-061 zweiter Filter), wirkt die Option „Nicht indexierbare URLs bei Issues ignorieren“ (TECH-009) nicht **(OS-Festlegung)**.

  | Issue Name | Typ | Priorität | Regeln | Priorität der Funktion |
  |---|---|---|---|---|
  | `Organic Keywords: Non-Indexable With Rankings` | Warning | High | T-W3, P-H1 | P0 |
  | `Link Metrics: Non-200 With Backlinks` | Issue | High | T-I1, P-H1 | P0 |
  | `Link Metrics: Non-Indexable With Backlinks` | Warning | High | T-W3, P-H1 | P0 |
  | `Organic Keywords: Cannibalization` | Warning | Medium | T-W3, P-M1 | P0 |
  | `Link Metrics: Redirect To Error With Backlinks` | Issue | High | T-I1, P-H1 | P1 |
  | `Link Metrics: Redirect With Backlinks` | Opportunity | Low | T-O2, P-L1 | P1 |
  | `Link Metrics: Lost Backlinks` | Warning | Low | T-W3, P-L1 | P1 |
  | `Organic Keywords: High Traffic Low Inlinks` | Opportunity | Medium | T-O2, P-M2 | P1 |
  | `Organic Keywords: Orphan URLs` | Warning | Medium | T-W3, P-M2 | P1 |
  | `LLM Mentions: Orphan URLs` | Warning | Medium | T-W3, P-M2 | P1 |
  | `Rankings: Different URL Ranking` | Warning | High | T-W3, P-H2 | P1 |
  | `Rankings: AI Overview Not Cited` | Opportunity | Low | T-O2, P-L1 | P1 |
  | `Organic Keywords: Main Keyword Not In Title` | Opportunity | Low | T-O2, P-L1 | P2 |

  Beschreibungen und Empfehlungen im Issues-Tab (UI-065) sind eigenständige OpenSpider-Texte. Reine Mengenfilter (`With Rankings`, `Top 3`, `Cited In AI Overview`, `Mentioned` …) sind keine Befunde.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Zeilen im Issues-Tab; Issues-Export (OUT-063) mit denselben Namen.
- **Akzeptanzkriterien:**
  - [ ] Volllauf der Fixture-Site mit allen Modulen, allen Umfangsoptionen und Orphan-Crawl → Issues-Tab enthält genau die Zeilen aus `tests/golden/dfs-site/issues.json` in der Sortierung nach UI-062.
  - [ ] Ein Unit-Test prüft Typ und Priorität jeder Zeile gegen die UI-063-Regelcodes der Tabelle.
  - [ ] Ohne aktive DFS-Module erscheint keines dieser Issues.
- **Testfixture:** `tests/fixtures/dfs-site/` + alle Mocks; Golden-Datei.

---

## H Ausgabe & Betrieb

### D4S-070 Internal-Tab und APIs-Modus
- **Priorität:** P0
- **Beschreibung:** Alle DFS-Spalten aktiver Module erscheinen zusätzlich im Internal-Tab (INTEG-013). Im APIs-Modus (INTEG-012) arbeiten alle Module ohne Crawl: Labs und Backlinks je Listen-URL, SERP mit Keyword-Liste (D4S-040), LLM-Seitenerwähnungen mit Zuordnung zu den Listen-URLs. Filter, die Crawl-Daten brauchen (Indexierbarkeit, Inlinks, Statuscodes), zeigen im APIs-Modus den Hinweis „im APIs-Modus nicht verfügbar“.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Spalten wie in den Modulen.
- **Akzeptanzkriterien:**
  - [ ] Für jede Spalte eines DFS-Tabs existiert im Internal-Tab eine gleichnamige Spalte mit identischen Werten; ohne aktive DFS-Module keine DFS-Spalten.
  - [ ] APIs-Modus mit `tests/fixtures/dfs-apis-list.txt` → Fixture-Server registriert 0 Zugriffe; Labs- und Backlinks-Spalten der 4 eindeutigen URLs sind gefüllt.
  - [ ] Im APIs-Modus zeigt `Organic Keywords: Non-Indexable With Rankings` den Hinweis statt `0`.
- **Testfixture:** `tests/fixtures/dfs-site/`, `tests/fixtures/dfs-apis-list.txt` + Mock.

### D4S-071 Fortschritt und Kostenanzeige
- **Priorität:** P0
- **Beschreibung:** DataForSEO nutzt die API-Warteschlange und die Seitenleiste „API“ (INTEG-007) mit je einer Zeile pro Modul. Zusätzlich zeigt das gemeinsame Statusleistenfeld `Kosten` (UI-013) während eines Laufs die laufenden Kosten gegen das Crawl-Budget (Tooltip mit DataForSEO-Anteil), und nach Abschluss erscheint eine Zusammenfassung (Kosten je Modul, URLs mit Daten, Fehler, gestoppte Module mit Grund). [P1] Die Abschlussmail (OUT-282) enthält eine Zeile „DataForSEO: <Kosten> $ (Budget <b> $)“.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Seitenleiste „API“ mit Zeilen `DataForSEO Labs`, `DataForSEO Backlinks`, `DataForSEO SERP`, `DataForSEO LLM Mentions` und Spalten `Progress`, `Errors`, `Cost (USD)`; Statusleistenfeld `Kosten` im Format von UI-013 (führend, **Entscheidung (Fable-Empfehlung)**): „Kosten: 0,4123 / 5,0000 USD“ bei UI-Sprache Deutsch (Tooltip „DataForSEO: 0,4123 USD“); Zusammenfassungsdialog.
- **Akzeptanzkriterien:**
  - [ ] Nach einem Lauf stimmt `Cost (USD)` je Modul mit der Summe des Kostenprotokolls (D4S-014) für dieses Modul überein.
  - [ ] `Errors` zählt URLs mit endgültigem Fehlerstatus einschließlich `Insufficient funds`, `Quota exceeded`, `Subscription required`, nicht aber `Budget reached` und `Skipped (…)`; `Budget reached` wird separat als „<n> wegen Budget ohne Daten“ ausgewiesen.
  - [ ] Wird ein Modul gestoppt (D4S-004), zeigt seine Zeile den Grund; die übrigen Module laufen sichtbar weiter.
  - [ ] Die Statusleiste aktualisiert sich höchstens 10-mal je Sekunde (UI-018/PERF-004).
- **Testfixture:** Mock-Lauf mit gemischten Ergebnissen (Szenarien `partial`, `subscription` für Backlinks, Budget 0,05 $ für Labs).

### D4S-072 Bulk-Exporte und Reports
- **Priorität:** P1
- **Beschreibung:** Neue Einträge im Bulk-Export- und Report-Menü (Format, Dateinamen und Zahlenformat nach OUT-001 ff.):
  - Bulk-Export `DataForSEO: All Ranked Keywords` – je (URL, Keyword) eine Zeile mit den Spalten des Detail-Tabs D4S-021.
  - Bulk-Export `DataForSEO: All SERP Results` – je (Keyword, Markt, Gerät, Ergebnis) `Keyword`, `Location`, `Language`, `Device`, `Rank Absolute`, `Rank Group`, `Type`, `Domain`, `URL`, `Title`, `Is Own Domain`, `Matched Address`.
  - Bulk-Export `DataForSEO: AI Overview References` – `Keyword`, `Location`, `Device`, `Reference Position`, `Source`, `Domain`, `URL`, `Title`, `Is Own Domain`, `Matched Address`.
  - Bulk-Export `DataForSEO: Backlinks To Error Pages` – Zeilen aus `Non-200 With Backlinks` und `Redirect To Error With Backlinks` mit `Address`, `Status Code`, `Final Address`, `Final Status Code`, `Link Backlinks`, `Link Referring Domains`, `Link Source`.
  - Reports `Keyword Cannibalization` (D4S-063), `DataForSEO Relevant Pages` (D4S-025), `LLM Share of Voice` (D4S-053), `LLM Top Domains per Keyword` und `LLM Top Pages per Keyword` (D4S-052), `LLM Co-Cited Pages` (D4S-051), `LLM Mentions History` (D4S-054, P2), `DataForSEO Cost Log` (D4S-014).
  Alle Exporte enthalten die Herkunftsspalte des Moduls, damit Sandbox- und Cache-Daten erkennbar bleiben.
- **Eingaben/Konfiguration:** Menüs „Bulk-Export → DataForSEO“ und „Reports → DataForSEO“; CLI-Namen `DataForSEO:<Eintrag>` (D4S-073).
- **Ausgabe:** Dateien `dataforseo_all_ranked_keywords.csv` usw. (Namenskonvention OUT-008).
- **Akzeptanzkriterien:**
  - [ ] `All Ranked Keywords` der Fixture-Site hat 3 + 1 + 1 + 1 + 1 = 7 Zeilen (Umfang Standard: `/a`, `/e`, `/f`, `/g`, `/i`) und entspricht `tests/golden/dfs-site/all_ranked_keywords.csv` (nach Normalisierung von Zeitstempeln).
  - [ ] `AI Overview References` enthält die Referenz `{BASE}/i#:~:text=widget` im Feld `URL` unverändert und `Matched Address = {BASE}/i`.
  - [ ] `Keyword Cannibalization` enthält 2 Zeilen (`/e` Primary, `/f` Secondary).
  - [ ] Ein Export ohne Daten (Modul aus) wird gemäß OUT-012 übersprungen bzw. mit Kopfzeile erzeugt.
  - [ ] Der Cost-Log-Export enthält keine Zugangsdaten.
- **Testfixture:** `tests/fixtures/dfs-site/` + Mocks; Golden-Dateien unter `tests/golden/dfs-site/`.

### D4S-073 CLI-Optionen
- **Priorität:** P1
- **Beschreibung:** Ergänzung zu OUT-201 (führend für API-Optionen der CLI):
  - `--use-dataforseo "<modul,…>"` mit Modulen `labs`, `backlinks`, `serp`, `llm`; weitere Moduloptionen kommen aus dem Konfigurationsprofil (`--config`, OUT-186).
  - `--dfs-location <code|name>`, `--dfs-language <code>` (überschreiben den Markt aus dem Profil), `--dfs-device desktop|mobile`, `--dfs-serp-mode auto|live|standard|priority`, `--dfs-keywords <datei>` (Liste für D4S-040).
  - `--dfs-budget <usd>` (Teilbudget DataForSEO; das gemeinsame Crawl-Budget setzt `--api-budget`, OUT-201), `--dfs-max-cache-age <tage>`, `--dfs-cache-from previous|<crawl-id>|none` (D4S-016).
  - `--dfs-sandbox` (D4S-003).
  - `--dfs-query-strings skip|allowed|all` und `--dfs-allowed-params <p1,p2>` (D4S-010, Standard `skip`); `--dfs-allow-operators` (D4S-040, Standard aus).
  - `--dfs-estimate-only`: führt den Crawl (bzw. lädt ihn mit `--load-crawl`) **ohne** kostenpflichtige DFS-Requests aus und schreibt die Kostenschätzung als JSON auf stdout: `{"modules":[{"module","requests","rows_max","cost_min_usd","cost_max_usd","cached"}],"total_min_usd","total_max_usd","balance_usd","budget_usd"}`.
  - Zugangsdaten aus dem Schlüsselbund oder aus `OPENSPIDER_DATAFORSEO_LOGIN` / `OPENSPIDER_DATAFORSEO_PASSWORD`; die einmalige Bestätigung aus D4S-010 entfällt in der CLI, wenn `--dfs-accept-data-transfer` gesetzt ist oder die Bestätigung in der App bereits erteilt wurde.
  - `--progress json` (OUT-205) ergänzt die Felder `dfs_cost_usd`, `dfs_budget_usd`, `dfs_pending`.
  Exit-Codes nach OUT-204: fehlende Zugangsdaten, unbekanntes Modul, ungültiger Markt oder 40100/40104/40201 beim Verbindungstest → `2` vor dem Crawl; Guthaben < `Cost Min` → `2` (mit `--dfs-allow-partial` Start trotzdem); `Budget reached`, gestoppte Module oder Fehler einzelner URLs → `0` mit Warnung auf stderr bzw. `4` mit `--strict`.
- **Eingaben/Konfiguration:** wie oben.
- **Ausgabe:** wie GUI-Lauf; Exporte über `--bulk-export "DataForSEO:All Ranked Keywords"` und `--save-report "DataForSEO:Keyword Cannibalization"`.
- **Akzeptanzkriterien:**
  - [ ] `--use-dataforseo labs` ohne Zugangsdaten → Exit-Code 2, stderr nennt die fehlenden Variablen; kein Crawl-Request an die Fixture-Site.
  - [ ] `--use-dataforseo foo` → Exit-Code 2 („unbekanntes Modul foo; erlaubt: labs, backlinks, serp, llm“).
  - [ ] `--dfs-estimate-only` gegen den Mock → 0 kostenpflichtige Requests (Mock-Log), gültiges JSON auf stdout mit den Werten aus D4S-012, Exit-Code 0.
  - [ ] Login `mock-low` und Schätzung `Cost Min` > Guthaben → Exit-Code 2; mit `--dfs-allow-partial` Start und später `Insufficient funds` bei offenen URLs, Exit-Code 0 (bzw. 4 mit `--strict`).
  - [ ] Passwort aus der Umgebungsvariable erscheint in keinem Log und keiner Ausgabe (OUT-243).
  - [ ] Alle Bulk-Export- und Report-Namen aus D4S-072 werden vor dem Crawl validiert (OUT-203).
- **Testfixture:** CLI-Integrationstests gegen `os-testsite` + Mock.

### D4S-074 Scheduling
- **Priorität:** P1
- **Beschreibung:** Ergänzung zu OUT-164 (Integrationsauswahl geplanter Aufgaben): Eine Aufgabe kann DataForSEO-Module aktivieren und hat ein eigenes **Budget je Lauf** sowie optional ein **Monatsbudget je Aufgabe** (Summe der Laufkosten im laufenden Kalendermonat laut Laufhistorie). Reicht das verbleibende Monatsbudget nicht für `Cost Min`, läuft der Crawl ohne DFS-Module, und die Laufhistorie vermerkt „DataForSEO übersprungen: Monatsbudget erreicht“. Standardmäßig wird der Cache des vorherigen Laufs derselben Aufgabe übernommen (D4S-016). Exporte starten erst, wenn auch offene SERP-Tasks abgeschlossen oder abgelaufen sind (D4S-008, INTEG-017).
- **Eingaben/Konfiguration:** Aufgabe → „API-Datenquellen“ → DataForSEO: Module, Budget je Lauf, Monatsbudget, Cache-Übernahme.
- **Ausgabe:** Laufhistorie (OUT-169) mit Spalte `DataForSEO Cost (USD)`.
- **Akzeptanzkriterien:**
  - [ ] Aufgabe mit aktivem DFS, aber ohne Zugangsdaten im Schlüsselbund → Gültigkeitsprüfung (OUT-168) meldet „Invalid: DataForSEO-Zugangsdaten fehlen“.
  - [ ] Monatsbudget 1,00 $, bisherige Läufe 0,95 $, `Cost Min` des nächsten Laufs 0,12 $ → Lauf ohne DFS-Module, Vermerk in der Historie, Mock-Log ohne kostenpflichtige Requests.
  - [ ] Zweiter Lauf am selben Tag übernimmt den Cache und verursacht für Labs und Backlinks 0 Requests.
  - [ ] Laufhistorie zeigt je Lauf die Kosten laut Kostenprotokoll.
- **Testfixture:** Aufgaben-Runner-Test mit Testuhr und Mock.

### D4S-075 Positionsverlauf im Crawl-Vergleich
- **Priorität:** P2
- **Beschreibung:** Im Vergleichsmodus (OUT-140 ff.) erhalten die Tabs `Rankings` und `Organic Keywords` Vergleichsspalten; zusammen mit geplanten Aufgaben und Auto Compare (OUT-152) entsteht ein einfaches Rank-Tracking. Verglichen wird je URL und Haupt-Keyword (gleicher Markt und Gerät vorausgesetzt; sonst Hinweis „nicht vergleichbar“).
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** `DFS SERP Position (Previous)`, `DFS SERP Position Change` (positiv = verbessert), `DFS Est. Traffic (Previous)`, `DFS Est. Traffic Change`, `DFS Ranked Keywords Change`; Filter `Position Improved`, `Position Declined`, `New Ranking`, `Lost Ranking`, `AI Overview Citation Gained`, `AI Overview Citation Lost`.
- **Akzeptanzkriterien:**
  - [ ] Vorher Pos. 8, jetzt Pos. 3 → `DFS SERP Position Change +5`, Filter `Position Improved`.
  - [ ] Vorher Pos. 4, jetzt nicht gefunden → `Lost Ranking`.
  - [ ] Unterschiedlicher Markt in beiden Crawls → Vergleichsspalten leer, Hinweis im Tab.
- **Testfixture:** zwei gespeicherte Crawls `tests/fixtures/dfs-crawls/compare-a.ospider` und `compare-b.ospider`.

### D4S-076 DataForSEO-Werte als Eingabe für KI-Fragen
- **Priorität:** P2
- **Beschreibung:** DFS-Werte stehen als Platzhalter in Prompts (INTEG-164) und in den Fragen der Decision-Engine (`os-ai`) zur Verfügung, z. B. um zu prüfen, ob eine Seite die Suchintention ihres Haupt-Keywords erfüllt. Platzhalter: `{{dfs.main_keyword}}`, `{{dfs.top_keyword}}`, `{{dfs.top_keywords_10}}` (die 10 Keywords mit dem höchsten Traffic, zeilenweise), `{{dfs.serp_features}}`, `{{dfs.ai_overview_cited}}`. Eine KI-Anfrage, die DFS-Platzhalter nutzt, wartet auf die DFS-Daten der URL; fehlen sie endgültig, wird sie mit `Skipped (empty input)` übersprungen (INTEG-173).
- **Eingaben/Konfiguration:** Platzhalter-Einfügen im Prompt- bzw. Fragen-Editor (nur angeboten, wenn das passende Modul aktiv ist).
- **Ausgabe:** keine eigene; Wirkung in den KI-Spalten.
- **Akzeptanzkriterien:**
  - [ ] Prompt „Intent von {{dfs.main_keyword}}?“ für `/a` → Mock-Echo empfängt „Intent von widget shop?“.
  - [ ] Für `/j` (kein Keyword) wird keine KI-Anfrage gesendet; Status `Skipped (empty input)`.
  - [ ] Die KI-Anfrage für eine URL startet erst nach Eintreffen ihrer SERP-Daten (Zeitstempel Mock-Log).
- **Testfixture:** Mocks DFS und KI-Echo-Endpunkt.

---

## Anhang A – Datenhaltung in der Crawl-Datei

| Tabelle | Inhalt | Schlüssel / Bemerkung |
|---|---|---|
| `dfs_cache` | komprimierte Rohantworten je Ziel | (Umgebung, Endpunkt, normalisiertes Ziel, Markt, Gerät, Parameter-Hash); Erhebungszeitpunkt, Kosten |
| `dfs_tasks` | gestellte Standard-Tasks | Task-ID, Endpunkt, Ziel, Parameter-Hash, gestellt um, Status |
| `dfs_ranked_keywords` | Keyword-Zeilen je URL (inkl. Domain-Abzug ohne Crawl-URL) | (URL bzw. API-URL, Keyword, Markt); Kennzahlen aus D4S-021 |
| `dfs_serp_results` | Ergebnislisten je Keyword | (Keyword, Markt, Gerät, `rank_absolute`); Typ, URL, Domain, Titel; AI-Overview-Referenzen |
| `dfs_backlinks` | Kennzahlen je Ziel und Ebene | (Ziel, Ebene, Rank-Skala) |
| `dfs_llm` | Domain-, Seiten-, Keyword- und Verlaufswerte; Erwähnungs-Details mit LLM-Fragen und -Antworten (Fremdinhalte, D4S-055) | (Abfragetyp, Ziel, Plattform, Markt); Fremdinhalte löschbar über „Erwähnungs-Details löschen“ |
| `dfs_cost_log` | Kostenprotokoll DataForSEO (D4S-014) | Zeit, Modul, Endpunkt, Modus, Zweck, Ziele, Zeilen, Reservierung und Kosten in Mikro-USD, Status-Code, Herkunft, Task-ID; KI-Kosten stehen getrennt in `ai_requests` (08, AI-014) |

Zugangsdaten, Basic-Auth-Header und vollständige Request-Bodies werden nie in der Crawl-Datei gespeichert. URLs mit Query-String oder Userinfo erscheinen nur dann in `dfs_*`-Tabellen, wenn sie nach D4S-010 gesendet werden durften.

---

## Anhang B – Quellen (geprüft am 13.10.2026)

- DataForSEO API v3 – Übersicht, Authentifizierung, Sandbox, Fehlercodes, User Data: `https://docs.dataforseo.com/v3/`, `…/v3/auth/`, `…/v3/appendix/sandbox/`, `…/v3/appendix/errors/`, `…/v3/appendix/user_data/`
- Labs: `…/v3/dataforseo_labs/google/ranked_keywords/live/`, `…/bulk_traffic_estimation/live/`, `…/relevant_pages/live/`, `…/serp_competitors/live/`, `…/v3/dataforseo_labs/locations_and_languages/`
- Backlinks: `…/v3/backlinks/summary/live/`, `…/bulk_pages_summary/live/`, `…/bulk_new_lost_backlinks/live/`
- SERP: `…/v3/serp/google/organic/live/advanced/`, `…/task_post/`, `…/tasks_ready/`, `…/v3/serp/google/ai_mode/live/advanced/`
- AI Optimization: `…/v3/ai_optimization/llm_mentions/overview/`, `…/locations_and_languages/`, `…/filters/`, `…/target_metrics/live/`, `…/search_mentions/live/`, `…/top_mentioned_pages/live/`, `…/multi_target_metrics/live/`, `…/historical/live/`
- Preise: `https://dataforseo.com/pricing`, `https://dataforseo.com/pricing/serp/google-organic-serp-api`, `https://dataforseo.com/pricing/serp/google-ai-mode-serp-api`, `https://dataforseo.com/pricing/backlinks/backlinks`, `https://dataforseo.com/pricing/ai-optimization/llm-mentions`
- Preisänderung vom 01.07.2026: `https://dataforseo.com/update/pricing-update-in-dataforseo-apis`

---

## Anhang C – Offene Fragen

Seit der Fable-Prüfung vom 13.10.2026 sind alle vorläufigen Festlegungen als **Entscheidung (Fable-Empfehlung)** bestätigt bzw. ergänzt; Q8 (Abrechnung) und der Kostenteil von Q5 sind geklärt. Fragen, die nur mit echten API-Aufrufen klärbar sind, sind zusätzlich am Dateiende unter „Vor Umsetzung zu verifizieren“ (V-D1 … V-D11) mit Prüfweg gesammelt.

| Nr. | Frage | Betroffen | Vorläufige Festlegung |
|---|---|---|---|
| Q1 | Gibt es für Backlinks oder LLM Mentions nach dem 01.07.2026 noch Konten, die 40204 („Abo erforderlich“) erhalten? Die Fehlercode-Doku nennt die Backlinks-Mindestabnahme noch, Ankündigung, Preisseiten und `user_data` nicht mehr. Klärung per Live-Test mit einem frischen Konto oder Support-Anfrage. | D4S-004, D4S-030, D4S-050 | **Entscheidung (Fable-Empfehlung)**: pay-as-you-go beibehalten; 40204 → `Subscription required` nur für das betroffene Modul; Prüfung beim ersten Live-Test mit frischem Konto, keine weitere Maßnahme |
| Q2 | Ab welchem Zeitpunkt (UTC-Tageswechsel?) wird ein im Dashboard gesetztes Tageskostenlimit (40203) zurückgesetzt? | D4S-004 | **Entscheidung (Fable-Empfehlung)**: UTC-Mitternacht beibehalten; zusätzlich vor jedem neuen Lauf und bei „DataForSEO-Daten anfordern“ `user_data` (kostenlos) abfragen und `money.limits.day` gegen `money.statistics` prüfen – ist wieder Luft, wird das Modul unabhängig von der Uhr freigegeben (D4S-004) |
| Q3 | Zuschläge `load_async_ai_overview` und `calculate_rectangles`: Endpunkt-Doku nennt pauschal +0,002 $, die Preisseiten +1 Basispreis (Standard 0,0006 $, Priority 0,0012 $, Live 0,002 $). Welcher Wert gilt im Standard-/Priority-Modus? Zusätzlich: Wird der Zuschlag je Task berechnet oder – wie der Basispreis – je 10 Ergebnisse von `depth` (bei depth 20 also doppelt)? Wird die Erstattung des AIO-Zuschlags ohne AI Overview in der gemeldeten `cost` der Antwort sichtbar oder erst später? | D4S-011, D4S-012, D4S-013, D4S-043 | Zuschlag je Task, unabhängig von `depth`; Untergrenze 1 Basispreis, Obergrenze max(1 Basispreis; 0,002 $); AIO-Zuschlag nicht in `Cost Min`; Reservierung mit Obergrenze; Abgleich über gemeldete `cost` (D4S-011). Falls der Zuschlag doch mit `depth` skaliert, würde die Reservierung zu niedrig – ein Golden-Test mit depth 20 gegen die Sandbox klärt das vor Freigabe. **Entscheidung (Fable-Empfehlung)**: Festlegung beibehalten; der Golden-Test depth 10/20 gegen die Sandbox ist Pflicht-Gate im Release-Check vor Freigabe von M5 (Abschnitt „Release-Gate M5“) |
| Q4 | Wie behandeln die Labs-Endpunkte Seiten-URLs, die nicht der dokumentierten Form entsprechen? `ranked_keywords` erkennt Seiten nur mit `https://` oder `www.`, `bulk_traffic_estimation` verlangt `https://` **und** `www.`. Werten die Endpunkte andere Formen als ganze Domain aus? | D4S-020, D4S-024 | `ranked_keywords`: `http://`-URLs ohne `www.` nicht senden (`Error: URL-Form von Labs nicht unterstützt`); `bulk_traffic_estimation`: Vorfilter nur für `https://www.`-URLs, alle anderen direkt einzeln abfragen; Bulk-Items mit abweichendem `target` verwerfen (**Entscheidung (Fable-Empfehlung)**: beibehalten) |
| Q5 | `top_mentioned_pages`: teilweise geklärt – verifiziert sind `include_pages`/`exclude_pages` (exakte URL-Listen), `filters` (≤ 8, u. a. `like`/`ilike`), `limit` Standard 100 / max. 1.000, `offset` ≤ 1.000.000. Geklärt: `available_filters` ist laut Doku kostenlos (verifiziert). Offen: Ist `page` als Filterfeld zulässig (die allgemeine Filterliste der Doku nennt es nicht; maßgeblich ist `available_filters`)? Welcher Feldname gilt für die Sortierung nach Erwähnungen in `order_by`? | D4S-051, D4S-052 | **Entscheidung (Fable-Empfehlung)**: serverseitiger `page`-Filter, wenn `available_filters` ihn anbietet, sonst clientseitig; `order_by: ["mentions,desc"]`; antwortet ein Endpunkt darauf mit 40501, einmal ohne `order_by` erneut senden und clientseitig sortieren, damit `Rank` deterministisch bleibt (D4S-052); Abgleich beim ersten Gebrauch |
| Q6 | Gilt für `bulk_traffic_estimation` dieselbe Preisstruktur (je Task + je Item) wie für die übrigen Labs-Endpunkte? | D4S-012, D4S-024 | ja (**Entscheidung (Fable-Empfehlung)**: beibehalten; Abgleich über die gemeldete `cost` beim ersten Live-Lauf) |
| Q7 | **Geklärt (13.10.2026):** LLM Mentions hat einen eigenen, kostenlosen Endpunkt `GET /v3/ai_optimization/llm_mentions/locations_and_languages` mit `available_platforms` je Land/Sprache. | D4S-007, D4S-050 | umgesetzt in D4S-007: eigene LLM-Marktliste, `Skipped (market unsupported)` vor dem Senden |
| Q8 | AI Mode: Pfad (`/v3/serp/google/ai_mode/live/advanced`), Struktur und Preise je Task sind geklärt (D4S-048). **Abrechnung geklärt (13.10.2026, Preisseite AI Mode, FAQ):** je Request = eine AI-Mode-Seite, nicht je 20/10 Ergebnisse. Offen nur noch: Felder und Parameter von `timeseries_new_lost`. | D4S-048, D4S-054 | je Task (geklärt); `timeseries_new_lost` vor Umsetzung per Probeaufruf prüfen |
| Q9 | **Entschieden (Architekt, 13.10.2026):** gemeinsames Budget über `BudgetLedger` in `os-model` (Grenzwerte persistiert über `os-store`, Tabelle `api_budget`); eine Kostenschätzung und eine Obergrenze je Crawl für KI und DataForSEO; Protokolle bleiben getrennt (`dfs_cost_log`, `ai_requests`). Umgesetzt in AI-012/AI-013 (führend), D4S-012/D4S-013. | D4S-012, D4S-013, D4S-014 | entschieden |
| Q10 | **Entschieden (Architekt, 13.10.2026):** Das Backlinks-Modul fragt standardmäßig nur indexierbare 200er ab; nicht indexierbare und Nicht-200-URLs sind je Modul zuschaltbar (D4S-017); der Kostenvoranschlag schlägt die Optionen vor. | D4S-017, D4S-061 | entschieden |
| Q11 | Erfordert die Sandbox dieselben API-Zugangsdaten wie die Produktiv-API, und liefert sie auch für AI Optimization Dummy-Daten? | D4S-003 | **Entscheidung (Fable-Empfehlung)**: gleiche Zugangsdaten beibehalten; liefert die Sandbox für LLM-Mentions-Endpunkte 40400/40501, zeigt das Modul im Sandbox-Modus den Banner „Sandbox liefert keine LLM-Mentions-Daten“, URLs erhalten `Not requested`, kein Fehler (D4S-003) |
| Q12 | Gilt `task_get` für SERP-Ergebnisse als kostenlos (Kosten werden beim `task_post` berechnet)? `tasks_ready` ist laut Doku kostenlos. | D4S-008, D4S-011 | **Entscheidung (Fable-Empfehlung)**: kostenlos beibehalten; Prüfung über die gemeldete `cost` je Schritt |

---

## Vor Umsetzung zu verifizieren

Gesammelt am 13.10.2026: API-Details, die sich aus Doku und Preisseiten nicht eindeutig klären lassen und erst mit echten Aufrufen (Sandbox oder Produktivkonto, optionale Live-Tests mit `OPENSPIDER_LIVE_API_TESTS=1`, nie Teil der Akzeptanz) bzw. per Support-Anfrage geklärt werden. Bis zur Klärung gilt die in der Tabelle genannte Festlegung; seit der Fable-Prüfung (13.10.2026) sind alle Festlegungen als **Entscheidung (Fable-Empfehlung)** bestätigt bzw. ergänzt, der Abrechnungsteil von V-D7 und der Kostenteil von V-D5 sind geklärt. Ergebnis je Punkt mit Datum hier nachtragen und Preiskatalog (D4S-011) bzw. betroffene Einträge anpassen. Entschiedene Fragen ohne API-Bezug (Q9, Q10) und die bereits geklärte Q7 sind nicht aufgeführt.

| Nr. | Frage | Fundstelle | Vorläufige Festlegung | Klärung durch |
|---|---|---|---|---|
| V-D1 | Kommt 40204 („Abo erforderlich“) für Backlinks oder LLM Mentions nach dem 01.07.2026 noch vor? | Q1; „Klärung: 100-$-Mindestabnahme“; D4S-004, D4S-030, D4S-050 | **Entscheidung (Fable-Empfehlung)**: pay-as-you-go; 40204 → `Subscription required` nur für das betroffene Modul; keine weitere Maßnahme | beim ersten Live-Test je ein Aufruf `bulk_pages_summary` und `target_metrics` mit einem frisch angelegten Konto ohne Vorgeschichte; ggf. Support-Anfrage |
| V-D2 | Rücksetzzeitpunkt eines im Dashboard gesetzten Tageskostenlimits (40203) | Q2; D4S-004 (Tabelle der Fehlercodes, „Freigabe nach 40203“) | **Entscheidung (Fable-Empfehlung)**: UTC-Mitternacht; zusätzlich Freigabe unabhängig von der Uhr, sobald `user_data` vor einem Lauf wieder Luft meldet (`money.limits.day` gegen `money.statistics`) | Live-Test mit niedrigem Tageslimit über einen Tageswechsel, Abfrage `user_data` vor und nach 00:00 UTC |
| V-D3 | SERP-Zuschläge `load_async_ai_overview` und `calculate_rectangles`: pauschal 0,002 $ oder +1 Basispreis je Modus; je Task oder je 10 Ergebnisse von `depth`; wird die AIO-Erstattung ohne AI Overview in der gemeldeten `cost` sofort sichtbar? | Q3; D4S-011, D4S-012, D4S-013, D4S-043 | **Entscheidung (Fable-Empfehlung)**: Zuschlag je Task, unabhängig von `depth`; Untergrenze 1 Basispreis, Obergrenze max(1 Basispreis; 0,002 $); AIO-Zuschlag nicht in `Cost Min`; Reservierung mit Obergrenze | **Pflicht-Gate vor Freigabe von M5** (Release-Gate M5): Golden-Test depth 10 und 20 gegen die Sandbox, Keyword mit und ohne AI Overview, Modi Standard/Priority/Live; meldet die Sandbox `cost = 0`, derselbe Test mit dem Produktivkonto; Vergleich der gemeldeten `cost` |
| V-D4 | Labs: Auswertung von Seiten-URLs, die nicht der dokumentierten Form entsprechen (`http://` ohne `www.` bei `ranked_keywords`; alles außer `https://www.` bei `bulk_traffic_estimation`) | Q4; D4S-020, D4S-024 | **Entscheidung (Fable-Empfehlung)**: beibehalten – `http://` ohne `www.` nicht an `ranked_keywords`; Vorfilter nur für `https://www.`; abweichendes `target` im Bulk verwerfen | Live-Aufrufe mit je einer URL pro Form und Vergleich mit der Domain-Antwort |
| V-D5 | `top_mentioned_pages`: ist `page` als Filterfeld zulässig, welcher Feldname gilt in `order_by` für die Sortierung nach Erwähnungen? (`available_filters` ist laut Doku kostenlos – verifiziert) | Q5; D4S-051, D4S-052 (Abschnitt „Zeilenkosten sparen“) | **Entscheidung (Fable-Empfehlung)**: serverseitiger `page`-Filter nur, wenn `available_filters` ihn anbietet; `order_by: ["mentions,desc"]`; bei 40501 einmal ohne `order_by` erneut senden und clientseitig sortieren (D4S-052) | Live-Aufruf `available_filters` und `top_mentioned_pages` mit `page`-Filter und `order_by` |
| V-D6 | Preisstruktur von `bulk_traffic_estimation` (je Task + je Item wie die übrigen Labs-Endpunkte?) | Q6; D4S-012, D4S-024 | **Entscheidung (Fable-Empfehlung)**: ja | Abgleich über die gemeldete `cost` beim ersten Live-Lauf (1, 10 und 100 Ziele) |
| V-D7 | AI Mode: **Teil „Abrechnung“ geklärt (13.10.2026, Preisseite AI Mode, FAQ): je Request = eine AI-Mode-Seite.** Offen nur: Parameter und Felder von `timeseries_new_lost` | Q8; D4S-048, D4S-054 | je Task (geklärt); `timeseries_new_lost` erst nach Klärung nutzen | Probeaufruf `timeseries_new_lost` mit eigener Domain vor Umsetzung von D4S-054 |
| V-D8 | Sandbox: dieselben Zugangsdaten wie die Produktiv-API? Liefert sie Dummy-Daten auch für AI Optimization / LLM Mentions? | Q11; D4S-003 | **Entscheidung (Fable-Empfehlung)**: gleiche Zugangsdaten; bei 40400/40501 für LLM-Mentions-Endpunkte Banner „Sandbox liefert keine LLM-Mentions-Daten“, URLs `Not requested`, kein Fehler (Mock-Szenario `sandbox-no-llm`) | Aufrufe aller genutzten Endpunkte gegen `sandbox.dataforseo.com` mit den Produktiv-Zugangsdaten |
| V-D9 | Ist `task_get` für SERP-Ergebnisse kostenlos (Abrechnung beim `task_post`)? | Q12; D4S-008, D4S-011 | **Entscheidung (Fable-Empfehlung)**: kostenlos; Prüfung über die gemeldete `cost` je Schritt | Live-Lauf `task_post` → `tasks_ready` → `task_get/advanced` mit Auswertung von `cost` je Schritt |
| V-D10 | Belegung von `func_type`/`func_name` je genutztem Endpunkt in `user_data.price` (Zuordnung im Preiskatalog, Feld `user_data_key`) | D4S-011 | **Entscheidung (Fable-Empfehlung)**: Zuordnung aus der Doku mit Herkunft `catalog` beibehalten | nach der ersten echten `user_data`-Antwort des Produktivkontos `user_data_key` je Endpunkt eintragen |
| V-D11 | `bulk_pages_summary`: Fehlt `referring_links_attributes.nofollow` bei Seiten ohne Nofollow-Links, oder wird 0 geliefert? | D4S-030 (Ableitung `DFS Dofollow Backlinks %`) | **Entscheidung (Fable-Empfehlung)**: fehlender Schlüssel gilt als 0 (beibehalten) | Live-Aufruf für eine Seite ohne Nofollow-Backlinks |

## Release-Gate M5

**Entscheidung (Fable-Empfehlung)** vom 13.10.2026: Vor Freigabe von M5 ist im Release-Check folgender Punkt abzuhaken. Er läuft manuell bzw. mit `OPENSPIDER_LIVE_API_TESTS=1` und ist kein Teil der CI-Akzeptanz.

1. **Golden-Test SERP-Zuschläge (V-D3, Q3):** SERP-Requests mit `depth` 10 und 20, je ein Keyword mit und ohne AI Overview, Modi Standard, Priority und Live, gegen die Sandbox; meldet die Sandbox `cost = 0`, wird derselbe Test mit dem Produktivkonto wiederholt. Bestanden, wenn jede gemeldete `cost` zwischen `Cost Min` und `Cost Max` aus D4S-012 liegt. Fällt der Test durch (z. B. weil der Zuschlag mit `depth` skaliert), werden Preiskatalog (D4S-011), Formeln (D4S-012) und Reservierung (D4S-013) angepasst und der Test wiederholt, bevor M5 freigegeben wird. Ergebnis mit Datum in V-D3 eintragen.

### D4S-200 DataForSEO über evnxt-Credits
- **Priorität:** P1
- **Beschreibung:** Alternativ zum eigenen DataForSEO-Konto bezieht OpenSpider Labs-, Backlink-, SERP- und LLM-Mentions-Daten über den evnxt-Account (gleiche Anmeldung wie AI-200); evnxt rechnet über Credits ab und nutzt seinen bestehenden DataForSEO-Adapter mit Cache. Spaltenschema und Ergebnisse identisch zum direkten Weg.
- **Akzeptanzkriterien:**
  - [ ] Gleiche Abfrage über eigenes Konto und über evnxt liefert identische Spalten und (bei gleicher Datenlage) identische Werte.
  - [ ] Kostenschätzung in Credits vor dem Lauf, gemeinsames Budget mit KI (BudgetLedger).
  - [ ] Ohne evnxt-Anmeldung bleibt der direkte Weg mit eigenem Konto vollständig nutzbar.


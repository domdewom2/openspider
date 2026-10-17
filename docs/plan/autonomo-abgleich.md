# Abgleich Autonomo SEO ↔ OpenSpider (Cloud- und Desktop-Variante)

Stand: 16.10.2026 · Planer: Opus · Grundlage: Lesezugriff auf den VPS (`/opt/autonomo-seo` Code-Stand `8cbef76`, `/etc/autonomo-seo` nur Struktur, systemd-Units `autonomo-*`, Caddy-Site `autonomo-extern.caddy`, Autonomo-Schnittstelle in `/opt/outreach`) sowie `docs/ARCHITECTURE.md`, `docs/spec/*`, `docs/plan/M2.md` und `docs/plan/ROADMAP-nach-1.0.md` dieses Repos. Es wurden keine Dateien, Dienste oder Datenbanken verändert. Geheimnisse sind nicht übernommen, Variablennamen nur, wo sie für das Verständnis nötig sind.

Vorgabe des Nutzers: Autonomo SEO (ausgeliefert über evnxt.de) ist die **Cloud-Variante**, OpenSpider die **Desktop-/Agent-Variante**. Beide sollen funktional zusammenpassen.

---

## 1 Kurzbeschreibung Autonomo SEO

### 1.1 Was es ist

Autonomo SEO ist eine Mehragenten-Plattform für SEO-Aufgaben (Brain-Orchestrator, Fachagenten, Aufgaben mit Freigabe, Workflows, Gedächtnis). Seit September 2026 ist ihr größter Teil ein eigener **Crawler mit Audit-Engine**. Seit dem Onpage-MVP vom 28.09.2026 dient sie außerdem als **Rechen-Backend für evnxt.de**: evnxt (Laravel) ist die Kundenoberfläche mit Mandanten, Konten, Paketen, GSC/GA4-Anbindung, Monitoring und einem eigenen MCP-Server. Autonomo crawlt, prüft, deutet und liefert die Ergebnisse über `/api/extern/*` zurück. „Cloud-Variante“ heißt deshalb praktisch **evnxt-Oberfläche + Autonomo-Engine**.

### 1.2 Stack und Betrieb

| Schicht | Technik |
|---|---|
| Backend | Python 3.12, FastAPI, SQLAlchemy 2 (async), Alembic (47 Migrationen), Pydantic v2, APScheduler, litellm, httpx; rund 146 000 Zeilen Python, Testsuite rund 6 000 Tests (mit E2E gegen ein lokales Testgelände) |
| Datenbank | PostgreSQL 16 mit pgvector, Redis 7 (Compose-Stack, nur Loopback) |
| Frontend | React 19, TypeScript, Vite, Tailwind, React Flow; ein zweites Bundle `dist-extern` wird 1:1 in evnxt eingeblendet (`/api/extern/crawler-ui`) |
| Prozess | **ein** uvicorn-Worker auf Unix-Socket (`autonomo-seo-backend.service`, gehärtet, `MemoryMax=8G`); Scheduler, Crawls, Nachläufe und KI-Aufrufe laufen in diesem einen Prozess |
| Netz | Kein eigenes Login. Von außen nur `/api/extern/*` über einen Host-Caddy auf Loopback-Port 7781, den evnxt-Container über eine Socket-Brücke (`autonomo-extern-bridge.socket`) auf der Docker-Bridge erreichen. Jede Anfrage braucht einen Bearer-Token (`AUTONOMO_EXTERN_TOKEN`); fehlt er in der Konfiguration, antwortet der Router mit 503. Betreiber-UI nur über Tailnet |
| KI | Gateway `sub2api` (Claude-Modelle, Rückfall `gpt-5.6-terra` mit strukturierter Ausgabe), Entscheidungsdienst JEV, Embeddings über pgvector |
| Ablage | Zusatzvolume: `pgdata` 8,6 GB, `data` 0,9 GB (HTML-Stichproben, Seitenarchiv, Exportverzeichnisse), Playwright-Browser 0,7 GB (installiert, im Crawler nicht verdrahtet) |
| Zeitgeber (systemd) | `autonomo-server-export@<domain>.timer` alle 5 min (root-seitiger Export-Helfer: Bot-Zeilen, KI-Referrals, Dateiliste, Soll-URLs aus der Site-DB) für caloone.de, rotweine.org, matchdaylegends.com, openspider.de; `autonomo-seo-sicherung-frist.timer` täglich (Sicherungen höchstens 14 Tage) |
| Zeitgeber (APScheduler, im Prozess) | Log-Import alle 5 min, Log-Deutung montags 06:00, Audit-Bericht wöchentlich, Keyword-Quellen wöchentlich, GEO-Wochenlauf, Seitenarchiv 03:40, Crawl-Aufbewahrung 03:50, dazu frei konfigurierbare Cron-Jobs und Workflows |

Datenbestand am 16.10.2026: 10 Projekte (8 davon evnxt-Portale), 60 Crawl-Sessions, rund 97 000 Seitenzeilen, **20,6 Mio. Linkzeilen**, 1,03 Mio. Befunde, 472 000 Bot-Treffer; größter Crawl 14 855 Seiten.

### 1.3 Datenfluss

```
evnxt (Laravel, Container)                                   Autonomo (FastAPI, ein Prozess)
  onpage:monatlich (täglich, Plan-Tag je Portal) ──POST /api/extern/onpage/crawls──▶ Auftrag → Projekt (extern_id = portal_id)
  GSC-Zahlen aus evnxt-Sync ───────────────────────(im Auftrag / nachgereicht)───▶      │
                                                                                   Orchestrator (httpx, Profil „kunde“)
                                                                                         │ crawl_pages / crawl_links / crawl_resources …
                                                                                   Nachlauf: engine → nachpruefung → analysen → probe → log →
                                                                                   klassifikation → politik → vergleich → keywords →
                                                                                   kannibalisierung → triage → anreicherung → fertig
                                                                                         │ crawl_issues (219 Regeln), seo_analyses, Audit (Laienfassung)
  Livewire-Seiten, MCP (120 Werkzeuge) ◀── GET status / befunde / seiten / vergleich / export.csv / crawler-ui / agentic / serverlog
  Log-Upload (.log/.gz) ───────────────POST /api/extern/serverlog/upload──▶ Einleser → bot-hits.jsonl → Import → Aggregate
  VPS-Domains: Export-Helfer (root, 5 min) ─────────────────────────────▶ Exportverzeichnis → Import
```

Mandanten trennt allein evnxt: Autonomo kennt nur die `portal_id`. Der Extern-Router liest nur Sessions, die er selbst gestartet hat. Die eingebettete Crawl-Oberfläche läuft über eine **Positivliste** von Handlern, und jede Id wird gegen den Bereich des Portals geprüft.

### 1.4 Crawler-Fähigkeiten (Engine-Version 8)

- **Abruf:** httpx, eigener Fetcher mit Fehlerklassen, Retry/Backoff (`Retry-After` bis 60 s), Größendeckel 5 MB, Meta-Refresh, SSRF-Sperre (`netpolicy`), Loopback-Crawl eigener Domains über eine Positivliste.
- **robots.txt:** RFC 9309 (in Runde 7 gegen Testfälle abgenommen), eigene Zeile für gesperrte URLs, Verhalten bei nicht erreichbarer robots.txt festgelegt. Bot-Token `AutonomoSEOBot` bzw. `evnxtAuditBot` für Kundenaufträge.
- **Sitemaps:** Index bis 3 Ebenen, höchstens 50 Dateien, `lastmod` wird genutzt (Aktualitätsregeln).
- **Tempo, fest je Lastprofil:** `eigen` 3 parallel/0,2 s, `fremd` (Wettbewerber) 2/1,0 s und höchstens 800 Seiten, `kunde` (evnxt) 2/0,5 s und höchstens 100 000 Seiten, Tiefe 50, Bilder nur als Stichprobe von 1 000. Ein Kundencrawl schafft damit höchstens etwa 4 URL/s.
- **Grenzen:** **kein JS-Rendering**: `js_rendering` erzeugt nur eine Warnung, Runde „B7“ ist ausgesetzt. Keine Custom Search/Extraction (B13 ausgesetzt). Keine Accessibility-, PSI- oder Lighthouse-Prüfung. Roh-HTML nur als Stichprobe (500 Seiten je Lauf). Kein Formular-Login, kein Proxy, kein Listen- oder SERP-Modus.
- **Stärken über das Übliche hinaus:** dynamisches Seitenlimit mit Hochrechnung je Ebene und ehrlicher Vollständigkeitsmeldung, Fallenwächter für Crawler-Fallen, Soft-404-Probe (zwei Abrufe sicher nicht existierender Pfade), Nachprüfung einzelner Befunde, Befund-Status „erledigt“, Indexierungspolitik je Projekt mit Projekttyp, Schwellen und „gewollt“-Unterdrückung, Soll-URL-Menge aus der Site-Datenbank (Waisen ohne Raten), Dateiliste des Docroots, Seitenarchiv (HTML-Fassungen 36 Monate) und Crawl-Vergleich in 15 Kategorien. Grundsatz: Regeln finden Fakten, das Modell deutet sie. Kein Modellbeitrag ändert eine Befundzahl, und ein Lauf ist beim Nachrechnen bitgleich.

### 1.5 Auswertungen, Berichte, KI

- **Befunde:** 219 Regeln in 25 Kategorien. Darunter sind die bekannten technischen und Onpage-Kategorien sowie die Autonomo-eigenen Kategorien Politik, Server-Protokoll, GEO, Keywords und Aktualität. Je Regel gibt es höchstens eine Zeile pro Seite, mit Beleg in `context`.
- **Audit (Laienfassung) für evnxt:** Lage, Gesamtwert, Maßnahmen je Themenbereich (wer, Aufwand, Beleg, Beispiele), Summary und „nicht geprüft“. Dazu kommt wöchentlich ein Audit-Bericht in zwölf Abschnitten (Markdown, HTML, CSV).
- **KI-Stufen** (Gateway, Belegprüfung): Seitentyp-Klassifikation über Vorlagen-Cluster, Befund-Triage, Deutung des Crawl-Vergleichs, Log-Deutung, Kannibalisierung, Zusammenfassung des Audit-Berichts.
- **Agentic-Module für evnxt:** SEO-Chat je Publisher (Streaming), Dashboard, Keyword-Belegung und Suchabsicht, interne Linkvorschläge (JEV, auf Klick), Wettbewerb und Sichtbarkeit (SISTRIX), Ranked Keywords und Backlinks (DataForSEO) mit Spam-Domain-Liste, GEO-Check (15 Faktoren) und GEO-Tracking (Prompts je Plattform messen, Wochenlauf, Abrechnung), Website-Wissen (Fakten, „gewollt“, Rückfragen), Gedächtnis.
- **Integrationen:** GSC (eigene Anbindung plus Zahlen, die evnxt mitliefert), SISTRIX, DataForSEO, Google Keyword Planner, Gemini Image, JEV. GA4, PSI und Uptime liegen in **evnxt**, nicht in Autonomo. Ein Wrapper für die CLI eines Desktop-Referenzcrawlers ist vorhanden, die Binärdatei aber nicht installiert.

### 1.6 Logfile-Analyse (L1 live seit 15.10.2026, Ausbau am 16.10.)

- **Quellen:** VPS-Domains automatisch über den Export-Helfer (Caddy-JSON), andere Kunden per Upload (`.log`/`.gz` bis 210 MB über Caddy, 512 MB entpackt). Formate Caddy-JSON, Apache/Nginx combined und vhost_combined werden je Zeile erkannt.
- **Datenschutz:** nur Bot-Zeilen und Besuche mit KI-Referrer (7 Hosts). Besucher-IPs werden beim Einlesen verworfen. Bot-Treffer bleiben 180 Tage, geprüfte IPs 90 Tage.
- **Bots:** 56 Einträge in 8 Klassen (Suchmaschine, KI-Training, KI-Suche, KI-Live-Abruf, SEO-Tool, Social, eigen, sonstige). Verifikation über veröffentlichte IP-Listen (höchstens alle 48 h geholt) und FCrDNS mit den Zuständen verifiziert, gefälscht, unverifizierbar und offen.
- **Auswertungen:** Übersicht, Achsen, URLs, Crawl-Budget, Dateien, nicht besuchte Seiten je Bot, „nicht im Crawl“, Statuswechsel, langsame URLs, URL-Detail, Verzeichnisse, Googlebot-Gerät, 10 Log-Regeln in `crawl_issues` und eine wöchentliche KI-Deutung. Alle Zahlen kommen aus genau einem Modul (`serverlog.py`). Es gibt keine zweite Abfrage über die Log-Tabellen.

### 1.7 Schnittstellen

- **`/api/extern/*`** (Bearer-Token, Fehlerform `{"fehler": …}`):
  - `onpage/crawls` (Auftrag, Status, Audit neu, GSC nachreichen, Befunde, Recheck, Erledigt, Nachprüfen, Seiten, Vergleich, `export.csv`) und `onpage/seitentypen`
  - `onpage/seitenarchiv/*`
  - `onpage/portal(e)` (Löschen mit der Website, Abgleich)
  - `crawler-ui/{portal}/…` (eingebettete Oberfläche über die Positivliste)
  - `agentic/*` (rund 45 Endpunkte)
  - `serverlog/*` (16 Endpunkte)
- **Intern:** `/api/crawler`, `/api/keywords` und weitere ohne Mandantenprüfung, nur über das Tailnet erreichbar.
- **MCP:** In Autonomo gibt es nur einen veralteten Datenbank-MCP (`app/mcp/server.py`, SQL-Lesezugriff, für den Betreiber). Der **produktive MCP-Server ist der von evnxt** (`laravel/mcp`, Streamable HTTP unter `evnxt.de/mcp`, API-Schlüssel mit Scopes, 120 Werkzeugdateien, darunter `ListCrawls`, `GetCrawlSummary`, `GetCrawlAnalysis`, `GetCrawlComparison`, `GetOnpageFindings`, `ListCrawlPages` und `GetAuditOverview`). Er liest Autonomo über `AutonomoLeser` (GET mit 10 min Zwischenspeicher) und hält einen Seitenspiegel in evnxt (`mcp_crawl_spiegel`).
- **Aufbewahrung:** Crawl-Daten der evnxt-Websites 13 Monate, Seitenarchiv 36 Monate, Löschen mit der Website über `DELETE /onpage/portal`.

---

## 2 Funktionsmatrix

Legende Autonomo: **ja** · **teilw.** · **nein** · **evnxt** (in der Cloud vorhanden, aber in evnxt statt in Autonomo).
Legende OpenSpider (Stand Spec/Plan): **M1** (umgesetzt) · **M2** (geplant für 1.0) · **M3** · **M4/M5** · **R3/R6/R7** (Roadmap nach 1.0) · **fehlt** (weder Spec noch Roadmap).

### 2.1 Crawl-Kern

| Funktion | Autonomo | OpenSpider |
|---|---|---|
| HTTP-Crawl, Weiterleitungen, Fehlerklassen, Retry | ja | M1 |
| robots.txt nach RFC 9309 | ja | M1 (eigener Parser) |
| XML-Sitemaps (Index, lastmod) | ja (3 Ebenen, 50 Dateien) | M1/M2 |
| JS-Rendering (Chromium), Roh- gegen Render-Vergleich, Screenshots | nein (ausgesetzt) | M2 |
| Listen-, SERP- und Vergleichsmodus | nein | M1/M2 |
| Formular-Login, Cookies, Proxy | nein | M2 |
| Lastprofile (eigen/fremd/kunde) | ja, fest im Code | teilw.: Rate-Limit M1, Crawl-delay/adaptiv/Auto-Tempo M2 (CRAWL-145–147); benannte Profile fehlen |
| Pause/Fortsetzen, Neu-Crawl einzelner URLs | ja | M1/M2 |
| Skalierung | 100 000 Seiten Deckel, Links als Zeilen in Postgres | 1 Mio.+ URLs mit konstantem Durchsatz (PERF-002), eine SQLite-Datei je Crawl |
| HTML speichern | Stichprobe 500 Seiten, dazu Seitenarchiv | M2 (CRAWL-063, alles oder nichts) |
| Soft-404 | ja (aktive Probe) | M2 (ONPAGE-147, Phrasen) |
| Fallenwächter, dynamisches Limit mit Hochrechnung, Vollständigkeitsmeldung | ja | fehlt |
| Loopback-Crawl eigener Domains, Soll-URL-Liste aus der Site-DB, Docroot-Dateiliste | ja (Same-Server) | fehlt (Listenmodus kommt dem nahe) |
| Fokussierter Crawl nach Seitentyp | teilw. (Seitentyp nur nachträglich per KI) | R3 (AGENT-060–062) |

### 2.2 Auswertung

| Funktion | Autonomo | OpenSpider |
|---|---|---|
| Onpage- und technische Tabs/Befunde (Titel, Description, Headings, Canonicals, Directives, hreflang, Pagination, Bilder, Security) | ja (219 Regeln) | M1 (P0) / M2 (P1) |
| Strukturierte Daten mit Validierung | ja | M2 |
| Accessibility (axe), Validation-Tab, Mobile | nein / teilw. / teilw. | M2 |
| Lesbarkeit, Rechtschreibung/Grammatik | nein | M2 |
| Duplikate exakt / near (SimHash/MinHash) | exakt; near nur geplant | M1/M2 |
| Link Score, Waisen, Klicktiefe, Visualisierungen | teilw. (keine Graphen gewollt) | M1/M2 |
| Custom Search / Extraction / JavaScript | nein | M2 |
| Segmente | teilw. (Seitentypen, Gruppen) | M2 |
| Crawl-Vergleich, Change Detection | ja (15 Kategorien, KI-Deutung) | M2 (OUT-140–149) |
| Indexierungspolitik je Projekt, „gewollt“-Unterdrückung | ja | fehlt |
| Aktualität (lastmod gegen Inhaltsänderung, „veraltet mit Sichtbarkeit“) | ja | R7 (nur lastmod-Plausibilität) |
| Befund-Status (erledigt), Nachprüfung einzelner Befunde | ja | fehlt (Neu-Crawl einzelner URLs: M2) |
| Laien-Audit mit Maßnahmenplan | ja (KI mit Belegprüfung) | M4 (AI-106/107) |

### 2.3 Datenablage, Export, Berichte

| Funktion | Autonomo | OpenSpider |
|---|---|---|
| Speicher | Postgres, gemeinsam für alle Projekte | `*.ospider` (SQLite) je Crawl, portabel |
| Export CSV | ja (Seiten, Befunde) | M1 |
| XLSX, Bulk-Exporte, Sitemap-Generator, Looker Studio | nein | M1/M2 |
| Berichte | wöchentlicher Audit-Bericht (MD/HTML/CSV) | M2 Reports, M4 Maßnahmenplan (HTML/PDF/DOCX/MD) |
| Seitenarchiv (Fassungen über Monate) | ja (36 Monate) | fehlt (R6 erwähnt ein Archiv je Crawl) |
| Aufbewahrungsfrist, Löschen je Mandant | ja (13 Monate) | M2 (Crawl Retention, OUT-229) |

### 2.4 Mandanten, Projekte, Zugriff

| Funktion | Autonomo | OpenSpider |
|---|---|---|
| Mandanten, Konten, Pakete, Rechte | evnxt (Autonomo kennt nur `portal_id`) | fehlt (Einzelplatz) |
| Projekte | ja (Projekt je Portal, Politik, Wettbewerber) | M2 (Crawl-Verwaltung/Registry, OUT-227) |
| Eigene Anmeldung | nein (Pfadfilter + Token) | nicht nötig (lokal), MCP-HTTP mit Token (M2) |

### 2.5 Zeitplanung und Monitoring

| Funktion | Autonomo | OpenSpider |
|---|---|---|
| Geplante Crawls | evnxt (monatlich je Portal) + APScheduler | M2 (OS-Planer, OUT-160 ff.) |
| E-Mail/Benachrichtigung bei Crawl-Ende | evnxt | M2 (OUT-280–284) |
| Uptime, SSL, URL-Änderungsmonitoring mit Bewertung, Alarme | evnxt (Überwachung, JEV) | R6 |
| KPI-Verlauf über viele Crawls | teilw. (Vergleich je Paar, Wochenberichte) | R6 |

### 2.6 Integrationen

| Funktion | Autonomo | OpenSpider |
|---|---|---|
| Google Search Console (Search Analytics) | ja / evnxt | M2 |
| GSC URL Inspection | nein | M2 |
| GA4 | evnxt | M2 |
| PageSpeed Insights / Lighthouse | evnxt (MCP) | M2 |
| Majestic, Ahrefs, Moz | nein | M2 |
| DataForSEO (Ranked Keywords, Backlinks, SERP, LLM Mentions) | ja (teilweise) | M5 |
| SISTRIX, Keyword Planner | ja | fehlt |

### 2.7 KI

| Funktion | Autonomo | OpenSpider |
|---|---|---|
| Anbieter-Abstraktion, Kosten-Tracking | ja (litellm, Gateway) | M4 (`os-ai`, Budget je Crawl) |
| Seitentyp-Klassifikation | ja | M4 (AI-050), R3 |
| Triage, Maßnahmenplan | ja | M4 |
| Frag den Crawl (NL → SQL) | nein (Betreiber-MCP mit SQL) | M4 (AI-100 ff.) |
| Decision-Engine, Embeddings, semantische Analysen | teilw. (JEV, pgvector) | M4 |
| Interne Linkvorschläge | ja (JEV) | M4 (AGENT-030), R7 (Link-Potenziale) |
| AI-Search-Sichtbarkeit, GEO-Tracking | ja (GEO-Check, Prompts messen) | M4 (AI-Search), M5 (LLM Mentions) |
| AI-Crawler-Katalog, robots je KI-Crawler | ja (Bot-Katalog, `llms.txt`) | M4 (AI-120/121) |
| Chat-Agent je Website, Wissen, Gedächtnis | ja | fehlt (gewollt: OpenSpider ist Werkzeug für fremde Agenten) |

### 2.8 Logfile-Analyse

| Funktion | Autonomo | OpenSpider |
|---|---|---|
| Import Caddy/Apache/Nginx, Upload | ja | fehlt |
| Bot-Klassen, Verifikation (IP-Listen, FCrDNS) | ja | fehlt |
| Abgleich Log ↔ Crawl (nicht besucht, nicht im Crawl, Budget) | ja | fehlt |
| KI-Referrals | ja | fehlt |
| Automatischer Log-Zulauf vom Server | ja (VPS) | nicht sinnvoll lokal |

### 2.9 Schnittstellen und Betrieb

| Funktion | Autonomo | OpenSpider |
|---|---|---|
| REST-API | ja (`/api/extern`, proprietär) | fehlt (MCP + CLI statt REST) |
| MCP | evnxt-MCP (HTTP, Scopes, mandantenfähig) | M1 STDIO lesend, M2 HTTP (Loopback + Token), Werkzeugklassen read/crawl/write/cost |
| Read-only SQL für Agenten | Betreiber-MCP | M2 (AGENT-014, gehärteter Sandkasten) |
| CLI / headless | nein | M1 |
| Plattform | Linux-Server | macOS/Windows; CLI soll unter Linux bauen (OUT-264, P2) |
| Paritätstests gegen Referenzexporte | nein (eigene Abnahme-Scorecards) | ja (`os-parity`) |

---

## 3 Was Autonomo hat und OpenSpider fehlt – mit Empfehlung

| # | Funktion | Empfehlung | Begründung |
|---|---|---|---|
| A1 | **Logfile-Analyse** (Import, Bot-Verifikation, Abgleich mit dem Crawl) | **In OpenSpider aufnehmen**, nach 1.0 als neuer Roadmap-Punkt **R8 „Logfile-Import“** (lokal, Datei hinein, Ergebnis in der Crawl-Datei). Der automatische Zulauf vom Server und die Langzeithistorie bleiben **cloud-exklusiv**. | Logfile-Analyse ist ein klassisches Desktop-Werkzeug, und der Abgleich mit dem Crawl ist genau die Stärke eines Crawlers. Lokal bleiben die IPs auf dem Rechner des Nutzers. Den Bot-Katalog sollen beide Varianten teilen (Etappe E0). |
| A2 | **Mandanten, Konten, Pakete, Löschen je Website** | **Cloud-exklusiv** (evnxt). | Einzelplatz-Desktop braucht das nicht. OpenSpider bekommt nur die Crawl-Verwaltung aus M2. |
| A3 | **Monitoring** (Uptime, SSL, URL-Änderungen, Alarme) | **Cloud-exklusiv** (evnxt-Überwachung). R6 in OpenSpider schrumpft auf das gemeinsame Regelformat (siehe 5.3) plus „Monitoring light“ (geplante Crawls, Vergleich, Mail aus M2). | Kurze Takte und dauerhafte Verfügbarkeit leistet ein Desktop nicht. evnxt hat die Bausteine bereits. |
| A4 | **Indexierungspolitik je Projekt / „gewollt“** | **In OpenSpider aufnehmen** als Datei mit Erwartungen und Ausnahmen je Crawl-Konfiguration (Muster → unterdrückte Issues mit Grund). Kandidat für 1.x, zusammen mit R7. Dasselbe Dateiformat liest auch die Cloud. | Ohne diese Datei wird jedes Audit einer Affiliate- oder Shopseite von Scheinbefunden überlagert. Auch Agenten profitieren: Ein Befund mit „gewollt“ ist ein beantworteter Befund. |
| A5 | **Fallenwächter, dynamisches Limit mit Hochrechnung, Vollständigkeitsmeldung** | **In den OpenSpider-Kern** (`os-engine`), Kandidat für 1.x. | Das ist generische Crawl-Qualität. Gerade unbeaufsichtigte Agenten- und Server-Läufe brauchen eine ehrliche Antwort auf die Frage „vollständig oder nicht“. |
| A6 | **Aktive Soft-404-Probe** | **In OpenSpider aufnehmen** als Ergänzung zu ONPAGE-147 (zwei Abrufe zufälliger Pfade je Host), P2. | Das kostet wenig und ist zuverlässiger als eine Phrasenliste. |
| A7 | **Befund-Status „erledigt“, Nachprüfen einzelner Befunde** | Status: **Cloud-exklusiv** (Arbeitsablauf mehrerer Personen). Nachprüfen: OpenSpider deckt es mit dem Neu-Crawl einzelner URLs (M2) ab. | |
| A8 | **Seitenarchiv über Monate** | **Cloud-exklusiv.** OpenSpider speichert HTML je Crawl (CRAWL-063); die Historie entsteht aus mehreren Crawl-Dateien. | Archiv-Aufbewahrung ist eine Betriebsaufgabe mit Datenschutzbezug. |
| A9 | **Same-Server-Quellen** (Soll-URLs aus der Site-DB, Docroot-Dateiliste, Loopback) | **Cloud-exklusiv** für den Server-Zugriff. In OpenSpider genügt eine allgemeine **Orphan-Quelle „URL-Liste aus Datei“** (P2), damit eine exportierte Soll-Liste lokal genauso wirkt. | |
| A10 | **Agentic-Module** (Chat je Website, Wissen, Keywords/Belegung, Kannibalisierung, Wettbewerb, Backlinks/Spam, GEO-Tracking, SISTRIX, Keyword Planner) | **Cloud-exklusiv.** Überschneidungen mit M4/M5 (Maßnahmenplan, interne Links, AI-Search, DataForSEO) baut OpenSpider lokal mit eigenem Schlüssel des Nutzers, ohne die Cloud-Logik zu kopieren. | OpenSpider ist ein Werkzeug *für* Agenten und kein Agent. Abrechnungspflichtige Dauerläufe gehören auf den Server. |
| A11 | **Laien-Audit** (Maßnahmen „wer/Aufwand/Beleg“) | Gemeinsames **Ausgabeformat** mit AI-106 (Maßnahmenplan) festlegen: gleiche Felder, damit ein lokal erzeugter Plan in evnxt gelesen werden kann und umgekehrt. | |

---

## 4 Was OpenSpider kann und Autonomo fehlt

1. **JS-Rendering** mit Roh- gegen Render-Vergleich, gerendertem DOM und Screenshots (M2). Autonomo hat es seit Runde 6 ausgesetzt, aus Sorge um Plattenplatz und Browser-Pool.
2. **Skalierung und Speicher:** 1 Mio.+ URLs mit konstantem Durchsatz und begrenztem RAM in einer Datei je Crawl. Autonomo legt jede Linkinstanz als Postgres-Zeile ab (20,6 Mio. Zeilen bei 97 000 Seiten, 8,6 GB). Bei 100 000-Seiten-Aufträgen für viele Portale wächst das linear weiter.
3. **Portable Crawl-Datei** (`*.ospider`): weitergeben, archivieren, lokal öffnen, per SQL lesen. Autonomo exportiert nur CSV.
4. **Custom Search, Custom Extraction und Custom JavaScript** (M2). In Autonomo sind sie ausgesetzt.
5. **Accessibility (axe), Validation, Lesbarkeit, Rechtschreibung, Near-Duplicates, Link Score, Segmente, Visualisierungen, Carbon** (M1/M2).
6. **Crawl-Modi** Liste, SERP und Vergleich, dazu **Formular-Login, Cookies und Proxy** (M2).
7. **Google-APIs direkt im Crawl** (GA4, GSC URL Inspection, PSI/Lighthouse) und Linkmetriken (Majestic/Ahrefs/Moz) (M2).
8. **Exporte und Berichte:** XLSX, Bulk-Exporte, Sitemap-Generator, Looker Studio, Reports (M1/M2).
9. **Standard-Agentenschnittstelle:** MCP über STDIO und Streamable HTTP mit Werkzeugklassen, Kostenbestätigung, Kennzeichnung nicht vertrauenswürdiger Inhalte, Paging und gehärtetem Read-only-SQL. Dazu ein **CLI für headless Läufe** und Skill/Plugin.
10. **Paritätstests** gegen Referenzexporte und deterministische Golden-Tests je Funktion.
11. **Getrennter Prozess je Crawl:** Ein Absturz oder Neustart der Oberfläche beendet keinen Crawl. In Autonomo laufen Crawls im einzigen Backend-Prozess; jedes Deploy unterbricht sie, und sie müssen fortgesetzt werden.

---

## 5 Zielbild „Autonomo = Cloud-Variante“

### 5.1 Kernentscheidung (Vorschlag)

**OpenSpider wird die Crawl-Engine der Cloud. Autonomo bleibt Auftrags-, Analyse- und KI-Schicht, evnxt bleibt Oberfläche und Mandantenverwaltung.** Die Kopplung läuft über **Prozessgrenze und Crawl-Datei**, nicht über eine Bibliothek:

- Autonomo startet für jeden Crawl das `openspider`-CLI (Linux x86_64) als eigenen, gehärteten Prozess (eigener Systemnutzer, systemd-Instanz oder Arbeiterwarteschlange mit fester Parallelität). Die Konfiguration kommt aus einer Datei (Lastprofil, User-Agent `evnxtAuditBot`, Limits). Das Ergebnis ist eine `*.ospider`-Datei je Crawl.
- Autonomo liest die Datei **nur lesend** (Python `sqlite3`, `mode=ro`) und übernimmt, was seine Nachlauf-Stufen brauchen. Die eigenen Regeln und KI-Stufen laufen zunächst unverändert weiter.
- Eine **Rust-Bibliothek in Python** (PyO3) wird **nicht** gebaut. Sie würde zwei Release-Zyklen verkoppeln, Abstürze in den einzigen Backend-Prozess tragen und die Clean-Room- und Lizenzgrenze (Apache-2.0 gegen proprietär) unscharf machen. Das CLI ist bereits der vorgesehene Zugang (Architektur-Grundsatz 1 „Kern ohne UI“).

### 5.2 Gemeinsames Crawl-Format

- `*.ospider` ist das Austauschformat in beide Richtungen. Dazu kommen ein **Manifest** (Schema-Version, OpenSpider-Version, Konfiguration, Start-URL, Zeitraum, Vollständigkeit, User-Agent) und eine **stabile, dokumentierte Lese-Schicht** (Sichten mit Präfix, wie sie AGENT-014/AI-100 ohnehin vorsehen). Fremde Leser wie Autonomo, evnxt oder Skripte greifen nur auf diese Sichten zu, nie auf interne Tabellen. Interne Migrationen (z. B. `url_counters`, Index-Lebenszyklen) brechen dann keinen Leser.
- **Befundkatalog:** Eine Zuordnungstabelle verbindet Autonomo-`rule_id` und OpenSpider-Issue (Tab/Filter-Kennung aus `M1-/M2-filters.toml`). Für generische technische und Onpage-Befunde ist der **OpenSpider-Katalog führend**. Autonomo-eigene Befunde (Politik, Log, GEO, Keywords, Aktualität) behalten einen eigenen Namensraum. Erklärtexte bleiben getrennt: OpenSpider hat eigene Clean-Room-Texte, Autonomo Laientexte. Übernommen werden **nur Kennungen, keine Texte**. Die Recherche zum Referenzprodukt in den Autonomo-Dokumenten (Issue-Listen der Referenz) wird für OpenSpider nicht verwendet.
- **Gemeinsame Datendateien** (versioniert im OpenSpider-Repo, von Autonomo eingebunden): Bot- und KI-Crawler-Katalog (Autonomo `bots.py` ↔ AI-120), Lastprofile (eigen/fremd/kunde ↔ CRAWL-145–147), Politik- und Ausnahmeformat (A4), Maßnahmenplan-Schema (A11).

### 5.3 MCP und Austausch Desktop ↔ Cloud

- **Keine Zusammenlegung der MCP-Server.** Der evnxt-MCP bleibt der mandantenfähige Cloud-Zugang (Scopes, Limits, OAuth). Der OpenSpider-MCP bleibt der lokale Zugang (STDIO, Loopback-HTTP). Beide bekommen **dieselben Namen, Parameter und Antwortformen** für die Crawl-Lesewerkzeuge (`list_crawls`, `crawl_overview`, `query_urls`, `url_details`, `list_issues`, `compare_crawls`, Paging nach AGENT-005, Datenblock nach AGENT-004). Ein Agent nutzt dann beide mit einem Modell im Kopf.
- **Umsetzung in der Cloud:** evnxt bedient diese Werkzeuge, indem es serverseitig `openspider mcp --read-only --crawl-dir <Ordner des Portals>` aufruft (STDIO-Prozess je Sitzung bzw. Loopback-HTTP hinter dem evnxt-Proxy). Mandantentrennung entsteht über **einen Crawl-Ordner je Portal**, Authentifizierung und Scopes übernimmt evnxt. Damit bleibt AGENT-002 unverändert (nur Loopback), und die heutigen Spiegeltabellen (`mcp_crawl_spiegel`, `mcp_crawl_seiten`) entfallen schrittweise.
- **Desktop ↔ Cloud** (R4b): Die Desktop-App verbindet sich per evnxt-API-Schlüssel und bietet
  - „Cloud-Crawl öffnen“: lädt die `.ospider` des Monats-Crawls herunter.
  - „Crawl an evnxt senden“: lädt einen lokalen Crawl hoch, um Audit, KI, Logabgleich und Bericht in der Cloud zu nutzen.
  - „Crawl in der Cloud starten“: für große oder regelmäßige Läufe.

  Vorher ist ein kleiner REST-Satz in evnxt nötig (`/api/v1/websites/{w}/crawls`, Scopes `crawls:read` und `crawls:write`).

### 5.4 Migrationspfad ohne Big Bang

| Etappe | Inhalt | Voraussetzung | Ergebnis / Abnahme |
|---|---|---|---|
| **E0 Verträge** (sofort, klein, parallel zu M2) | Manifest und Lese-Sichten von `.ospider` als öffentliche Schnittstelle spezifizieren; Befund-Zuordnungstabelle; gemeinsame Datendateien (Bot-Katalog, Lastprofile); **Linux-CI-Job für `os-cli` aus OUT-264 in M2 vorziehen**, inklusive Golden-Vergleich und headless Chromium unter Linux; Secret-Store für Server (Umgebung/Datei statt Schlüsselbund) | keine | Spec-Einträge und CI grün auf `ubuntu-latest` |
| **E1 Schattenbetrieb** | `openspider` auf dem VPS als Crawl-Arbeiter neben dem Python-Crawler. Autonomo-Importer (`ospider_import`) schreibt die Daten ins **bestehende** Schema (`crawl_pages`, `crawl_links`, `crawl_resources`, `crawl_sitemap_urls`), Nachlauf und 219 Regeln laufen unverändert. Doppelläufe auf caloone.de und rotweine.org, Abweichungsliste je Regel (Autonomos Prinzip „Lauf == Nachrechnen“). Den ungenutzten Wrapper des Referenzcrawlers durch einen OpenSpider-Wrapper für die Agenten ersetzen | M1-CLI (statische Crawls), E0 | Befundzahlen je Regel gleich oder erklärt; kein Einfluss auf Kunden |
| **E2 Umschalten Kundencrawls** | Schalter je Projekt: Profil `kunde` (evnxt-Monatscrawls) läuft über OpenSpider. Python-Fetcher bleibt zwei Monatszyklen als Rückfall und wird dann entfernt. `ENGINE_VERSION` wird erhöht, damit der Monatsvergleich den Engine-Wechsel als Ursache ausweist statt Scheinänderungen. JS-Rendering und Custom Extraction werden für Kunden nutzbar, `.ospider`-Download in evnxt | M2 (Rendering, Profile CRAWL-145–147), E1 abgenommen | Monatscrawls aller Portale über OpenSpider, Fehlerquote ≤ heute |
| **E3 Daten verschlanken, Regeln wandern** | `crawl_links` nicht mehr vollständig nach Postgres übernehmen; Link-Abfragen gehen gegen die Crawl-Datei bzw. `url_counters`. Generische Regeln, die ein OpenSpider-Issue haben, liest Autonomo aus der Datei statt sie selbst zu rechnen. Autonomo-eigene Regeln bleiben in Python | E2, Lese-Sichten stabil | Postgres-Wachstum je Crawl um Größenordnungen kleiner, eine Wahrheit für generische Befunde |
| **E4 Desktop ↔ Cloud, gemeinsamer MCP** (= R4b) | evnxt-REST für Crawls, App-Verbindung, Upload/Download, evnxt-MCP-Crawlwerkzeuge über `openspider mcp` | Release 1.0 (M3), E3 | Ein lokal erzeugter Crawl erscheint in evnxt mit Audit, und umgekehrt |
| **E5 Monitoring** (= R6, Cloud) | Regelbasierte Tests im gemeinsamen Format, Alarme und KPI-Verlauf in evnxt. Desktop: Monitoring light mit M2-Mitteln | E2 | wie R6 |

### 5.5 Abhängigkeit zu R4 und R6

- **R4 aufteilen.** **R4a „Engine im Serverbetrieb“** (E0–E3) hängt nur an M1/M2 und sollte **nicht** „ganz am Schluss“ kommen, sondern parallel zu M2/M3 laufen. Die Etappen sind klein, gefahrlos (Schattenbetrieb) und liefern echte Lastdaten aus Kundencrawls bis 100 000 Seiten, die M2 sonst nur synthetisch misst. **R4b „Desktop ↔ Cloud“** (E4) bleibt nach 1.0.
- **R5 (Mobile)** setzt R4b voraus.
- **R6 (Monitoring)** setzt nur R4a voraus, denn die Engine muss in der Cloud laufen. Die Oberfläche entsteht in evnxt, nicht in OpenSpider. Für OpenSpider bleibt aus R6 das Regelformat; der Rest steckt schon in M2 (Aufgabenplanung, Vergleich, Mail).
- **R3 (Fokussierter Crawl)** und **R7** profitieren direkt von Autonomo-Erfahrung (Seitentyp-Cluster, Politik, Fallen, Aktualität) und sollten die Formate aus E0 nutzen.

### 5.6 Risiken

| Risiko | Gegenmaßnahme |
|---|---|
| Verhaltensunterschiede (URL-Normalisierung, Weiterleitungen, Indexierbarkeit, robots) verändern Kunden-Befundzahlen beim Umschalten | Schattenbetrieb E1 mit Abweichungsliste, Engine-Version im Vergleich, Umschalten je Projekt |
| Schema-Drift der Crawl-Datei durch M2-Migrationen bricht den Importer | Nur dokumentierte Lese-Sichten lesen, Schema-Version im Manifest prüfen, CLI-Version je Deploy fest pinnen |
| Doppelpflege der Regeln in Rust und Python | Zuordnungstabelle, generische Regeln wandern in E3 nach OpenSpider, Autonomo behält nur Eigenes |
| Ressourcen auf dem geteilten VPS (16 Kerne, 30 GB RAM, Volume zu 68 % belegt): Chromium-Rendering und Crawl-Dateien von rund 0,6 GB je 100 000-Seiten-Crawl × 13 Monate × Portale | Rendering nur auf Anforderung je Projekt; feste Arbeiterzahl; cgroup-Grenzen wie beim Backend; Crawl-Dateien früh auf Object Storage legen (dieselbe Schwelle wie beim Seitenarchiv) |
| Sicherheit: Autonomo hat keine eigene Anmeldung; jeder neue Dienst vergrößert die Angriffsfläche | OpenSpider-Arbeiter öffnet keine Ports; MCP nur Loopback hinter evnxt; „Private Ziele erlauben“ aus, Loopback nur für eigene Domains über Positivliste wie heute |
| Datenschutz Logfiles | Server-Logs bleiben in der Cloud; lokale Logfile-Analyse (R8) speichert nur in der Crawl-Datei des Nutzers |
| Clean-Room: Autonomo-Dokumente enthalten Recherche zum Referenzprodukt | Für OpenSpider nur Kennungen und eigene Spec verwenden, keine Texte oder Listen aus Autonomo übernehmen |
| Linux ist keine Zielplattform von OpenSpider | Nur CLI und Kern, kein Desktop. Linux-CI in E0 macht den Server-Betrieb zu einer geprüften Zusage statt eines Zufalls |

### 5.7 Folgearbeiten für Spec und Plan (nicht Teil dieses Dokuments)

- Spec: neuer Abschnitt zum Austauschformat (Manifest, Lese-Sichten) und OUT-264 auf P1 für das CLI heben (Linux-CI).
- Spec: neue Einträge Fallenwächter, Vollständigkeitsmeldung, aktive Soft-404-Probe, Politik-/Ausnahmedatei, Orphan-Quelle „Liste aus Datei“, benannte Lastprofile.
- Roadmap: R4 in R4a/R4b teilen, R8 „Logfile-Import“ aufnehmen, R6 auf „Cloud-Oberfläche in evnxt, Regelformat in OpenSpider“ zuschneiden.
- Autonomo/evnxt (eigene Sitzungen): Importer und Schattenbetrieb (E1), REST-Crawls und MCP-Anbindung (E4).

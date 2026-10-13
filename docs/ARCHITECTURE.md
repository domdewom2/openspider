# OpenSpider – Architektur

Ziel: Funktionsparität mit Screaming Frog SEO Spider 24.3, aber schneller und sparsamer im Speicher. Plattformen: macOS (arm64, x64) und Windows (x64). Lizenz: Apache-2.0.

**Mindestplattformen** (Entscheidung (Fable-Empfehlung) vom 13.10.2026): macOS 13.0 oder neuer (arm64/x64; WKWebView ab Safari 16.2 für `color-mix(in oklch)`, `forced-colors`, Canvas-DPR), Windows 10 ab 1809 bzw. Windows 11, x64, mit WebView2-Evergreen-Runtime. Festgeschrieben in `apps/desktop/src-tauri/tauri.conf.json` (`bundle.macOS.minimumSystemVersion = "13.0"`, M2-U00) und im M3-Installerplan (OUT-260 … 262).

## Grundsätze

1. **Kern ohne UI.** Die gesamte Crawl- und Analyse-Logik lebt in Rust-Crates und ist über CLI und Tauri gleichermaßen nutzbar. Die UI ist eine reine Ansicht auf den Crawl-Speicher.
2. **Disk-first.** Jeder Crawl ist eine SQLite-Datei (`*.ospider`, WAL-Modus). Kein „Memory vs. Database“-Modus nötig: SQLite + Page-Cache deckt beides ab. Millionen URLs ohne RAM-Explosion.
3. **Baum je Seite, Speicher begrenzt.** HTML wird je Seite mit html5ever (über `scraper`) in einen Baum geparst, weil die HTML5-Tree-Builder-Semantik (z. B. „liegt im `<head>`“, TECH-003) einen Baum verlangt (Planentscheidung K-09). Der Baum lebt nur während der Verarbeitung einer Seite; sein Speicher ist durch `limits.max_page_kb` begrenzt. Ergebnisse fließen sofort in die Crawl-Datei – es gibt nie einen crawlweiten DOM oder Seiteninhalt im RAM.
4. **Deterministisch testbar.** Jede Funktion aus `docs/spec` hat eine lokale Testfixture und Golden-Outputs.

## Crates (Cargo-Workspace)

| Crate | Aufgabe | Wichtige Abhängigkeiten |
|---|---|---|
| `os-model` | Gemeinsame Typen: `CrawlConfig`, Schreib-/Parse-Modell, IDs, `Clock`/`IdSource`, Indexierbarkeitsmodell (`indexability::evaluate`, reine Funktion) | serde, schemars |
| `os-url` | URL-Normalisierung, Umschreibung, Include/Exclude, Segmente | url, fancy-regex, psl |
| `os-fetch` | HTTP-Client, robots.txt, Rate-Limit, Auth, Proxy, Cookies, HTTP/2 | reqwest (rustls), cookie_store; **eigener RFC-9309-Parser** für robots.txt, **eigener Token-Bucket** für das Rate-Limit (gilt für alle Abrufe) |
| `os-parse` | HTML-Extraktion: Links, Meta, Headings, Canonicals, hreflang, Bilder, Directives, Sitemaps, PDF-Metadaten; ab M2 strukturierte Daten, Custom Search/Extraction | scraper/html5ever (**Baum je Seite**, K-09), encoding_rs, quick-xml, lopdf, serde_json |
| `os-render` | JS-Rendering über Chromium (CDP), Screenshots, gerendertes DOM | chromiumoxide |
| `os-store` | Crawl-Datei (SQLite), Schema-Migrationen, Abfragen für UI-Tabellen (Paging, Filter, Sortierung), Änderungsfeed (`urls.change_seq`/`crawl_meta.change_seq`, vom Writer je Batch gestempelt), Text-Kollation `OS_TEXT` (Unicode-Root, sprachunabhängig – eine Sortierregel für UI, Export und MCP) | rusqlite (bundled, collation), icu_collator |
| `os-engine` | Frontier, Scheduler, Worker-Pool, Pause/Resume, Modi (Spider/Liste/SERP/Vergleich); **ohne Abhängigkeit auf `os-analyze`** – Indexierbarkeit aus `os-model`, die Crawl-Analyse am Ende löst der Aufrufer über einen Hook aus | tokio |
| `os-analyze` | Filterlogik je Tab (SQL-Plan `views::ViewPlan`, Grundlage auch für die View-Caches der UI ab M2), Issues inkl. Erklärungstexten (what/why/fix, eine Quelle für UI, Export, MCP), Nachlauf-Analyse (Link Score, Duplikate per SimHash/MinHash, Orphans, Ketten, hreflang-Rücklinks, Sitemaps) | ab M2: roaring (View-Cache) |
| `os-export` | CSV, XLSX, Bulk-Exporte, Reports, Sitemap-Generator | csv, rust_xlsxwriter |
| `os-ai` | KI-Anbieter (Claude, OpenAI Responses + Decisions API, OpenRouter, Ollama), Embeddings, Decision-Engine (predicate/choice/score), Batch, Cache, Kostenschätzung/Budget, „Frag den Crawl“ (read-only SQL) | reqwest, keyring |
| `os-integrations` | GSC, GA4, PSI, Majestic/Ahrefs/Moz, DataForSEO (Labs, Backlinks, SERP, LLM Mentions), OAuth | reqwest, oauth2, keyring |
| `os-mcp` | MCP-Server (STDIO + Streamable HTTP auf Loopback mit Token), Werkzeugklassen read/crawl/write/cost, Prompts; Skill + Claude-Code-Plugin unter `integrations/` | rmcp |
| `os-app` (ab M2) | Tauri-freie Service-Schicht der Desktop-App (Entscheidung (Fable-Empfehlung) vom 13.10.2026): Sitzungszustand, View-Register und View-Caches, Ereignis-Coalescer, DTOs (ts-rs), Operationen mit Abbruch; transportunabhängiger Command-Dispatcher (Name + JSON) für Tauri-Commands, E2E-Bridge und In-App-HTTP-MCP; Lese-Pool für alle Datenbankzugriffe der App. **Keine Tauri-Abhängigkeit**; hängt nur an `os-model`, `os-store`, `os-analyze`, `os-engine`, `os-export`, `os-mcp` – `os-mcp` und `os-cli` hängen **nie** an `os-app` | ts-rs |
| `os-cli` | `openspider` Binary: headless Crawl, Export, `openspider mcp`, Scheduling-Ziel | clap |
| `os-testsite` | Lokaler Fixture-Server (axum) mit Szenarien aus `tests/fixtures`; synthetische Sites ohne Dateien (`synthetic:n=1000000,links=30` oder `[synthetic]` in `scenario.toml`), optionales Request-Log | axum |
| `os-parity` | Paritätsabgleich gegen Referenz-Exporte (`diff`, `run`) und Spec-Abdeckung aus Tags und Testergebnissen (`coverage`) | csv, toml, serde |
| `os-loadtest` | Lasttest-Runner (Binary, PERF-009): `os-loadtest run --scenario perf-001 --size small` bzw. `--size large`, schreibt `target/loadtest/report.json`; Trait `CrawlerUnderTest`, den `os-engine` später implementiert | os-testsite, clap |

## Desktop-App

`apps/desktop`: Tauri v2 + React + TypeScript + Vite; Konzept und Entscheidungen: `docs/plan/M2-ui-concept.md`.
- Schichten: React-Oberfläche → typisierte IPC-Wrapper → dünne Tauri-Command-Hüllen (`src-tauri`, je Domäne ein internes Plugin) → `os-app` → Kern-Crates. `openspider-desktop` ist der einzige Ort mit Tauri-Abhängigkeiten.
- Tabellen: Glide Data Grid (Canvas, MIT; `@glideapps/glide-data-grid@6.0.4-alpha24` exakt gepinnt, hinter eigenem Adapter; Rückfall: Fork/Vendoring → Fenster-Rebasing → eigenes Canvas-Grid) mit serverseitigem Paging (Offset auf versionierten Views) über `os-app`.
- Datenbankzugriffe der App ausschließlich über den Lese-Pool bzw. `spawn_blocking` in `os-app`, nie auf dem UI-Thread oder einem Tokio-Worker.
- Fortschritt/Ereignisse: ein Ereigniskanal je Fenster, im Kern gebündelt (≤ 10 Nachrichten/s über alle Ereignisarten, ≤ 1 Hz bei verdecktem/minimiertem Fenster); Quelle ist der Änderungsfeed der Crawl-Datei, damit auch Crawls fremder Prozesse live erscheinen.
- UI-Bausteine: React Aria Components, Zustand, TanStack Query, Fluent (i18n), Lucide, CodeMirror 6.
- Visualisierungen: D3 (Bäume über d3-hierarchy + Canvas, Wortwolken über d3-cloud), sigma.js/graphology (kraftgerichtete Graphen, WebGL), three.js (3D).

## KI & externe Daten (nach Release 1.0, M4/M5)

- Alle KI-Aufrufe laufen über `os-ai`; Fachlogik kennt nur „Frage“ bzw. „Aufgabe“, nie einen konkreten Anbieter.
- Decision-Engine: Fragen einmal anbieterneutral definiert; OpenAI Decisions API bevorzugt, sonst strukturierte Ausgabe über Claude/OpenAI/OpenRouter (Wahrscheinlichkeit dann als Selbsteinschätzung gekennzeichnet).
- Ergebnis-Cache je (Inhalts-Hash, Fragen-Version, Modell) in der Crawl-Datei; Kostenschätzung vor jedem Lauf, hartes Budget je Crawl.
- API-Schlüssel ausschließlich im OS-Schlüsselbund. In CI nur Mock-Server (`os-testsite`), keine echten API-Aufrufe.
- Spezifikation: `docs/spec/08-ai.md`, `docs/spec/09-dataforseo.md`.

## Meilensteine

**Version 1.0 (stabil)** = M1 + M2 + M3. KI-API-Funktionen und DataForSEO kommen erst danach als eigene Feature-Releases.

| M | Inhalt | Abnahme durch Fable |
|---|---|---|
| M1 | Crawl-Kern, Speicher, CLI, Basis-Tabs (P0 aus 01/02/03/06), MCP headless lesend (10: AGENT P0), Lasttests klein | Paritätstests gegen SF-Free-Exporte, PERF-001/003/005 |
| M2 | Desktop-UI, alle P0/P1-Tabs und Filter, Reports/Exporte, JS-Rendering, Google-APIs (GSC, GA4, PSI) und Linkmetriken (04 ohne KI), MCP-Menü, Skill + Plugin | volle Paritätsmatrix P0+P1 (ohne KI), PERF komplett |
| M3 | Signierte Installer (dmg/exe/msi), Auto-Update, Homebrew/winget → **Release 1.0** | Installationstest macOS arm64/x64, Windows |
| M4 | KI-Funktionen: 08-ai, KI-Teile aus 04 (Anbieter, Prompts, Embeddings, semantische Ähnlichkeit), AGENT-030 → Release 1.x | Mock-Tests + Stichprobe mit echten Schlüsseln |
| M5 | DataForSEO: 09-dataforseo → Release 1.y | Mock-Tests + Stichprobe mit echtem Konto |

## Datenfluss

```
Config → os-engine ─┬→ os-fetch ──→ os-parse ──→ os-store (urls, links, resources, issues…)
                    └→ os-render (optional)            │
                                                       ├→ os-analyze (live + Nachlauf)
                                                       ├→ os-export / os-cli
                                                       └→ os-app → Tauri-Commands → UI
```

Abhängigkeitsrichtung: `os-engine` kennt `os-analyze` nicht; `os-app` (ab M2) sitzt über allen Kern-Crates, und kein Kern-Crate (auch nicht `os-mcp`) hängt an `os-app` – Sitzungszugriff für In-App-MCP-Werkzeuge kommt über ein optionales Trait-Objekt im `os_mcp::ToolContext`, das `os-app` bereitstellt. Nach Crawl-Ende ruft die Engine nur einen Hook (`EngineDeps.on_finished`); CLI bzw. App starten darüber die Crawl-Analyse (`os_analyze::analysis::run_after_crawl`). Verbindliche Crate-Abhängigkeiten für M1: `docs/plan/M1.md` Abschnitt 3.2.

## Qualitätssicherung

- `cargo test --workspace`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo deny check licenses bans sources` (Lizenzpolitik in `deny.toml`: nur permissive Lizenzen und MPL-2.0, kein GPL/LGPL/AGPL).
- Paritätstests: Fixtures in `tests/fixtures/<szenario>/` + Golden-Exporte in `tests/golden/`. Referenzwerte aus Screaming Frog Free (≤500 URLs) werden manuell exportiert und abgelegt.
- CI: GitHub Actions Matrix `macos-14` (arm64), `macos-13` (x64), `windows-latest`. Zusätzlich die Jobs `rustfmt` und `cargo-deny` (ubuntu).
- Desktop-UI (ab M2): vitest (Logik, Komponenten), Playwright gegen Frontend + Kern-Bridge in WebKit und Chromium (Hauptlast der E2E-Tests), WebDriver gegen die echte App auf beiden Plattformen (Windows `tauri-driver`, macOS WebdriverIO-Service mit eingebettetem Server `tauri-plugin-wdio-webdriver`), Bedienungshilfen-Smoke nur für native macOS-Menüs; Stufenmodell in `docs/plan/M2-ui-concept.md` 7.3.
- Fortschritt: `docs/spec/PARITY.md` – Status je Funktions-ID (offen / in Arbeit / implementiert / verifiziert).

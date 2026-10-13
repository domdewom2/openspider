# OpenSpider – Architektur

Ziel: Funktionsparität mit Screaming Frog SEO Spider 24.3, aber schneller und sparsamer im Speicher. Plattformen: macOS (arm64, x64) und Windows (x64). Lizenz: Apache-2.0.

## Grundsätze

1. **Kern ohne UI.** Die gesamte Crawl- und Analyse-Logik lebt in Rust-Crates und ist über CLI und Tauri gleichermaßen nutzbar. Die UI ist eine reine Ansicht auf den Crawl-Speicher.
2. **Disk-first.** Jeder Crawl ist eine SQLite-Datei (`*.ospider`, WAL-Modus). Kein „Memory vs. Database“-Modus nötig: SQLite + Page-Cache deckt beides ab. Millionen URLs ohne RAM-Explosion.
3. **Streaming statt Vollbaum.** HTML wird mit einem Streaming-Parser verarbeitet; nur für XPath/Custom JS wird ein DOM aufgebaut.
4. **Deterministisch testbar.** Jede Funktion aus `docs/spec` hat eine lokale Testfixture und Golden-Outputs.

## Crates (Cargo-Workspace)

| Crate | Aufgabe | Wichtige Abhängigkeiten |
|---|---|---|
| `os-model` | Gemeinsame Typen: `CrawlConfig`, `UrlRecord`, `Link`, `Issue`, IDs | serde |
| `os-url` | URL-Normalisierung, Umschreibung, Include/Exclude, Segmente | url, regex |
| `os-fetch` | HTTP-Client, robots.txt, Rate-Limit, Auth, Proxy, Cookies, HTTP/2 | reqwest (rustls), texting_robots, governor |
| `os-parse` | HTML-Extraktion: Links, Meta, Headings, Canonicals, hreflang, Bilder, Directives, strukturierte Daten, Custom Search/Extraction | lol_html, scraper, sxd-xpath bzw. libxml, serde_json |
| `os-render` | JS-Rendering über Chromium (CDP), Screenshots, gerendertes DOM | chromiumoxide |
| `os-store` | Crawl-Datei (SQLite), Schema-Migrationen, Abfragen für UI-Tabellen (Paging, Filter, Sortierung) | rusqlite (bundled) |
| `os-engine` | Frontier, Scheduler, Worker-Pool, Pause/Resume, Modi (Spider/Liste/SERP/Vergleich) | tokio |
| `os-analyze` | Filterlogik je Tab, Issues, Nachlauf-Analyse (Link Score, Duplikate per SimHash/MinHash, Orphans, Ketten, hreflang-Rücklinks, Sitemaps) | |
| `os-export` | CSV, XLSX, Bulk-Exporte, Reports, Sitemap-Generator | csv, rust_xlsxwriter |
| `os-integrations` | GSC, GA4, PSI, Majestic/Ahrefs/Moz, KI-Anbieter, OAuth | reqwest, oauth2 |
| `os-cli` | `openspider` Binary: headless Crawl, Export, Scheduling-Ziel | clap |
| `os-testsite` | Lokaler Fixture-Server (axum) mit Szenarien aus `tests/fixtures` | axum |

## Desktop-App

`apps/desktop`: Tauri v2 + React + TypeScript + Vite.
- Tabellen: Glide Data Grid (Canvas, MIT) mit serverseitigem Paging über Tauri-Commands → `os-store`.
- Fortschritt/Ereignisse: Tauri-Events (Throttling auf ~10 Hz).
- Visualisierungen: D3 / Cytoscape.js.

## Datenfluss

```
Config → os-engine ─┬→ os-fetch ──→ os-parse ──→ os-store (urls, links, resources, issues…)
                    └→ os-render (optional)            │
                                                       ├→ os-analyze (live + Nachlauf)
                                                       ├→ os-export / os-cli
                                                       └→ Tauri-Commands → UI
```

## Qualitätssicherung

- `cargo test --workspace`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo deny check licenses`.
- Paritätstests: Fixtures in `tests/fixtures/<szenario>/` + Golden-Exporte in `tests/golden/`. Referenzwerte aus Screaming Frog Free (≤500 URLs) werden manuell exportiert und abgelegt.
- CI: GitHub Actions Matrix `macos-14` (arm64), `macos-13` (x64), `windows-latest`.
- Fortschritt: `docs/spec/PARITY.md` – Status je Funktions-ID (offen / in Arbeit / implementiert / verifiziert).

# Workspace crates

All 18 members below come from the root [workspace manifest](../../../../Cargo.toml). Module names are checked against each `src/lib.rs`. Dependency lists below are **direct normal internal dependencies** plus selected direct external dependencies, not a transitive graph or dev-dependency list. Optional entries are still listed; target-specific dependencies and features are in the linked manifests. Some crate-level comments retain historical scaffold notes: use current modules and function bodies to establish behavior.

## os-model

Shared IDs, configuration, parse/write records, cancellation and clock sources. See [module root](../../../../crates/os-model/src/lib.rs) and [manifest](../../../../crates/os-model/Cargo.toml).

Key modules: `config`, `ids`, `time`, `parse`, `write`, `cancel`.

Direct internal dependencies: none. Selected external dependencies: `serde`, `serde_json`, `toml`, `thiserror`, `uuid`, `time`, `schemars`.

## os-url

URL normalization, rewrite rules, scope and comparisons. See [module root](../../../../crates/os-url/src/lib.rs) and [manifest](../../../../crates/os-url/Cargo.toml).

Key modules: `normalize`, `rewrite`, `scope`, `rules`, `compare`.

Direct internal dependencies: `os-model`. Selected external dependencies: `url`, `thiserror`, `fancy-regex`, `psl`, `idna`, `percent-encoding`.

## os-fetch

HTTP fetching, robots, pacing, authentication and secret-store boundary. See [module root](../../../../crates/os-fetch/src/lib.rs) and [manifest](../../../../crates/os-fetch/Cargo.toml).

Key modules: `client`, `robots`, `rate`, `net`, `secrets`.

Direct internal dependencies: `os-model`, `os-url`. Selected external dependencies: `rustls`, `rustls-platform-verifier`, `sha2`, `url`, `thiserror`, `tracing`, `tokio`.

## os-parse

HTML and header extraction, sitemaps, structured data and validation. See [module root](../../../../crates/os-parse/src/lib.rs) and [manifest](../../../../crates/os-parse/Cargo.toml).

Key modules: `dom`, `links`, `headers`, `sitemap`, `sd`, `validation`.

Direct internal dependencies: `os-model`, `os-url`. Selected external dependencies: `xee-xpath`, `url`, `scraper`, `html5ever`, `ego-tree`, `encoding_rs`, `quick-xml`.

## os-render

Chromium/CDP rendering and browser-side checks. See [module root](../../../../crates/os-render/src/lib.rs) and [manifest](../../../../crates/os-render/Cargo.toml).

Key modules: `api`, `axe`, `login`, `snippets`.

Direct internal dependencies: `os-model`. Selected external dependencies: `chromiumoxide`, `serde`, `serde_json`, `thiserror`, `tracing`, `tokio`, `futures-util`.

## os-store

Crawl-file persistence, migrations, counters and public read API. See [module root](../../../../crates/os-store/src/lib.rs) and [manifest](../../../../crates/os-store/Cargo.toml).

Key modules: `open`, `schema`, `writer`, `profile`, `read_api`, `queries`.

Direct internal dependencies: `os-model`. Selected external dependencies: `chacha20poly1305`, `argon2`, `zstd`, `rusqlite`, `thiserror`, `dirs`, `serde_json`.

## os-engine

Frontier, workers, crawl modes, pause/resume and resource protection. See [module root](../../../../crates/os-engine/src/lib.rs) and [manifest](../../../../crates/os-engine/Cargo.toml).

Key modules: `crawl`, `frontier`, `worker`, `lifecycle`, `memory`, `disk_guard`.

Direct internal dependencies: `os-model`, `os-url`, `os-fetch`, `os-parse`, `os-store`, `os-render`, `os-integrations`, `os-testsite`. Selected external dependencies: `sysinfo`, `serde_json`, `url`, `thiserror`, `tracing`, `tokio`, `bytes`.

## os-analyze

Post-crawl analysis, issues, filter registry and view sessions. See [module root](../../../../crates/os-analyze/src/lib.rs) and [manifest](../../../../crates/os-analyze/Cargo.toml).

Key modules: `analysis`, `tabs`, `views`, `issues`, `search`, `compare`.

Direct internal dependencies: `os-model`, `os-store`. Selected external dependencies: `fancy-regex`, `rusqlite`, `thiserror`, `tracing`, `serde`, `serde_json`, `toml`.

## os-export

CSV/XLSX, tab/detail/bulk exports, reports and sitemap generation. See [module root](../../../../crates/os-export/src/lib.rs) and [manifest](../../../../crates/os-export/Cargo.toml).

Key modules: `pipeline`, `tab_export`, `reports`, `csv`, `xlsx`, `sitemap_gen`.

Direct internal dependencies: `os-integrations`, `os-model`, `os-store`, `os-analyze`, `os-parse`, `os-url`. Selected external dependencies: `image`, `fancy-regex`, `toml`, `thiserror`, `tracing`, `serde`, `serde_json`.

## os-integrations

External API adapters, OAuth, quotas and crawl integration. See [module root](../../../../crates/os-integrations/src/lib.rs) and [manifest](../../../../crates/os-integrations/Cargo.toml).

Key modules: `core`, `oauth`, `queue`, `quota`, `gsc`, `ga4`, `psi`.

Direct internal dependencies: `os-model`, `os-store`, `os-fetch`. Selected external dependencies: `aes-gcm`, `webbrowser`, `reqwest`, `oauth2`, `keyring`, `serde`, `serde_json`.

## os-mcp

MCP tools, permissions, sessions, STDIO and protected HTTP transport. See [module root](../../../../crates/os-mcp/src/lib.rs) and [manifest](../../../../crates/os-mcp/Cargo.toml).

Key modules: `server`, `permissions`, `tools`, `session`, `transport`, `http`.

Direct internal dependencies: `os-engine`, `os-fetch`, `os-model`, `os-store`, `os-analyze`, `os-export`, `os-integrations`, `os-render`. Selected external dependencies: `axum`, `getrandom`, `clap`, `rmcp`, `serde`, `serde_json`, `toml`.

## os-parity

Export comparison and specification coverage reporting. See [module root](../../../../crates/os-parity/src/lib.rs) and [manifest](../../../../crates/os-parity/Cargo.toml).

Key modules: `diff`, `normalize`, `mapping`, `run`, `coverage`.

Direct internal dependencies: `os-model`, `os-testsite`. Selected external dependencies: `clap`, `csv`, `toml`, `serde`, `serde_json`, `thiserror`, `tokio`.

## os-cli

The openspider command line: validation, execution, exports and tasks. See [module root](../../../../crates/os-cli/src/lib.rs) and [manifest](../../../../crates/os-cli/Cargo.toml).

Key modules: `args`, `plan`, `exec`, `help`, `run_task`.

Direct internal dependencies: `os-model`, `os-url`, `os-fetch`, `os-store`, `os-engine`, `os-analyze`, `os-export`, `os-mcp`, `os-sched`, `os-integrations`, `os-render`. Selected external dependencies: `time`, `zip`, `sysinfo`, `sys-locale`, `clap`, `serde_json`, `thiserror`.

## os-testsite

Local fixture server, mocks, synthetic sites and TLS test helpers. See [module root](../../../../crates/os-testsite/src/lib.rs) and [manifest](../../../../crates/os-testsite/Cargo.toml).

Key modules: `client`, `golden`, `mocks`, `synthetic`, `tls`.

Direct internal dependencies: none. Selected external dependencies: `h2`, `csv`, `axum`, `serde`, `tokio`, `toml`, `serde_json`.

## os-loadtest

Synthetic-site load runner, child crawler measurement and reports. See [module root](../../../../crates/os-loadtest/src/lib.rs) and [manifest](../../../../crates/os-loadtest/Cargo.toml).

Key modules: `scenario`, `runner`, `engine`, `rss`, `report`.

Direct internal dependencies: `os-model`, `os-fetch`, `os-store`, `os-engine`, `os-testsite`. Selected external dependencies: `clap`, `serde`, `serde_json`, `tokio`, `tempfile`, `tracing`.

## os-app

Tauri-free desktop services, command dispatch, sessions, DTOs and operations. See [module root](../../../../crates/os-app/src/lib.rs) and [manifest](../../../../crates/os-app/Cargo.toml).

Key modules: `commands`, `dto`, `registry`, `runtime`, `pool`, `session`, `views`, `write_queue`.

Direct internal dependencies: `os-model`, `os-url`, `os-fetch`, `os-parse`, `os-store`, `os-analyze`, `os-engine`, `os-export`, `os-mcp`, `os-integrations`, `os-render`, `os-sched`. Selected external dependencies: `tempfile`, `tracing-subscriber`, `zip`, `sysinfo`, `rusqlite`, `serde`, `serde_json`.

## os-sched

Task definitions, history, OS scheduler adapters and completion mail. See [module root](../../../../crates/os-sched/src/lib.rs) and [manifest](../../../../crates/os-sched/Cargo.toml).

Key modules: `tasks`, `history`, `planner`, `mail`.

Direct internal dependencies: `os-model`, `os-store`, `os-fetch`. Selected external dependencies: `lettre`, `serde`, `serde_json`, `toml`, `thiserror`, `tracing`, `tokio`.

## openspider-desktop

Tauri host: plugins, windows, native integration and asset protocol. See [module root](../../../../apps/desktop/src-tauri/src/lib.rs) and [manifest](../../../../apps/desktop/src-tauri/Cargo.toml).

Key modules: `commands`, `events`, `window`, `open`, `assets`.

Direct internal dependencies: `os-model`, `os-app`. Selected external dependencies: `tauri`, `tauri-plugin-dialog`, `tauri-plugin-clipboard-manager`, `tauri-plugin-notification`, `tauri-plugin-os`, `tauri-plugin-single-instance`, `tauri-plugin-deep-link`.

<p align="center"><img src="assets/brand/openspider-logo.png" width="180" alt="OpenSpider Logo"></p>

# OpenSpider

**The crawler for the agentic era.** OpenSpider is an open-source website crawler and technical SEO auditor built for AI agents and humans alike – the best crawling tool you can get, for macOS (Apple Silicon & Intel) and Windows.

- **Agent-native:** a built-in MCP server lets Claude, Codex and other agents start crawls, query results and run audits – with read-only defaults, cost guards and prompt-injection-safe outputs.
- **Built for large sites:** disk-first storage (one SQLite file per crawl), constant memory, crash-safe resume – 1 million URLs with under 0.5 GB RAM.
- **Open data:** every crawl is a plain SQLite file you can query with SQL, Python or any BI tool.
- **Complete technical SEO:** status codes, redirects, canonicals, hreflang, directives, duplicates, content, images, links, sitemaps, structured data and more – plus CLI, scheduling and exports.
- **Fast and native:** Rust core, Tauri desktop app, small installers.

> Status: early development (milestone 1 of 3 towards 1.0). See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/spec](docs/spec) and the [roadmap after 1.0](docs/plan/ROADMAP-nach-1.0.md).

Website: [openspider.de](https://openspider.de)

## License

Apache-2.0 – see [LICENSE](LICENSE).

The OpenSpider name and logo (`assets/brand/`) are not covered by the Apache-2.0 license; please do not use them to brand forks or derived products without permission.

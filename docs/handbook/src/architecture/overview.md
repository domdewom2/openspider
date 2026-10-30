# Architecture overview

The core is independent of the desktop UI. A crawl persists facts to a disk-first SQLite `.ospider` file; analysis and presentation read from that file. These are the [architecture principles](../../../ARCHITECTURE.md), implemented across the [workspace crates](crates.md).

```text
Crawl: frontier → fetch → parse → optional Chromium rendering
                        ↓ persisted batches
                    Crawl file (.ospider)
                        ↓
               Analysis → views / issues / reports
                        ↓
              Desktop UI / CLI / MCP / exports
```

The [engine](../../../../crates/os-engine/src/lib.rs) coordinates URL admission, workers, lifecycle and persistence. [Fetch](../../../../crates/os-fetch/src/lib.rs) supplies HTTP/robots/rate limiting; [parse](../../../../crates/os-parse/src/lib.rs) extracts page facts; [render](../../../../crates/os-render/src/lib.rs) provides the optional Chromium path. The [store](../../../../crates/os-store/src/lib.rs) owns schema, batches, counters and queries.

The engine does not depend on analysis in its normal dependencies. It exposes `EngineDeps::on_finished`; callers connect post-crawl analysis through that hook. Check [engine types](../../../../crates/os-engine/src/types.rs) and its [manifest](../../../../crates/os-engine/Cargo.toml), rather than introducing an engine → analysis dependency.

[Analysis](../../../../crates/os-analyze/src/lib.rs) supplies filters, view sessions, issues and post-crawl computations. [Exports](../../../../crates/os-export/src/lib.rs), [CLI](../../../../crates/os-cli/src/lib.rs) and [MCP](../../../../crates/os-mcp/src/lib.rs) use the core directly. The desktop goes through the Tauri-free [os-app service](../../../../crates/os-app/src/lib.rs), then the thin [Tauri host](../../../../apps/desktop/src-tauri/src/lib.rs).

App database work belongs on blocking workers in the [read pool](../../../../crates/os-app/src/pool.rs) or [write queue](../../../../crates/os-app/src/write_queue.rs). Reads use bounded connections; cached read paths keep dedicated slots, and app writes are serialized. See [desktop architecture](desktop-app.md) and [decision entries](../decisions/architecture.md).

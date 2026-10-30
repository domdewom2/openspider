# Glossary

Terms used in the handbook. The source column points to the implementation or requirement defining the meaning here.

| English | Deutsch | Meaning / source |
|---|---|---|
| Crawl | Crawl / Website-Durchlauf | A run that fetches and records discovered URLs; [engine](../../../crates/os-engine/src/lib.rs) |
| Crawl file | Crawl-Datei | SQLite `.ospider` database; [file format](architecture/crawl-file.md) |
| Frontier | Abruffront / Warteschlange | URLs awaiting crawl admission and dispatch; [frontier](../../../crates/os-engine/src/frontier.rs) |
| Disk-first | Datenträgerorientiert | Persist crawl facts rather than keeping the whole crawl in RAM; [principles](../../ARCHITECTURE.md) |
| Write batch | Schreibpaket | Group of crawl facts sent to persistence; [write model](../../../crates/os-model/src/write.rs) |
| WAL | Schreibvorausprotokoll | SQLite write-ahead log used by the storage profiles; [profile](../../../crates/os-store/src/profile.rs) |
| Checkpoint | Rückführung / Prüfpunkt | WAL transfer in storage, or cooperative pause/cancel checkpoint in operations; [checkpointer](../../../crates/os-store/src/checkpointer.rs), [cancel token](../../../crates/os-model/src/cancel.rs) |
| Resource guard | Ressourcenwächter | Memory/disk protection controlling crawl progress; [memory](../../../crates/os-engine/src/memory.rs), [disk guard](../../../crates/os-engine/src/disk_guard.rs) |
| Storage profile | Speicherprofil | Cache/synchronization settings; [profiles](../../../crates/os-store/src/profile.rs) |
| View session | Ansichtssitzung | Stateful view with selection/filter/order and generation; [sessions](../../../crates/os-analyze/src/views/session/mod.rs) |
| Change sequence | Änderungssequenz | Stored change feed position, distinct from view generation; [schema](../../../crates/os-store/migrations/0001_init.sql) |
| Read pool | Leseverbindungspool | Bounded blocking database read service; [pool](../../../crates/os-app/src/pool.rs) |
| Read lane | Lesespur | Workload separation through cached read slots; inspect [run_cached](../../../crates/os-app/src/pool.rs) for the current mechanism |
| DTO | Datenübertragungsobjekt | Rust/service data exposed to the frontend; [DTOs](../../../crates/os-app/src/dto) |
| IPC | Prozessübergreifende Kommunikation | Typed command/event boundary and transports; [client](../../../apps/desktop/src/ipc/client.ts) |
| Tauri bridge | Tauri-Brücke | Native host forwarding to application services; [host](../../../apps/desktop/src-tauri/src/lib.rs) |
| MCP | Modellkontextprotokoll | Agent tool interface with permission classes and transports; [server crate](../../../crates/os-mcp/src/lib.rs) |
| Fixture | Testeingabe / Testaufbau | Local spec-derived scenario/input; [fixture server](../../../crates/os-testsite/src/lib.rs) |
| Golden output | Sollausgabe | Checked-in expected output used in regression comparison; [testing rules](../../plan/M1.md) |
| Spec-ID | Anforderungskennung | Stable requirement identifier referenced by test tags; [spec index](../../spec/README.md) |
| Work package | Arbeitspaket | Assigned scope, owned paths and dependencies; [M2 package registry](../../plan/M2-packages.json) |
| Gate | Abnahmeprüfung | Integrated milestone/layer evidence check; [M2 acceptance](../../plan/M2.md) |
| Semantic token | Semantischer Gestaltungswert | Named UI design value shared across themes; [tokens](../../../apps/desktop/src/ui/tokens/tokens.ts) |
| Clean room | Unabhängige Implementierung | Work only from the permitted specification without inspecting reference implementation assets; [rules](../../../CLAUDE.md) |

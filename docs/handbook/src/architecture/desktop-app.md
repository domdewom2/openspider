# Desktop application

The desktop separates React presentation, Tauri native integration and Tauri-free Rust services. See the [workspace dependencies](../../../../apps/desktop/src-tauri/Cargo.toml), [host entry point](../../../../apps/desktop/src-tauri/src/lib.rs) and [os-app entry point](../../../../crates/os-app/src/lib.rs).

## Command and event path

1. A feature calls a domain wrapper in [src/ipc](../../../../apps/desktop/src/ipc).
2. The [IPC client](../../../../apps/desktop/src/ipc/client.ts) selects Tauri, E2E bridge or mock transport, maps errors, connects cancellation and rejects stale view responses.
3. [Tauri command plugins](../../../../apps/desktop/src-tauri/src/commands.rs) forward service commands to `App::dispatch`. The host also contains native-only commands; inspect [lib.rs](../../../../apps/desktop/src-tauri/src/lib.rs) for that boundary.
4. The [service registry](../../../../crates/os-app/src/registry.rs) and [command modules](../../../../crates/os-app/src/commands) connect domain logic. The [read pool](../../../../crates/os-app/src/pool.rs) runs blocking database work; the [write queue](../../../../crates/os-app/src/write_queue.rs) serializes app writes.
5. [Host events](../../../../apps/desktop/src-tauri/src/events.rs) connect to the service event sink and frontend subscriptions. View generation checks in the IPC client prevent older row responses replacing newer data.

DTOs live in [os-app/dto](../../../../crates/os-app/src/dto). [gen-ipc.mjs](../../../../apps/desktop/scripts/gen-ipc.mjs) maintains command wrappers and metadata; [generated TypeScript](../../../../apps/desktop/src/ipc/gen) carries Rust DTO types. Follow the generator and [static checks](../../../../apps/desktop/scripts/check-static.mjs) when changing the contract.

## Frontend map

| Directory / entry | Role and source |
|---|---|
| [main.tsx](../../../../apps/desktop/src/main.tsx), [App.tsx](../../../../apps/desktop/src/App.tsx) | Frontend startup and application composition |
| [app/](../../../../apps/desktop/src/app) | Application shell and window-level composition |
| [features/](../../../../apps/desktop/src/features) | Domain UI: crawl management, configuration, details, exports, comparisons and more |
| [grid/](../../../../apps/desktop/src/grid) | Table/grid presentation |
| [ipc/](../../../../apps/desktop/src/ipc) | Typed wrappers, transports, errors and events |
| [state/](../../../../apps/desktop/src/state), [data/](../../../../apps/desktop/src/data) | Frontend state and data adapters |
| [ui/](../../../../apps/desktop/src/ui) | Shared UI, semantic tokens, shortcuts and static checks |
| [i18n/](../../../../apps/desktop/src/i18n) | Localization runtime; messages also live under [ui/i18n](../../../../apps/desktop/src/ui/i18n) |

The [design concept](../../../plan/M2-ui-concept.md) defines intended interactions and visual rules. Inspect [tokens](../../../../apps/desktop/src/ui/tokens/tokens.ts), [static tests](../../../../apps/desktop/src/ui/static/static.test.ts) and [UI CSS](../../../../apps/desktop/src/ui/ui.css) before adding styles. Native file opening, asset URLs and window lifetime belong in the [Tauri host modules](../../../../apps/desktop/src-tauri/src/lib.rs); crawl and analysis logic belongs below the host.

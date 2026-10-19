//! Command-Domänen der Service-Schicht: je Domäne eine Datei `commands/<domäne>.rs` (Besitzer laut `docs/plan/M2.md`
//! 3.6). `register_all` ruft `register` jeder Domäne; Namen und Typen stehen im Katalog (`apps/desktop/src/ipc/commands.json`).
//!
//! Die Dateien sind von M2-F01 als Stubs angelegt (`STUB(M2-xx)`): der Besitzer ersetzt die Datei und behält Namen,
//! Argument-/Rückgabetypen und `long_running` bei.

/// Registriert Stub-Commands: `(name, args, ret, long_running)` mit TS-Typausdrücken für `args`/`ret`
/// (`void` = keine Argumente bzw. kein Ergebnis). Jeder antwortet mit `CmdError::unsupported(name, owner)`.
macro_rules! stub_commands {
    ($owner:literal, [ $( ($name:literal, $args:literal, $ret:literal, $long:literal) ),* $(,)? ]) => {
        /// Trägt die Commands dieser Domäne in die Registry ein.
        pub fn register(r: &mut $crate::registry::Registry) {
            $(
                r.register(
                    $crate::registry::CommandMeta {
                        name: $name,
                        args: $args,
                        ret: $ret,
                        long_running: $long,
                    },
                    {
                        fn stub<'a>(
                            _app: &'a $crate::runtime::App,
                            _ctx: $crate::runtime::CallCtx,
                            _args: ::serde_json::Value,
                        ) -> ::futures_util::future::BoxFuture<'a, $crate::registry::CmdResult> {
                            Box::pin(async {
                                Err($crate::dto::CmdError::unsupported($name, $owner))
                            })
                        }
                        stub
                    },
                );
            )*
        }
    };
}

pub mod analysis;
pub mod api;
pub mod app;
pub mod auth;
pub mod auth_login;
pub mod catalog;
pub mod compare;
pub mod config;
pub mod crawl;
pub mod crawl_add;
pub mod crawls;
pub mod detail;
pub mod detail_a11y;
pub mod detail_ext;
pub mod detail_spelling;
pub mod diag;
pub mod events;
pub mod export;
pub mod mail;
pub mod mcp;
pub mod op;
pub mod profile;
pub mod projects;
pub mod search;
pub mod segment;
pub mod settings;
pub mod sidebar;
pub mod sidebar_ext;
pub mod tasks;
pub mod uistate;
pub mod view;
pub mod viz;

use crate::registry::Registry;

/// Trägt alle Domänen in die Registry ein (Reihenfolge = Reihenfolge von `commands.json`).
pub fn register_all(r: &mut Registry) {
    app::register(r);
    app_quit::register(r);

    settings::register(r);
    settings_network::register(r);
    uistate::register(r);
    op::register(r);
    events::register(r);
    diag::register(r);
    crawl::register(r);
    analysis::register(r);
    crawl_add::register(r);
    catalog::register(r);
    view::register(r);
    search::register(r);
    detail::register(r);
    detail_ext::register(r);
    detail_spelling::register(r);
    detail_a11y::register(r);
    sidebar::register(r);
    sidebar_ext::register(r);
    config::register(r);
    config_robots::register(r);
    profile::register(r);
    segment::register(r);
    crawls::register(r);
    projects::register(r);
    export::register(r);
    export_multi::register(r);
    export_summary::register(r);
    compare::register(r);
    viz::register(r);
    mcp::register(r);
    api::register(r);
    tasks::register(r);
    auth::register(r);
    auth_login::register(r);
    mail::register(r);
}

pub mod app_quit;
pub mod config_robots;
pub mod export_multi;
pub mod export_summary;
pub mod settings_network;

/// Crawl-file desktop extension points (M2-U15, K2-61).
pub mod crawl_files;

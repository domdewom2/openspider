//! Laufzeit der Service-Schicht: `App`, Aufruf-Kontext und Dispatcher.
//!
//! STUB(M2-F03): Gerüst von M2-F01 mit den Signaturen aus `docs/plan/M2.md` 3.6. `dispatch` schlägt den Command in der
//! Registry nach und ruft den Handler; Lese-Pool, Schreibwarteschlange, Operationen, Ereigniskanäle und die
//! Einstellungen setzt M2-F03 um und ersetzt diese Datei.

use std::path::PathBuf;
use std::sync::Arc;

use os_analyze::analysis::CancelToken;

use crate::dto::{CmdError, ErrorCode};
use crate::events::EventSink;
use crate::ops::Ops;
use crate::pool::ReadPool;
use crate::quit::QuitDecision;
use crate::registry::{default_registry, CmdResult, Registry};
use crate::write_queue::WriteQueue;

/// Kennung eines Fensters (`main`, `settings`, `viz-<n>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WindowId(pub String);

/// Kontext eines Aufrufs: aufrufendes Fenster und Abbruchmarke.
#[derive(Debug, Clone)]
pub struct CallCtx {
    /// Fenster.
    pub window: WindowId,
    /// Abbruchmarke (`op.cancel`, SQLite-Interrupt).
    pub cancel: CancelToken,
}

/// Konfiguration beim Start. STUB(M2-F03): wird um Einstellungen, Uhr, ID-Quelle und `SecretStoreMut` erweitert.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Basisordner der App (`OPENSPIDER_HOME` bzw. Benutzerordner).
    pub home: PathBuf,
}

struct Inner {
    #[allow(dead_code)]
    config: AppConfig,
    registry: Registry,
    read_pool: ReadPool,
    write_queue: WriteQueue,
    ops: Ops,
}

/// Die Anwendung ohne Tauri: ein Exemplar je Prozess, geteilt von Tauri-Hülle, E2E-Bridge und HTTP-MCP.
#[derive(Clone)]
pub struct App(Arc<Inner>);

impl App {
    /// Erzeugt die Anwendung.
    pub fn new(config: AppConfig) -> Result<App, CmdError> {
        Ok(App(Arc::new(Inner {
            config,
            registry: default_registry(),
            read_pool: ReadPool,
            write_queue: WriteQueue,
            ops: Ops,
        })))
    }

    /// Führt den Command `name` mit `args` aus (Tauri, Bridge, HTTP-MCP).
    pub async fn dispatch(&self, ctx: CallCtx, name: &str, args: serde_json::Value) -> CmdResult {
        match self.0.registry.get(name) {
            Some((_, handler)) => handler(self, ctx, args).await,
            None => {
                Err(CmdError::new(ErrorCode::NotFound, "err-unknown-command")
                    .with_arg("command", name))
            }
        }
    }

    /// Das Register dieser Anwendung.
    pub fn registry(&self) -> &Registry {
        &self.0.registry
    }

    /// Abonniert die Ereignisse eines Fensters (ein Kanal je Fenster).
    pub fn subscribe(
        &self,
        _window: WindowId,
        _sink: Arc<dyn EventSink>,
    ) -> crate::dto::SubscriptionId {
        todo!("STUB(M2-F03): App::subscribe")
    }

    /// Meldet Sichtbarkeit eines Fensters (UI-143).
    pub fn set_visibility(&self, _window: WindowId, _visible: bool) {
        todo!("STUB(M2-F03): App::set_visibility")
    }

    /// Beenden anfragen (F-11).
    pub async fn request_quit(&self) -> QuitDecision {
        todo!("STUB(M2-F03): App::request_quit")
    }

    /// Lese-Pool für alle Datenbankzugriffe der App.
    pub fn read_pool(&self) -> &ReadPool {
        &self.0.read_pool
    }

    /// Serialisierte Schreibwarteschlange (K-20).
    pub fn write_queue(&self) -> &WriteQueue {
        &self.0.write_queue
    }

    /// Operationen mit Abbruch.
    pub fn ops(&self) -> &Ops {
        &self.0.ops
    }
}

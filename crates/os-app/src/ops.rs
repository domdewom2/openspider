//! Lang laufende Operationen mit Fortschritt und Abbruch (≤ 100 ms über SQLite-Interrupt).
//!
//! STUB(M2-F03): Signatur aus `docs/plan/M2.md` 3.6; M2-F03 setzt `Ops` um.

use futures_util::future::BoxFuture;
use os_analyze::analysis::CancelToken;

use crate::dto::{CmdError, OpHandle};

/// Meldet Fortschritt einer Operation (erledigt, gesamt).
#[derive(Debug, Clone, Default)]
pub struct ProgressSink;

/// Verwaltung der laufenden Operationen.
#[derive(Debug, Default)]
pub struct Ops;

impl Ops {
    /// Startet `f` im Hintergrund und gibt sofort das `OpHandle` zurück.
    pub fn spawn<F>(&self, _label_key: &'static str, _cancellable: bool, _f: F) -> OpHandle
    where
        F: FnOnce(
                CancelToken,
                ProgressSink,
            ) -> BoxFuture<'static, Result<serde_json::Value, CmdError>>
            + Send
            + 'static,
    {
        todo!("STUB(M2-F03): Ops::spawn")
    }

    /// Bricht die Operation ab; `false`, wenn es sie nicht (mehr) gibt.
    pub fn cancel(&self, _op_id: &str) -> bool {
        false
    }
}

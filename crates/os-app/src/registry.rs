//! Command-Register: Name → Metadaten + Handler (`docs/plan/M2.md` 3.6, Verträge).
//!
//! Die Registry ist die einzige Quelle für `apps/desktop/src/ipc/commands.json` (Binary `os-app-manifest`); ein
//! Vertragstest vergleicht Registry und Datei, ein zweiter die TypeScript-Wrapper. Tauri, die E2E-Bridge und der
//! HTTP-MCP-Transport rufen Commands ausschließlich über `App::dispatch`.

use std::collections::HashMap;

use futures_util::future::BoxFuture;
use serde::Serialize;

use crate::dto::CmdError;
use crate::runtime::{App, CallCtx};

/// Ergebnis eines Commands: JSON-Wert oder Fehler im Format von `M2-ui-concept.md` 4.2.
pub type CmdResult = Result<serde_json::Value, CmdError>;

/// Handler eines Commands: Anwendung, Aufruf-Kontext (Fenster, Abbruchmarke), Argumente als JSON.
pub type Handler = for<'a> fn(&'a App, CallCtx, serde_json::Value) -> BoxFuture<'a, CmdResult>;

/// Metadaten eines Commands (Zeile von `commands.json`).
///
/// `args` und `ret` sind TypeScript-Typausdrücke über den Typen aus `apps/desktop/src/ipc/gen/` (`void` = keine
/// Argumente bzw. kein Ergebnis, `Typ[]`, `Typ | null`, `string`, `number`). Bei `long_running` liefert der Command
/// sofort einen `OpHandle`; `ret` bezeichnet dann das Ergebnis der Operation (`Tick.ops[].result`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandMeta {
    /// Name `<domäne>.<command>`, z. B. `view.rows`.
    pub name: &'static str,
    /// Typausdruck der Argumente.
    pub args: &'static str,
    /// Typausdruck des Ergebnisses.
    pub ret: &'static str,
    /// Liefert sofort einen `OpHandle`.
    pub long_running: bool,
}

impl CommandMeta {
    /// Domäne (Teil vor dem Punkt).
    pub fn domain(&self) -> &'static str {
        self.name.split_once('.').map_or(self.name, |(d, _)| d)
    }
}

/// Das Register aller Commands in Registrierungsreihenfolge.
#[derive(Default)]
pub struct Registry {
    entries: Vec<(CommandMeta, Handler)>,
    index: HashMap<&'static str, usize>,
}

impl Registry {
    /// Leeres Register.
    pub fn new() -> Registry {
        Registry::default()
    }

    /// Trägt einen Command ein. Ein doppelter Name ist ein Programmierfehler und bricht ab.
    pub fn register(&mut self, meta: CommandMeta, h: Handler) {
        assert!(
            self.index.insert(meta.name, self.entries.len()).is_none(),
            "Command {} ist doppelt registriert",
            meta.name
        );
        self.entries.push((meta, h));
    }

    /// Metadaten aller Commands in Registrierungsreihenfolge.
    pub fn manifest(&self) -> Vec<CommandMeta> {
        self.entries.iter().map(|(m, _)| *m).collect()
    }

    /// Metadaten und Handler zu einem Namen.
    pub fn get(&self, name: &str) -> Option<&(CommandMeta, Handler)> {
        self.index.get(name).map(|&i| &self.entries[i])
    }

    /// Anzahl der Commands.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Ohne Commands?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Ruft `commands::<domäne>::register` für jede Domäne auf.
pub fn register_all(r: &mut Registry) {
    crate::commands::register_all(r);
}

/// Das vollständige Register aller Domänen.
pub fn default_registry() -> Registry {
    let mut r = Registry::new();
    register_all(&mut r);
    r
}

/// Das Manifest als JSON in der Form von `apps/desktop/src/ipc/commands.json`: ein Array von
/// `{ "name", "args", "ret", "longRunning" }`, eingerückt mit zwei Leerzeichen, abschließender Zeilenumbruch.
pub fn manifest_json(r: &Registry) -> String {
    let mut s = serde_json::to_string_pretty(&r.manifest()).unwrap_or_default();
    s.push('\n');
    s
}

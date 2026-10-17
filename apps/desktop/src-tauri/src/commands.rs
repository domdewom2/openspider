//! Command-Plugins: je Domäne ein internes Tauri-Plugin `plugin:<domäne>|<command>` (M2-ui-concept 4.1).
//!
//! Die Plugins entstehen mechanisch aus dem Register von `os-app` (Domäne = Teil des Namens vor dem Punkt, derselbe
//! Katalog wie `commands.json`): jeder Aufruf geht unverändert an `App::dispatch`. Neue Commands späterer Pakete
//! brauchen nur den Registry-Eintrag in `os-app`; neue Domänen zusätzlich den Eintrag in
//! `capabilities/default.json` (der Test `tests/capabilities.rs` erinnert daran).
//!
//! Aufrufkonvention: das Argument ist das JSON-Objekt der Argumente des Commands (`void` = leer), das Ergebnis der
//! JSON-Wert, ein Fehler das `CmdError`-Objekt. `events.subscribe` bekommt zusätzlich `channel` (Tauri-`Channel`) und
//! läuft über [`crate::events`].

use os_app::{App, CallCtx};
use tauri::ipc::{Invoke, InvokeBody, InvokeError};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

use crate::plugin_names::plugin_name;

/// Alle Domänen des Registers in Registrierungsreihenfolge.
pub fn domains(app: &App) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for meta in app.registry().manifest() {
        let d = meta.domain();
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

/// Ein Plugin je Domäne.
pub fn plugins<R: Runtime>(app: &App) -> Vec<TauriPlugin<R>> {
    domains(app)
        .into_iter()
        .map(|d| domain_plugin::<R>(d, app.clone()))
        .collect()
}

fn domain_plugin<R: Runtime>(domain: &'static str, app: App) -> TauriPlugin<R> {
    // Namen sind klein und leben so lange wie der Prozess
    let name: &'static str = Box::leak(plugin_name(domain).into_boxed_str());
    Builder::new(name)
        .invoke_handler(move |invoke: Invoke<R>| handle(&app, domain, invoke))
        .build()
}

fn handle<R: Runtime>(app: &App, domain: &str, invoke: Invoke<R>) -> bool {
    let name = format!("{domain}.{}", invoke.message.command());
    if app.registry().get(&name).is_none() {
        return false;
    }
    let Invoke {
        message, resolver, ..
    } = invoke;
    let args = match message.payload() {
        InvokeBody::Json(v) => v.clone(),
        InvokeBody::Raw(_) => serde_json::Value::Null,
    };
    if name == "events.subscribe" {
        crate::events::subscribe(app, &message, resolver, &args);
        return true;
    }
    let ctx = CallCtx::for_window(message.webview_ref().label());
    let app = app.clone();
    resolver.respond_async(async move {
        app.dispatch(ctx, &name, args)
            .await
            .map_err(InvokeError::from)
    });
    true
}

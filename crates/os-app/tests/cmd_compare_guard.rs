//! Ressourcenschutz im Vergleich (CRAWL-129, K2-56; M2-C01): Meldet der Ressourcenwächter Stufe 2, pausiert die
//! Vergleichsberechnung von `compare.open`, statt abzubrechen, und läuft nach Speicherfreigabe weiter. Eigenes Testziel,
//! weil die Messsonde des Wächters prozessweit gilt. Fixture: `out-m2-compare/v1`, `v2`.
// Testcode legt Crawl-Dateien an und liest sie: die Ausnahme von `clippy.toml` gilt hier, nicht in Quellcode.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

#[path = "../../os-analyze/tests/compare_common.rs"]
mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::*;
use os_app::dto::*;
use os_app::{App, AppConfig, CallCtx};
use os_engine::memory::{set_probe, Sample};
use serde_json::json;

// spec: CRAWL-129
#[tokio::test(flavor = "multi_thread")]
async fn comparison_pauses_at_stage_two_and_resumes_after_release() {
    let (old, new) = (v1().await, v2().await);
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    // Injizierte Grenze (K2-56): 950 von 1 000 Byte = 95 % → Stufe 2.
    let low = Arc::new(AtomicBool::new(true));
    let flag = low.clone();
    set_probe(Some(Arc::new(move || Sample {
        engine_bytes: if flag.load(Ordering::SeqCst) {
            950
        } else {
            100
        },
        injected_limit: Some(1_000),
        ..Sample::default()
    })));
    let args = json!({"current": new.file.path(), "previous": old.file.path(), "mapping": [], "elements": []});
    let pending = tokio::spawn({
        let app = app.clone();
        async move {
            app.dispatch(CallCtx::for_window("main"), "compare.open", args)
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert!(
        !pending.is_finished(),
        "Vergleich muss pausieren, nicht abbrechen"
    );
    low.store(false, Ordering::SeqCst);
    let result = tokio::time::timeout(Duration::from_secs(30), pending)
        .await
        .expect("läuft nach Freigabe weiter")
        .unwrap();
    set_probe(None);
    let session: CompareSession = serde_json::from_value(result.unwrap()).unwrap();
    assert_eq!(session.current.file, new.file.path().to_string_lossy());

    // Abbruch während der Pause liefert „abgebrochen“, kein Absturz.
    low.store(true, Ordering::SeqCst);
    let flag = low.clone();
    set_probe(Some(Arc::new(move || Sample {
        engine_bytes: if flag.load(Ordering::SeqCst) {
            950
        } else {
            100
        },
        injected_limit: Some(1_000),
        ..Sample::default()
    })));
    let ctx = CallCtx::for_window("main");
    let cancel = ctx.cancel.clone();
    let args = json!({"current": new.file.path(), "previous": old.file.path(), "mapping": [], "elements": []});
    let pending = tokio::spawn({
        let app = app.clone();
        async move { app.dispatch(ctx, "compare.open", args).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    cancel.cancel();
    let err = tokio::time::timeout(Duration::from_secs(10), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    set_probe(None);
    assert_eq!(err.code, ErrorCode::Cancelled);
}

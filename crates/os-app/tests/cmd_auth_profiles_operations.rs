use os_app::{App, AppConfig, CallCtx};
use os_fetch::secrets::{MemorySecretStore, SecretStore};
use serde_json::{json, Value};
use std::sync::Arc;
async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap()
}
fn profile(name: &str, kind: &str) -> Value {
    json!({"profile":{"name":name,"kind":kind,"host":"fixture.test","username":"fixture-user","secret":"PASSWORD-FIXTURE; session=COOKIE-FIXTURE"}})
}
// spec: OUT-223
// spec: CRAWL-121
#[tokio::test]
async fn crud_export_import_roundtrip_and_metadata_only_replies() {
    let home = tempfile::tempdir().unwrap();
    let keys = Arc::new(MemorySecretStore::new());
    let app = App::new(AppConfig::new(home.path()).with_secrets(keys.clone())).unwrap();
    for kind in ["form", "basic", "digest", "ntlm"] {
        let reply = call(&app, "auth.saveProfile", profile(kind, kind)).await;
        assert_eq!(reply["kind"], kind);
        assert_eq!(reply["hasSecret"], true);
        assert!(!reply.to_string().contains("FIXTURE"));
    }
    let list = call(&app, "auth.listProfiles", Value::Null).await;
    assert_eq!(list.as_array().unwrap().len(), 4);
    assert_eq!(list[0]["name"], "basic");
    for e in std::fs::read_dir(home.path().join("auth")).unwrap() {
        let bytes = std::fs::read(e.unwrap().path()).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("FIXTURE"));
    }
    let dest = home.path().join("portable.osauth");
    call(
        &app,
        "auth.exportProfile",
        json!({"name":"form","path":dest,"passphrase":"fixture-passphrase","bindToMachine":false}),
    )
    .await;
    let other = tempfile::tempdir().unwrap();
    let other_keys = Arc::new(MemorySecretStore::new());
    let other_app =
        App::new(AppConfig::new(other.path()).with_secrets(other_keys.clone())).unwrap();
    assert!(other_app
        .dispatch(
            CallCtx::for_window("main"),
            "auth.importProfile",
            json!({"path":dest,"passphrase":"wrong","bindToMachine":true})
        )
        .await
        .is_err());
    let reply = call(
        &other_app,
        "auth.importProfile",
        json!({"path":dest,"passphrase":"fixture-passphrase","bindToMachine":true}),
    )
    .await;
    assert_eq!(reply["name"], "form");
    let machine_file = std::fs::read(
        std::fs::read_dir(other.path().join("auth"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path(),
    )
    .unwrap();
    let imported =
        os_store::auth_profiles::open(&machine_file, None, |id| other_keys.get(id)).unwrap();
    assert_eq!(
        imported["profile"]["secret"],
        "PASSWORD-FIXTURE; session=COOKIE-FIXTURE"
    );
    call(&other_app, "auth.deleteProfile", json!({"name":"form"})).await;
    assert!(other_keys.get("auth/form").is_none());
    assert_eq!(
        call(&other_app, "auth.listProfiles", Value::Null).await,
        json!([])
    );
}
// spec: OUT-223
#[tokio::test]
async fn updates_keep_secrets_and_machine_exports_fail_on_another_machine() {
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    let first = call(&app, "auth.saveProfile", profile("../../escape", "basic")).await;
    let reply = call(&app,"auth.saveProfile",json!({"profile":{"name":"../../escape","kind":"basic","host":"fixture.test","username":"changed","secret":null}})).await;
    assert_eq!(reply["createdAt"], first["createdAt"]);
    assert_eq!(reply["hasSecret"], true);
    let dest = home.path().join("machine.osauth");
    call(
        &app,
        "auth.exportProfile",
        json!({"name":"../../escape","path":dest,"bindToMachine":true}),
    )
    .await;
    let other = tempfile::tempdir().unwrap();
    let other_app = App::new(AppConfig::new(other.path())).unwrap();
    let err = other_app
        .dispatch(
            CallCtx::for_window("main"),
            "auth.importProfile",
            json!({"path":dest,"bindToMachine":true}),
        )
        .await
        .unwrap_err();
    assert!(err.detail.unwrap().contains("Rechner"));
    assert!(app
        .dispatch(
            CallCtx::for_window("main"),
            "auth.saveProfile",
            profile("", "basic")
        )
        .await
        .is_err());
    assert!(!home.path().join("escape.osauth").exists());
}

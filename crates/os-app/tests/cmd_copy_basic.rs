#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use serde_json::json;
// spec: UI-012
#[tokio::test]
async fn copy_resolves_nonloaded_rows_exceptions_columns_and_platform_tsv() {
    let fx = Fx::new();
    let h = fx.open(tab(), json!(["Address", "Status Code"])).await;
    let result=fx.call("view.copy",json!({"viewId":h["viewId"],"selection":{"kind":"all","viewId":h["viewId"],"version":42,"except":[1,2,3,4,5]},"columns":["Status Code","Address"],"withHeader":true})).await;
    let eol = if cfg!(windows) { "\r\n" } else { "\n" };
    let text = format!("Status Code\tAddress{eol}200\thttps://site.test/noindex{eol}");
    assert_eq!(*fx.clipboard.0.lock().unwrap(), text);
    assert_eq!(result["rows"], 1);
    assert_eq!(result["bytes"], text.len());
    assert_eq!(
        os_app::clipboard::tsv_row(&[json!("a\tb\nc\r"), json!(null), json!(1.25)], false),
        "a b c \t\t1.25\n"
    );
    assert_eq!(os_app::clipboard::tsv_row(&[json!(true)], true), "true\r\n");
}
// spec: UI-012
#[tokio::test]
async fn copy_rejects_more_than_one_million_cells_without_touching_clipboard() {
    let fx = Fx::new();
    fx.write("WITH RECURSIVE n(x) AS (SELECT 10 UNION ALL SELECT x+1 FROM n WHERE x<20010) INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length) SELECT x,'https://site.test/'||x,'https://site.test/'||x,'https://site.test',1,1,'done','link',0,25 FROM n");
    let h = fx.open(tab(), json!([])).await;
    *fx.clipboard.0.lock().unwrap() = "unchanged".into();
    let e=fx.app.dispatch(os_app::CallCtx::for_window("main"),"view.copy",json!({"viewId":h["viewId"],"selection":{"kind":"all","viewId":h["viewId"],"version":42,"except":[]},"columns":[],"withHeader":false})).await.unwrap_err();
    assert_eq!(e.message_key, "err-copy-too-large");
    assert_eq!(*fx.clipboard.0.lock().unwrap(), "unchanged");
}

//! Tests der Redirect-Reports (OUT-081, OUT-082) gegen die Fixture `out-redirects`.
//!
//! Start der Crawls ist `http://site.test/` mit `scope.all_subdomains = true` (der Host-Wechsel auf
//! `www.site.test` bleibt dann intern). Die Erwartungen stammen aus den Akzeptanzkriterien der Spec, nicht
//! aus der Ausgabe.

#[path = "reports_common.rs"]
mod common;

use common::{base_config, Table};
use os_engine::testing::{crawl_fixture_from, FixtureCrawl};
use os_export::{save_report, Format};

const BASE: &str = "http://site.test";

async fn crawl(
    max_redirects: Option<u32>,
    exclude: &[&str],
) -> (FixtureCrawl, os_model::CrawlConfig) {
    let mut cfg = base_config();
    cfg.scope.all_subdomains = true;
    if let Some(n) = max_redirects {
        cfg.limits.max_redirects = n;
    }
    cfg.scope.exclude = exclude.iter().map(|s| (*s).to_string()).collect();
    let fx = crawl_fixture_from("out-redirects", "http://site.test/", cfg.clone()).await;
    (fx, cfg)
}

fn report(
    fx: &FixtureCrawl,
    cfg: &os_model::CrawlConfig,
    name: &str,
    format: Format,
) -> (tempfile::TempDir, os_export::ReportResult) {
    let tmp = tempfile::tempdir().unwrap();
    let result = save_report(&fx.file, cfg, name, tmp.path(), format).unwrap();
    (tmp, result)
}

fn load(fx: &FixtureCrawl, cfg: &os_model::CrawlConfig, name: &str) -> (tempfile::TempDir, Table) {
    let (tmp, result) = report(fx, cfg, name, Format::Csv);
    let table = Table::read(&result.files[0].path);
    (tmp, table)
}

/// Wert der Spalte `<field> <n>` einer Zeile.
fn hop<'a>(t: &Table, row: &'a [String], field: &str, n: usize) -> &'a str {
    t.get(row, &format!("{field} {n}"))
}

fn addr(path: &str) -> String {
    format!("{BASE}{path}")
}

// spec: OUT-081
#[tokio::test]
async fn all_redirects_header_and_hop_columns_follow_the_longest_chain() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    let fixed = [
        "Chain Type",
        "Number of Redirects",
        "Redirect Loop",
        "Temp Redirect in Chain",
        "Address",
        "Final Address",
        "Final Status Code",
        "Final Status",
        "Final Content Type",
        "Final Indexability",
        "Final Indexability Status",
        "Inlinks",
        "Sample Source",
    ];
    // Die längste Kette hat 6 Weiterleitungen (/long1 … /long7): 13 feste Spalten + 6 × 4 Sprungspalten.
    assert_eq!(&t.header[..13], fixed);
    assert_eq!(t.header.len(), 13 + 4 * 6);
    assert_eq!(
        &t.header[13..17],
        [
            "Redirect Type 1",
            "Redirect URL 1",
            "Status Code 1",
            "Status 1"
        ]
    );
    assert_eq!(t.header.last().map(String::as_str), Some("Status 6"));
}

// spec: OUT-081
#[tokio::test]
async fn simple_301_is_one_row_with_one_hop() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    let r = t.row("Address", "/a");
    assert_eq!(t.get(r, "Address"), addr("/a"));
    assert_eq!(t.get(r, "Number of Redirects"), "1");
    assert_eq!(t.get(r, "Redirect Loop"), "False");
    assert_eq!(t.get(r, "Temp Redirect in Chain"), "False");
    assert_eq!(t.get(r, "Chain Type"), "HTTP Redirect");
    assert_eq!(t.get(r, "Final Address"), addr("/b"));
    assert_eq!(t.get(r, "Final Status Code"), "200");
    assert_eq!(t.get(r, "Final Status"), "OK");
    assert_eq!(t.get(r, "Final Content Type"), "text/html; charset=utf-8");
    assert_eq!(t.get(r, "Final Indexability"), "Indexable");
    assert_eq!(t.get(r, "Final Indexability Status"), "");
    assert_eq!(hop(&t, r, "Redirect Type", 1), "HTTP Redirect");
    assert_eq!(hop(&t, r, "Redirect URL", 1), addr("/b"));
    assert_eq!(hop(&t, r, "Status Code", 1), "301");
    assert_eq!(hop(&t, r, "Status", 1), "Moved Permanently");
    // Hop 2 gibt es für diese Kette nicht: leere Zellen.
    assert_eq!(hop(&t, r, "Redirect URL", 2), "");
    // Inlinks und Beispielquelle: die Startseite verlinkt /a genau einmal.
    assert_eq!(t.get(r, "Inlinks"), "1");
    assert_eq!(t.get(r, "Sample Source"), format!("{BASE}/"));
}

// spec: OUT-081
#[tokio::test]
async fn temp_redirect_rule_covers_302_303_307_and_delayed_refresh_but_not_301_308() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    // /c 302 → /d 301 → /e 200: zwei Weiterleitungen, davon eine temporäre.
    let c = t.row("Address", "/c");
    assert_eq!(t.get(c, "Number of Redirects"), "2");
    assert_eq!(t.get(c, "Temp Redirect in Chain"), "True");
    assert_eq!(t.get(c, "Final Address"), addr("/e"));
    assert_eq!(hop(&t, c, "Status Code", 1), "302");
    assert_eq!(hop(&t, c, "Status Code", 2), "301");
    for path in ["/t303", "/t307"] {
        assert_eq!(
            t.get(t.row("Address", path), "Temp Redirect in Chain"),
            "True",
            "{path}"
        );
    }
    assert_eq!(
        t.get(t.row("Address", "/p308"), "Temp Redirect in Chain"),
        "False"
    );
    assert_eq!(
        t.get(t.row("Address", "/a"), "Temp Redirect in Chain"),
        "False"
    );
    // Meta Refresh mit Verzögerung > 0 ist temporär, ohne Verzögerung nicht.
    assert_eq!(
        t.get(t.row("Address", "/meta-delay"), "Temp Redirect in Chain"),
        "True"
    );
    assert_eq!(
        t.get(t.row("Address", "/meta"), "Temp Redirect in Chain"),
        "False"
    );
}

// spec: OUT-081
#[tokio::test]
async fn refresh_redirects_use_the_page_status_and_their_own_labels() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    let hr = t.row("Address", "/hr");
    assert_eq!(t.get(hr, "Chain Type"), "HTTP Refresh");
    assert_eq!(hop(&t, hr, "Redirect Type", 1), "HTTP Refresh");
    assert_eq!(hop(&t, hr, "Status Code", 1), "200");
    assert_eq!(hop(&t, hr, "Redirect URL", 1), addr("/hr2"));
    let meta = t.row("Address", "/meta");
    assert_eq!(t.get(meta, "Chain Type"), "Meta Refresh Redirect");
    assert_eq!(hop(&t, meta, "Redirect Type", 1), "Meta Refresh Redirect");
    assert_eq!(hop(&t, meta, "Status Code", 1), "200");
    assert_eq!(t.get(meta, "Final Address"), addr("/meta2"));
}

// spec: OUT-081
#[tokio::test]
async fn mixed_chain_301_then_meta_refresh() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    let r = t.row("Address", "/mix");
    assert_eq!(t.get(r, "Chain Type"), "Mixed");
    assert_eq!(t.get(r, "Number of Redirects"), "2");
    assert_eq!(hop(&t, r, "Redirect Type", 1), "HTTP Redirect");
    assert_eq!(hop(&t, r, "Redirect Type", 2), "Meta Refresh Redirect");
    assert_eq!(t.get(r, "Final Address"), addr("/mixend"));
    // Die Zwischenstation /mixmeta ist nur über die Kette erreichbar: keine eigene Zeile.
    assert!(t.find("Address", "/mixmeta").is_none());
}

// spec: OUT-081
#[tokio::test]
async fn five_hop_chain_and_chain_across_hosts() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    let l1 = t.row("Address", "/l1");
    assert_eq!(t.get(l1, "Number of Redirects"), "5");
    assert_eq!(t.get(l1, "Final Address"), addr("/l6"));
    assert_eq!(hop(&t, l1, "Redirect URL", 5), addr("/l6"));
    // Host-Wechsel site.test → www.site.test.
    let hw = t.row("Address", "/hw1");
    assert_eq!(t.get(hw, "Number of Redirects"), "2");
    assert_eq!(hop(&t, hw, "Redirect URL", 1), "http://www.site.test/hw2");
    assert_eq!(t.get(hw, "Final Address"), "http://www.site.test/hw3");
}

// spec: OUT-081
#[tokio::test]
async fn chain_to_404_reports_the_final_error() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    let m = t.row("Address", "/m");
    assert_eq!(t.get(m, "Final Address"), addr("/n"));
    assert_eq!(t.get(m, "Final Status Code"), "404");
    assert_eq!(t.get(m, "Final Status"), "Not Found");
    assert_eq!(t.get(m, "Final Indexability"), "Non-Indexable");
    assert_eq!(t.get(m, "Final Indexability Status"), "Client Error");
}

// spec: OUT-081
#[tokio::test]
async fn redirect_loops_have_no_final_address() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    // /f → /g → /f
    let f = t.row("Address", "/f");
    assert_eq!(t.get(f, "Redirect Loop"), "True");
    assert_eq!(t.get(f, "Number of Redirects"), "2");
    assert_eq!(t.get(f, "Final Address"), "");
    assert_eq!(t.get(f, "Final Status"), "Redirect Loop");
    assert_eq!(hop(&t, f, "Redirect URL", 1), addr("/g"));
    assert_eq!(hop(&t, f, "Redirect URL", 2), addr("/f"));
    // /g wurde nur über die Kette entdeckt und hat keine eigene Zeile.
    assert!(t.find("Address", "/g").is_none());
    // Selbst-Weiterleitung /s → /s
    let s = t.row("Address", "/s");
    assert_eq!(t.get(s, "Redirect Loop"), "True");
    assert_eq!(t.get(s, "Number of Redirects"), "1");
    assert_eq!(t.get(s, "Final Status"), "Redirect Loop");
    // Ohne Schleife ist die Spalte False.
    assert_eq!(t.get(t.row("Address", "/a"), "Redirect Loop"), "False");
}

// spec: OUT-081
#[tokio::test]
async fn intermediate_hops_get_a_row_only_when_directly_linked() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    // /d liegt nur in der Kette /c → /d → /e: keine eigene Zeile.
    assert!(t.find("Address", "/d").is_none());
    // /x2 ist zusätzlich von der Startseite verlinkt: eigene Zeile mit einem Sprung.
    let x2 = t.row("Address", "/x2");
    assert_eq!(t.get(x2, "Number of Redirects"), "1");
    assert_eq!(t.get(x2, "Final Address"), addr("/x3"));
    // /x1 behält seine Kette über /x2.
    assert_eq!(t.get(t.row("Address", "/x1"), "Number of Redirects"), "2");
    // Weiterleitungsziele ohne eigene Weiterleitung sind nie Start einer Zeile.
    for path in ["/b", "/e", "/l6", "/n", "/long7"] {
        assert!(t.find("Address", path).is_none(), "{path}");
    }
}

// spec: OUT-081
#[tokio::test]
async fn rows_follow_the_crawl_order() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    let addresses = t.column("Address");
    let position = |p: &str| addresses.iter().position(|a| a.ends_with(p)).unwrap();
    // Die Startseite verlinkt in dieser Reihenfolge; die Ausgabe folgt der Entdeckungsreihenfolge.
    assert!(position("/a") < position("/c"));
    assert!(position("/c") < position("/l1"));
    assert!(position("/l1") < position("/f"));
    assert!(position("/f") < position("/long1"));
}

// spec: OUT-082
#[tokio::test]
async fn redirect_chains_report_has_only_chains_and_loops() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:Redirect Chains");
    let addresses: Vec<&str> = t.column("Address");
    let has = |p: &str| addresses.iter().any(|a| a.ends_with(p));
    // Ein Sprung fehlt (/a, /x2, /t307, /meta, /m).
    for path in ["/a", "/x2", "/t307", "/meta", "/p308", "/m", "/hr"] {
        assert!(!has(path), "{path} darf nicht im Chains-Report stehen");
    }
    // Zwei und mehr Sprünge sowie die Schleifen sind enthalten.
    for path in ["/c", "/l1", "/mix", "/hw1", "/x1", "/long1", "/f", "/s"] {
        assert!(has(path), "{path} muss im Chains-Report stehen");
    }
    // Spalten wie All Redirects; die Zahl der Sprungspalten folgt der längsten ausgegebenen Kette.
    assert_eq!(t.header.len(), 13 + 4 * 6);
    // Filterbar über die Spalte Redirect Loop.
    let loops: Vec<&str> = t
        .rows
        .iter()
        .filter(|r| t.get(r, "Redirect Loop") == "True")
        .map(|r| t.get(r, "Address"))
        .collect();
    assert_eq!(loops, vec![addr("/f"), addr("/s")]);
    // Jede Zeile außer Schleifen hat mindestens zwei Weiterleitungen.
    for r in &t.rows {
        let n: usize = t.get(r, "Number of Redirects").parse().unwrap();
        assert!(n >= 2 || t.get(r, "Redirect Loop") == "True");
    }
}

// spec: OUT-082
#[tokio::test]
async fn chains_report_columns_shrink_to_the_longest_selected_chain() {
    // Mit Weiterleitungslimit 3 endet die längste Kette nach 3 Sprüngen: nur 3 Sprungspalten.
    let (fx, cfg) = crawl(Some(3), &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:Redirect Chains");
    assert_eq!(t.header.len(), 13 + 4 * 3);
}

// spec: OUT-081
#[tokio::test]
async fn max_redirects_reached_ends_the_chain_at_the_last_fetched_hop() {
    let (fx, cfg) = crawl(Some(3), &[]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    for start in ["/l1", "/long1"] {
        let r = t.row("Address", start);
        assert_eq!(t.get(r, "Final Status"), "Max Redirects Reached", "{start}");
        assert_eq!(t.get(r, "Number of Redirects"), "3", "{start}");
        assert_eq!(t.get(r, "Redirect Loop"), "False");
        // Das Endglied ist das letzte abgerufene (noch weiterleitende) Glied.
        assert_eq!(t.get(r, "Final Status Code"), "301");
    }
    assert_eq!(
        t.get(t.row("Address", "/long1"), "Final Address"),
        addr("/long4")
    );
    // Ketten innerhalb des Limits bleiben unberührt.
    let c = t.row("Address", "/c");
    assert_eq!(t.get(c, "Final Status"), "OK");
}

// spec: OUT-081
#[tokio::test]
async fn target_that_was_not_fetched_is_reported_as_not_followed() {
    // Das Ziel von /a ist per Exclude ausgeschlossen und wird nicht angefragt.
    let (fx, cfg) = crawl(None, &[".*/b$"]).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    let a = t.row("Address", "/a");
    assert_eq!(t.get(a, "Final Status"), "Not Followed");
    assert_eq!(t.get(a, "Final Status Code"), "");
    assert_eq!(t.get(a, "Number of Redirects"), "1");
    assert_eq!(hop(&t, a, "Redirect URL", 1), addr("/b"));
}

// spec: OUT-081, OUT-082
#[tokio::test]
async fn files_are_named_after_the_report_and_xlsx_holds_the_same_table() {
    let (fx, cfg) = crawl(None, &[]).await;
    let (_tmp, result) = report(&fx, &cfg, "Redirects:Redirect Chains", Format::Csv);
    assert_eq!(result.files.len(), 1);
    assert_eq!(
        result.files[0].path.file_name().unwrap(),
        "redirect_chains.csv"
    );
    let (_t2, all) = report(&fx, &cfg, "redirects : all redirects", Format::Csv);
    assert_eq!(all.files[0].path.file_name().unwrap(), "all_redirects.csv");

    use calamine::{open_workbook, Reader, Xlsx};
    let (_t3, x) = report(&fx, &cfg, "Redirects:All Redirects", Format::Xlsx);
    assert_eq!(x.files[0].path.file_name().unwrap(), "all_redirects.xlsx");
    let csv = Table::read(&all.files[0].path);
    let mut wb: Xlsx<_> = open_workbook(&x.files[0].path).unwrap();
    let range = wb.worksheet_range("All Redirects").unwrap();
    assert_eq!(range.height(), csv.rows.len() + 1);
    // Zahlenspalten sind echte Zahlen, True/False sind Text.
    let n_col = csv.col("Number of Redirects");
    assert!(matches!(
        range.get((1, n_col)).unwrap(),
        calamine::Data::Float(_) | calamine::Data::Int(_)
    ));
    let loop_col = csv.col("Redirect Loop");
    assert!(matches!(
        range.get((1, loop_col)).unwrap(),
        calamine::Data::String(s) if s == "False" || s == "True"
    ));
}

// spec: OUT-081 [M2]
// spec: CRAWL-018 [M2]
#[tokio::test]
async fn hsts_policy_hop_is_307_hsts_policy() {
    let (fx, cfg) = crawl(None, &[]).await;
    fx.file.conn().execute("UPDATE urls SET status_code = 307, status = 'HSTS Policy', redirect_type = 'hsts_policy' WHERE address = 'http://site.test/a'", []).unwrap();
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    let r = t.row("Address", "/a");
    assert_eq!(hop(&t, r, "Redirect Type", 1), "HSTS Policy");
    assert_eq!(hop(&t, r, "Status Code", 1), "307");
    assert_eq!(hop(&t, r, "Status", 1), "HSTS Policy");
    assert_eq!(t.get(r, "Temp Redirect in Chain"), "False");
}

// spec: OUT-081 [M2]
#[test]
#[ignore = "M2: ONPAGE-057 JavaScript-Weiterleitungen gibt es nur mit Rendering"]
fn javascript_redirect_hop_needs_rendering() {}

// spec: OUT-112
#[tokio::test]
async fn redirect_reports_have_no_prerequisites_to_warn_about() {
    let (fx, cfg) = crawl(None, &[]).await;
    for name in ["Redirects:All Redirects", "Redirects:Redirect Chains"] {
        let (_t, r) = report(&fx, &cfg, name, Format::Csv);
        assert!(r.warnings.is_empty(), "{name}: {:?}", r.warnings);
    }
}

// spec: OUT-081, OUT-082
#[tokio::test]
async fn reports_match_the_regression_goldens() {
    use os_testsite::golden::{assert_golden, normalize};
    let (fx, cfg) = crawl(None, &[]).await;
    let golden =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/out-redirects");
    for (name, file) in [
        ("Redirects:All Redirects", "all_redirects.csv"),
        ("Redirects:Redirect Chains", "redirect_chains.csv"),
    ] {
        let (_tmp, result) = report(&fx, &cfg, name, Format::Csv);
        let text = std::fs::read_to_string(&result.files[0].path).unwrap();
        let bases = fx.site.golden_bases();
        let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
        assert_golden(golden.join(file), &normalize(&text, &bases));
    }
}

// spec: OUT-084
// spec: ONPAGE-067
#[tokio::test]
async fn errors_report_includes_mixed_chain_but_excludes_success_and_loop() {
    let cfg = base_config();
    let fx = crawl_fixture_from("onpage-m2-rc", "http://site.test/", cfg.clone()).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:Redirects to Errors");
    assert_eq!(t.rows.len(), 1);
    let r = t.row("Address", "/refresh");
    assert_eq!(t.get(r, "Number of Redirects"), "2");
    assert_eq!(t.get(r, "Final Status Code"), "404");
    assert_eq!(t.get(r, "Chain Type"), "Mixed");
    assert_eq!(t.header.len(), 21);
}

// spec: OUT-084
#[tokio::test]
async fn all_redirect_types_include_blocked_no_response_and_server_error_targets() {
    let cfg = base_config();
    let fx = crawl_fixture_from("onpage-m2-rc", "http://site.test/", cfg.clone()).await;
    for kind in [
        "http",
        "meta_refresh",
        "http_refresh",
        "javascript",
        "hsts_policy",
    ] {
        fx.file
            .conn()
            .execute(
                "UPDATE urls SET redirect_type = ?1 WHERE address = 'http://site.test/refresh'",
                [kind],
            )
            .unwrap();
        for (code, blocked, status) in [
            (0, 0, "DNS Lookup Failed"),
            (0, 1, "Blocked by Robots.txt"),
            (503, 0, "Service Unavailable"),
        ] {
            fx.file.conn().execute("UPDATE urls SET status_code = ?1, robots_blocked = ?2, status = ?3 WHERE address = 'http://site.test/error'", (code, blocked, status)).unwrap();
            let (_tmp, t) = load(&fx, &cfg, "Redirects:Redirects to Errors");
            assert_eq!(t.rows.len(), 1, "{kind}: {status}");
            assert_eq!(t.get(&t.rows[0], "Final Status"), status);
        }
    }
}

// spec: OUT-085
#[tokio::test]
async fn list_reports_keep_each_uploaded_start_and_upload_order() {
    let cfg = base_config();
    let fx = crawl_fixture_from("onpage-m2-rc", "http://site.test/", cfg.clone()).await;
    fx.file.conn().execute("INSERT INTO list_input (position, original, url_id, outcome) SELECT 0, address, id, 'ok' FROM urls WHERE address = 'http://site.test/redirect'", []).unwrap();
    fx.file.conn().execute("INSERT INTO list_input (position, original, url_id, outcome) SELECT 1, address, id, 'ok' FROM urls WHERE address = 'http://site.test/refresh'", []).unwrap();
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    assert_eq!(
        t.column("Address"),
        ["http://site.test/redirect", "http://site.test/refresh"]
    );
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET redirect_target_id = NULL WHERE address = 'http://site.test/refresh'",
            [],
        )
        .unwrap();
    let (_tmp, t) = load(&fx, &cfg, "Redirects:All Redirects");
    assert_eq!(
        t.get(t.row("Address", "/refresh"), "Final Status"),
        "Not Followed"
    );
}

// spec: OUT-085
#[tokio::test]
async fn list_chain_report_keeps_not_followed_starts() {
    let cfg = base_config();
    let fx = crawl_fixture_from("onpage-m2-rc", "http://site.test/", cfg.clone()).await;
    fx.file.conn().execute("INSERT INTO list_input (position, original, url_id, outcome) SELECT 0, address, id, 'ok' FROM urls WHERE address = 'http://site.test/refresh'", []).unwrap();
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET redirect_target_id = NULL WHERE address = 'http://site.test/refresh'",
            [],
        )
        .unwrap();
    let (_tmp, t) = load(&fx, &cfg, "Redirects:Redirect Chains");
    assert_eq!(t.rows.len(), 1);
    assert_eq!(t.get(&t.rows[0], "Final Status"), "Not Followed");
    assert_eq!(t.get(&t.rows[0], "Number of Redirects"), "1");
}

// spec: OUT-084
// spec: ONPAGE-067
#[tokio::test]
async fn error_report_matches_failure_filters_and_writes_the_same_xlsx_table() {
    let cfg = base_config();
    let fx = crawl_fixture_from("onpage-m2-rc", "http://site.test/errors-root", cfg.clone()).await;
    let (_tmp, t) = load(&fx, &cfg, "Redirects:Redirects to Errors");
    assert_eq!(t.rows.len(), 4);
    for (path, hops, code, status) in [
        ("/r-to-404", "1", "404", "Not Found"),
        ("/r-to-blocked", "1", "0", "Blocked by Robots.txt"),
        ("/r-chain-500", "2", "500", "Internal Server Error"),
        ("/r-to-drop", "1", "0", "Connection Error"),
    ] {
        let row = t.row("Address", path);
        assert_eq!(t.get(row, "Number of Redirects"), hops, "{path}");
        assert_eq!(t.get(row, "Final Status Code"), code, "{path}");
        assert_eq!(t.get(row, "Final Status"), status, "{path}");
        let mut targets = Vec::new();
        for filter in [
            "Blocked by Robots.txt",
            "No Response",
            "Client Error (4XX)",
            "Server Error (5XX)",
        ] {
            let mut q = os_analyze::views::ViewQuery::new("Response Codes", filter);
            q.columns = os_analyze::views::ColumnSelection::Named(vec!["Address".into()]);
            targets.extend(
                os_analyze::views::query_view(&fx.file, &cfg, &q)
                    .unwrap()
                    .rows
                    .into_iter()
                    .map(|r| r[0].clone()),
            );
        }
        assert!(targets.contains(&os_model::Value::Text(t.get(row, "Final Address").into())));
    }
    let (_tmp, result) = report(&fx, &cfg, "Redirects:Redirects to Errors", Format::Xlsx);
    use calamine::Reader;
    let mut workbook: calamine::Xlsx<_> = calamine::open_workbook(&result.files[0].path).unwrap();
    let range = workbook.worksheet_range_at(0).unwrap().unwrap();
    let actual: Vec<Vec<String>> = range
        .rows()
        .map(|r| r.iter().map(ToString::to_string).collect())
        .collect();
    let mut expected = vec![t.header];
    expected.extend(t.rows);
    assert_eq!(actual, expected);
}

// spec: OUT-085
#[tokio::test]
async fn real_list_crawl_reports_unfollowed_targets_in_upload_order() {
    use os_engine::testing::{site_fetcher, test_deps};
    use os_engine::{start_crawl, CrawlRequest, CrawlSource};
    let site = os_testsite::TestSite::start("onpage-m2-rc").await.unwrap();
    let cfg = os_engine::list_input::list_config(base_config());
    let deps = test_deps();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("list.ospider");
    let handle = start_crawl(
        site_fetcher(&site, &cfg, &deps),
        CrawlRequest {
            config: cfg.clone(),
            source: CrawlSource::List {
                items: vec![addr("/r-chain-500"), addr("/refresh")],
                label: "redirect starts".into(),
            },
            file: path.clone(),
            project: "d12".into(),
            name: "list redirects".into(),
        },
        deps,
    )
    .await
    .unwrap();
    handle.wait().await.unwrap();
    let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap();
    for name in ["Redirects:All Redirects", "Redirects:Redirect Chains"] {
        let out = tempfile::tempdir().unwrap();
        let result = save_report(&file, &cfg, name, out.path(), Format::Csv).unwrap();
        let table = Table::read(&result.files[0].path);
        assert_eq!(
            table.column("Address"),
            [addr("/r-chain-500"), addr("/refresh")]
        );
        for row in &table.rows {
            assert_eq!(table.get(row, "Number of Redirects"), "1");
            assert_eq!(table.get(row, "Final Status"), "Not Followed");
            assert_eq!(table.get(row, "Final Status Code"), "");
        }
    }
    assert!(!site.requests().iter().any(
        |r| ["/r-to-500", "/redirect", "/e500", "/error"].contains(&r.path_and_query.as_str())
    ));
    let out = tempfile::tempdir().unwrap();
    let result = save_report(
        &file,
        &cfg,
        "Redirects:Redirects to Errors",
        out.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(result.files[0].rows, 0);
}

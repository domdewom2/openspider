//! Tab „Images“ gegen echte Crawls der Fixture `onpage-images`: Bildquellen, Spalten, Größe, Größenfilter und
//! die Alt-Filter je Instanz (M1-P23).

use os_model::FilterState;

const TAB: &str = "Images";
const SCENARIO: &str = "onpage-images";

// ---------------------------------------------------------------------------------------------------
// Gemeinsame Hilfen der Element-Tab-Tests (in jeder Testdatei des Pakets M1-P23 gleich: ein eigenes
// Hilfsmodul gehört nicht zum Besitz des Pakets).

use os_analyze::views::{filter_counts, query_view, ColumnSelection, ViewPage, ViewQuery};
use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_model::{CrawlConfig, Value};

/// Crawlt das Szenario vollständig (Testsite auf Loopback).
async fn crawl(scenario: &str, cfg: CrawlConfig) -> FixtureCrawl {
    crawl_fixture(scenario, cfg).await
}

/// Alle Zeilen einer Ansicht mit allen Spalten.
fn view(fx: &FixtureCrawl, cfg: &CrawlConfig, tab: &str, filter: &str) -> ViewPage {
    let mut q = ViewQuery::new(tab, filter);
    q.columns = ColumnSelection::All;
    q.limit = 500;
    query_view(&fx.file, cfg, &q).unwrap_or_else(|e| panic!("{tab}:{filter}: {e}"))
}

/// Adresse ohne Basis-URL (`/basic.html`; externe Adressen bleiben vollständig).
fn short(fx: &FixtureCrawl, address: &str) -> String {
    address
        .strip_prefix(&fx.site.base_url())
        .unwrap_or(address)
        .to_string()
}

/// Adressen einer Ansicht in Anzeigereihenfolge.
fn addresses(fx: &FixtureCrawl, cfg: &CrawlConfig, tab: &str, filter: &str) -> Vec<String> {
    view(fx, cfg, tab, filter)
        .rows
        .iter()
        .map(|r| match &r[0] {
            Value::Text(a) => short(fx, a),
            other => panic!("Address ist kein Text: {other:?}"),
        })
        .collect()
}

/// Erwartete Adressen, alphabetisch.
fn expect(list: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = list.iter().map(|s| (*s).to_string()).collect();
    v.sort();
    v
}

/// Wert einer Zelle der Zeile mit der Adresse `path`.
fn cell(fx: &FixtureCrawl, page: &ViewPage, path: &str, column: &str) -> Value {
    let col = page
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("Spalte {column} fehlt in {:?}", page.columns));
    let row = page
        .rows
        .iter()
        .find(|r| matches!(&r[0], Value::Text(a) if short(fx, a) == path))
        .unwrap_or_else(|| panic!("Zeile {path} fehlt"));
    row[col].clone()
}

fn text(s: &str) -> Value {
    Value::Text(s.to_string())
}

/// Zähler (Treffer, Anzeigename) eines Filters aus der Übersicht.
fn counted(fx: &FixtureCrawl, cfg: &CrawlConfig, tab: &str, filter_id: &str) -> (u64, String) {
    let counts = filter_counts(&fx.file, cfg).unwrap();
    let c = counts
        .iter()
        .find(|c| c.tab == tab && c.filter == filter_id)
        .unwrap_or_else(|| panic!("{tab}:{filter_id} fehlt in der Übersicht"));
    (c.count.expect("Filter ist bereit"), c.label.clone())
}

/// CSV-Wiedergabe einer Ansicht mit allen Spalten (Dateikopf, RFC 4180, UTF-8 mit BOM, `{BASE}`).
fn render_csv(fx: &FixtureCrawl, page: &ViewPage) -> String {
    fn field(v: &Value) -> String {
        let s = match v {
            Value::Null => String::new(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Text(t) => t.clone(),
        };
        if s.contains([',', '"', '\r', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s
        }
    }
    let mut out = String::from('\u{feff}');
    out.push_str(&page.columns.join(","));
    out.push('\n');
    for row in &page.rows {
        let cells: Vec<String> = row.iter().map(field).collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    // Das externe Bild liegt auf dem Host localhost (anderer Host derselben Site): {EXT}.
    let out = out.replace(&format!("http://localhost:{}", fx.site.port()), "{EXT}");
    os_testsite::golden::normalize(&out, &[&fx.site.base_url()])
}

/// Dateiname-Teil: klein, Buchstaben und Ziffern, sonst `_` (`Over 60 Characters` → `over_60_characters`).
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_string()
}

/// Vergleicht die Ansicht mit `tests/golden/<szenario>/<tab>__<filter>.csv` (`UPDATE_GOLDEN=1` schreibt).
fn assert_view_golden(
    fx: &FixtureCrawl,
    cfg: &CrawlConfig,
    scenario: &str,
    tab: &str,
    filter_id: &str,
) {
    let label = os_analyze::names::label(filter_id, cfg);
    let page = view(fx, cfg, tab, filter_id);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden")
        .join(scenario)
        .join(format!("{}__{}.csv", slug(tab), slug(&label)));
    os_testsite::golden::assert_golden(path, &render_csv(fx, &page));
}

/// Zeilen aus `image_instances` einer Seite: (Quelle, Rohwert, Alt vorhanden, Ziel gesetzt).
fn instances(fx: &FixtureCrawl, page: &str) -> Vec<(String, String, bool, bool)> {
    let mut stmt = fx
        .file
        .conn()
        .prepare(
            "SELECT i.source, i.src_raw, i.alt_present, i.image_id IS NOT NULL FROM image_instances i \
             JOIN urls p ON p.id = i.page_id WHERE p.address LIKE ?1 ORDER BY i.position",
        )
        .unwrap();
    stmt.query_map([format!("%{page}")], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

fn int(v: i64) -> Value {
    Value::Int(v)
}

/// Alle Bild-URLs der Fixture mit Antwort (alphabetisch, externe Adresse vollständig).
fn all_images() -> Vec<String> {
    expect(&[
        "/img/a.png",
        "/img/alt-100.png",
        "/img/alt-101.png",
        "/img/alt-bare.png",
        "/img/alt-empty.png",
        "/img/alt-entities.png",
        "/img/alt-none.png",
        "/img/alt-ok.png",
        "/img/alt-spaces.png",
        "/img/aria.png",
        "/img/big.jpg",
        "/img/both.png",
        "/img/chunked.png",
        "/img/clen.png",
        "/img/counted.png",
        "/img/gz.png",
        "/img/missing-big.png",
        "/img/missing.png",
        "/img/mixed.png",
        "/img/only-link.jpg",
        "/img/p.jpg",
        "/img/role.png",
        "/img/size-102400.png",
        "/img/size-102401.png",
        "EXT/img/ext-200k.png",
    ])
}

/// Adressen einer Ansicht; die externe Basis (`localhost`) wird zu `EXT`.
fn images_in(fx: &FixtureCrawl, cfg: &CrawlConfig, filter: &str) -> Vec<String> {
    let ext = format!("http://localhost:{}", fx.site.port());
    let mut v: Vec<String> = addresses(fx, cfg, TAB, filter)
        .into_iter()
        .map(|a| a.replace(&ext, "EXT"))
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn filter_registry_ids_are_unchanged() {
    // Infrastruktur: die Kennungen kommen aus docs/plan/M1-filters.toml (Details prüft filters_registry.rs).
    let ids: Vec<&str> = os_analyze::tabs::images::tab()
        .filters
        .iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(
        ids,
        [
            "All",
            "Over {image.max_kb} KB",
            "Missing Alt Text",
            "Missing Alt Attribute",
            "Alt Text Over {image.alt_max_chars} Characters",
            "Background Images",
            "Missing Size Attributes",
            "Incorrectly Sized Images"
        ]
    );
}

// spec: ONPAGE-170
#[tokio::test]
async fn image_sources_img_and_link_are_found_and_srcset_is_only_an_instance() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let all = images_in(&fx, &cfg, "All");
    assert_eq!(all, all_images());
    // <img src> und <a href> auf ein Bild stehen im Tab, intern wie extern.
    for found in [
        "/img/a.png",
        "/img/big.jpg",
        "/img/p.jpg",
        "EXT/img/ext-200k.png",
    ] {
        assert!(all.contains(&found.to_string()), "{found}");
    }
    // srcset-Kandidaten (Option „Extract images from srcset“ aus), Lazy-Load und data:-URIs: keine Zeile.
    for absent in ["/img/s.png", "/img/l.png", "/img/p.webp", "/img/lazy.png"] {
        assert!(!all.contains(&absent.to_string()), "{absent}");
    }
    assert!(all.iter().all(|a| !a.starts_with("data:")));

    // Instanzen der Seite sources.html: Quelle img/a/srcset; a-Instanz ohne Alt-Information; die
    // Kandidaten aus srcset haben keine URL-Zeile (Ziel nicht gesetzt), weil nichts sie abruft.
    let inst = instances(&fx, "/sources.html");
    let find = |raw: &str| inst.iter().find(|i| i.1 == raw).cloned();
    assert_eq!(
        find("img/a.png"),
        Some(("img".into(), "img/a.png".into(), true, true))
    );
    assert_eq!(
        find("img/big.jpg"),
        Some(("a".into(), "img/big.jpg".into(), false, true))
    );
    assert_eq!(
        find("img/p.jpg"),
        Some(("img".into(), "img/p.jpg".into(), true, true))
    );
    for srcset in ["img/s.png", "img/l.png", "img/p.webp"] {
        let i = find(srcset).unwrap_or_else(|| panic!("Instanz {srcset} fehlt"));
        assert_eq!(i.0, "srcset", "{srcset}");
        assert!(!i.3, "{srcset} hat keine URL-Zeile");
    }
    assert!(find("img/lazy.png").is_none());
    assert!(inst.iter().all(|i| !i.1.starts_with("data:")));

    // Der Fixture-Server erhielt keine Anfrage auf die srcset-Kandidaten, den Lazy-Load-Wert und die
    // data:-URI; die übrigen Bilder wurden abgerufen.
    for path in ["/img/s.png", "/img/l.png", "/img/p.webp", "/img/lazy.png"] {
        assert_eq!(
            fx.site.count("127.0.0.1", path),
            0,
            "{path} wurde abgerufen"
        );
    }
    assert_eq!(fx.site.count("127.0.0.1", "/img/a.png"), 1);
    assert_eq!(fx.site.count("127.0.0.1", "/img/big.jpg"), 1);
}

// spec: ONPAGE-170
#[tokio::test]
async fn image_address_comes_from_the_target_not_from_the_raw_attribute() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    // K-37: links.target_raw ist der Rohwert wie im Markup; die Adresse im Tab kommt aus urls.
    let raw: String = fx
        .file
        .conn()
        .query_row(
            "SELECT l.target_raw FROM links l JOIN urls p ON p.id = l.source_id \
             WHERE p.address LIKE '%/sources.html' AND l.link_type = 'Image' AND l.target_raw LIKE '%a.png'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(raw, "img/a.png");
    let page = view(&fx, &cfg, TAB, "All");
    let base = fx.site.base_url();
    assert!(page
        .rows
        .iter()
        .any(|r| r[0] == Value::Text(format!("{base}/img/a.png"))));
    assert!(page
        .rows
        .iter()
        .all(|r| matches!(&r[0], Value::Text(a) if a.starts_with("http"))));
}

// spec: ONPAGE-170
#[tokio::test]
async fn check_images_off_leaves_the_tab_empty_and_store_only_lists_without_download() {
    let mut off = CrawlConfig::default();
    off.spider.images = os_model::StoreCrawl::OFF;
    let fx = crawl(SCENARIO, off).await;
    let cfg = fx.file.config().unwrap();
    // Die <img>-Bilder fehlen und werden nicht abgerufen; nur die per <a href> verlinkten Bilder sind als
    // Hyperlink-Ziele (Spider → Interne Hyperlinks) trotzdem Zeilen und stehen als Bildinstanz im Tab.
    assert_eq!(
        images_in(&fx, &cfg, "All"),
        expect(&["/img/big.jpg", "/img/only-link.jpg"])
    );
    assert_eq!(fx.site.count("127.0.0.1", "/img/a.png"), 0);

    let mut store_only = CrawlConfig::default();
    store_only.spider.images = os_model::StoreCrawl::STORE_ONLY;
    let fx = crawl(SCENARIO, store_only).await;
    let cfg = fx.file.config().unwrap();
    assert!(view(&fx, &cfg, TAB, "All").total > 0);
    assert_eq!(
        fx.site.count("127.0.0.1", "/img/a.png"),
        0,
        "nur gespeichert"
    );
    // Nicht geladen: Größe 0, in keinem Größenfilter.
    let page = view(&fx, &cfg, TAB, "All");
    assert_eq!(cell(&fx, &page, "/img/a.png", "Size"), int(0));
    assert!(images_in(&fx, &cfg, "Over {image.max_kb} KB").is_empty());
}

// spec: ONPAGE-170 [M2]
#[test]
#[ignore = "M2: ONPAGE-170 Lazy-Load-Bilder (data-src) brauchen Rendering (M2-R02)"]
fn lazy_load_images_are_found_with_rendering() {}

// spec: ONPAGE-171
#[tokio::test]
async fn columns_and_img_inlinks() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let tab = os_analyze::tabs::images::tab();
    let keys: Vec<&str> = tab.columns.iter().map(|c| c.key).collect();
    assert_eq!(
        keys,
        [
            "Address",
            "Content",
            "Size",
            "IMG Inlinks",
            "Indexability",
            "Indexability Status"
        ]
    );
    assert_eq!(tab.columns[0].kind, os_analyze::ColumnKind::Url);
    assert!(tab.columns.iter().all(|c| c.default_visible));
    let page = view(&fx, &cfg, TAB, "All");
    assert_eq!(page.columns, keys);

    // Export in Bytes (die UI rechnet in KB um): 102 400 Byte bleiben 102400.
    assert_eq!(
        cell(&fx, &page, "/img/size-102400.png", "Size"),
        int(102_400)
    );
    assert_eq!(cell(&fx, &page, "/img/a.png", "Content"), text("image/png"));
    assert_eq!(
        cell(&fx, &page, "/img/big.jpg", "Content"),
        text("image/jpeg")
    );
    assert_eq!(
        cell(&fx, &page, "/img/a.png", "Indexability"),
        text("Indexable")
    );
    assert_eq!(
        cell(&fx, &page, "/img/missing.png", "Indexability"),
        text("Non-Indexable")
    );

    // Bild auf 3 Seiten (davon 2× auf einer): IMG Inlinks 3; ein Bild auf 2 Seiten: 2; <a href>-Bild: 1.
    assert_eq!(cell(&fx, &page, "/img/counted.png", "IMG Inlinks"), int(3));
    assert_eq!(cell(&fx, &page, "/img/mixed.png", "IMG Inlinks"), int(2));
    assert_eq!(cell(&fx, &page, "/img/big.jpg", "IMG Inlinks"), int(1));
    assert_eq!(cell(&fx, &page, "/img/both.png", "IMG Inlinks"), int(1));
    // Gegenprobe aus den Instanzen: verschiedene interne Seiten je Bild.
    let mut stmt = fx
        .file
        .conn()
        .prepare(
            "SELECT u.address, COUNT(DISTINCT i.page_id) FROM image_instances i JOIN urls u ON u.id = i.image_id \
             JOIN urls s ON s.id = i.page_id AND s.is_internal = 1 GROUP BY u.id",
        )
        .unwrap();
    let recomputed: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(!recomputed.is_empty());
    for (address, n) in recomputed {
        let row = page
            .rows
            .iter()
            .find(|r| r[0] == Value::Text(address.clone()))
            .unwrap_or_else(|| panic!("{address} fehlt im Tab"));
        assert_eq!(row[3], int(n), "IMG Inlinks von {address}");
    }
    assert_view_golden(&fx, &cfg, SCENARIO, TAB, "All");
}

// spec: ONPAGE-173
#[tokio::test]
async fn size_is_body_bytes_then_content_length_then_zero() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let page = view(&fx, &cfg, TAB, "All");
    // Korrekte Content-Length → Länge.
    assert_eq!(cell(&fx, &page, "/img/clen.png", "Size"), int(1234));
    // Chunked ohne Content-Length → tatsächliche Bytes, nicht 0.
    let headers: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM http_headers h JOIN urls u ON u.id = h.url_id \
             WHERE u.address LIKE '%/img/chunked.png' AND h.direction = 'response' AND lower(h.name) = 'content-length'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(headers, 0, "die Route sendet kein Content-Length");
    assert_eq!(cell(&fx, &page, "/img/chunked.png", "Size"), int(5000));
    // Komprimiert ausgeliefert: Size ist der dekomprimierte Körper (ONPAGE-173, A-6).
    assert_eq!(cell(&fx, &page, "/img/gz.png", "Size"), int(60_000));
    // 404: Size 0, auch wenn die Fehlerseite 300 000 Byte groß ist.
    assert_eq!(cell(&fx, &page, "/img/missing.png", "Size"), int(0));
    assert_eq!(cell(&fx, &page, "/img/missing-big.png", "Size"), int(0));
    let stored: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT size_bytes FROM urls WHERE address LIKE '%/img/missing-big.png'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(stored >= 300_000, "die Fehlerseite wurde geladen: {stored}");
    // …und erscheint deshalb auch in keinem Größenfilter, selbst bei einer Schwelle von 0 KB.
    let mut zero = cfg.clone();
    zero.image.max_kb = 0;
    let over = images_in(&fx, &zero, "Over {image.max_kb} KB");
    assert!(!over.contains(&"/img/missing-big.png".to_string()));
    assert!(!over.contains(&"/img/missing.png".to_string()));
    assert_eq!(over.len(), all_images().len() - 2);
}

// spec: ONPAGE-173
#[tokio::test]
async fn size_falls_back_to_content_length_header_when_no_body_was_stored() {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.http_headers = true;
    let fx = crawl(SCENARIO, cfg).await;
    let cfg = fx.file.config().unwrap();
    // Simuliert eine nicht geladene Antwort mit bekanntem Header (gekürzter Download, zulässige
    // Optimierung): Körpergröße unbekannt, Content-Length vorhanden.
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET size_bytes = NULL WHERE address LIKE '%/img/clen.png'",
            [],
        )
        .unwrap();
    let page = view(&fx, &cfg, TAB, "All");
    assert_eq!(cell(&fx, &page, "/img/clen.png", "Size"), int(1234));
    // Ohne Körper und ohne Header: 0.
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET size_bytes = NULL WHERE address LIKE '%/img/chunked.png'",
            [],
        )
        .unwrap();
    let page = view(&fx, &cfg, TAB, "All");
    assert_eq!(cell(&fx, &page, "/img/chunked.png", "Size"), int(0));
}

// spec: ONPAGE-174
#[tokio::test]
async fn over_kb_compares_bytes_against_the_threshold_for_internal_and_external_images() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let over = images_in(&fx, &cfg, "Over {image.max_kb} KB");
    // 102 400 Byte: nicht; 102 401 Byte: ja; externes 200-KB-Bild: ja.
    assert_eq!(
        over,
        expect(&["/img/size-102401.png", "EXT/img/ext-200k.png"])
    );
    assert_eq!(
        counted(&fx, &cfg, TAB, "Over {image.max_kb} KB"),
        (2, "Over 100 KB".to_string())
    );
    assert_view_golden(&fx, &cfg, SCENARIO, TAB, "Over {image.max_kb} KB");

    // image.max_kb = 200: das 200-KB-Bild (204 800 Byte) ist nicht mehr dabei, auch kein anderes.
    let mut c200 = cfg.clone();
    c200.image.max_kb = 200;
    assert!(images_in(&fx, &c200, "Over {image.max_kb} KB").is_empty());
    assert_eq!(
        counted(&fx, &c200, TAB, "Over {image.max_kb} KB").1,
        "Over 200 KB"
    );
    // 199 KB (203 776 Byte): 204 800 Byte liegen darüber.
    let mut c199 = cfg.clone();
    c199.image.max_kb = 199;
    assert_eq!(
        images_in(&fx, &c199, "Over {image.max_kb} KB"),
        expect(&["EXT/img/ext-200k.png"])
    );
    // Der Name löst sich auch in der Eingabeform auf.
    assert_eq!(
        os_analyze::views::resolve_name("images:over 100 kb").unwrap(),
        ("Images".to_string(), "Over {image.max_kb} KB".to_string())
    );
}

// spec: ONPAGE-175
#[tokio::test]
async fn missing_alt_text_is_an_empty_alt_on_at_least_one_img_instance() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let missing = images_in(&fx, &cfg, "Missing Alt Text");
    assert_eq!(
        missing,
        expect(&[
            // alt="" und alt="   "
            "/img/alt-empty.png",
            "/img/alt-spaces.png",
            // alt ohne Wert
            "/img/alt-bare.png",
            // Seite 1 alt="Logo", Seite 2 alt=""
            "/img/mixed.png",
            // role="presentation" / aria-hidden befreien nicht (A-46)
            "/img/role.png",
            "/img/aria.png",
            // alt="" und ohne alt auf derselben Seite
            "/img/both.png",
        ])
    );
    // Kein Attribut: nicht hier; Bild mit Alt-Text: nicht; nur über <a href> verlinkt: nicht.
    for absent in [
        "/img/alt-none.png",
        "/img/alt-ok.png",
        "/img/only-link.jpg",
        "/img/big.jpg",
    ] {
        assert!(!missing.contains(&absent.to_string()), "{absent}");
    }
    assert_eq!(counted(&fx, &cfg, TAB, "Missing Alt Text").0, 7);
    assert_view_golden(&fx, &cfg, SCENARIO, TAB, "Missing Alt Text");
}

// spec: ONPAGE-176
#[tokio::test]
async fn missing_alt_attribute_is_an_img_instance_without_alt() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let missing = images_in(&fx, &cfg, "Missing Alt Attribute");
    // Ohne alt-Attribut; alt ohne Wert gehört zu Missing Alt Text; <a href>-Bilder ohne Alt-Information
    // zählen nicht; both.png steht (andere Instanz) in beiden Filtern.
    assert_eq!(missing, expect(&["/img/alt-none.png", "/img/both.png"]));
    let text_missing = images_in(&fx, &cfg, "Missing Alt Text");
    assert!(text_missing.contains(&"/img/both.png".to_string()));
    assert!(!missing.contains(&"/img/alt-bare.png".to_string()));
    assert!(!missing.contains(&"/img/only-link.jpg".to_string()));
    assert!(!missing.contains(&"/img/big.jpg".to_string()));
    assert_view_golden(&fx, &cfg, SCENARIO, TAB, "Missing Alt Attribute");
}

// spec: ONPAGE-177
#[tokio::test]
async fn alt_text_over_n_characters_measures_decoded_codepoints() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let over = images_in(&fx, &cfg, "Alt Text Over {image.alt_max_chars} Characters");
    // 100 Zeichen: nicht; 101: ja; 99 Zeichen + &amp; (dekodiert 100, im Quelltext 104): nicht.
    assert_eq!(over, expect(&["/img/alt-101.png"]));
    assert_eq!(
        counted(
            &fx,
            &cfg,
            TAB,
            "Alt Text Over {image.alt_max_chars} Characters"
        ),
        (1, "Alt Text Over 100 Characters".to_string())
    );
    let mut c99 = cfg.clone();
    c99.image.alt_max_chars = 99;
    assert_eq!(
        images_in(&fx, &c99, "Alt Text Over {image.alt_max_chars} Characters"),
        expect(&[
            "/img/alt-100.png",
            "/img/alt-101.png",
            "/img/alt-entities.png"
        ])
    );
    assert_eq!(
        counted(
            &fx,
            &c99,
            TAB,
            "Alt Text Over {image.alt_max_chars} Characters"
        )
        .1,
        "Alt Text Over 99 Characters"
    );
    assert_view_golden(
        &fx,
        &cfg,
        SCENARIO,
        TAB,
        "Alt Text Over {image.alt_max_chars} Characters",
    );
}

// spec: ONPAGE-171, ONPAGE-174, ONPAGE-175, ONPAGE-176, ONPAGE-177
#[tokio::test]
async fn filters_are_live_subsets_of_all_and_ready() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let all = images_in(&fx, &cfg, "All");
    let tab = os_analyze::tabs::images::tab();
    for filter in tab.filters.iter().filter(|f| f.requires.is_empty()) {
        let rows = images_in(&fx, &cfg, filter.id);
        assert!(rows.iter().all(|a| all.contains(a)), "{} ⊆ All", filter.id);
        assert_eq!(counted(&fx, &cfg, TAB, filter.id).0 as usize, rows.len());
    }
    for c in filter_counts(&fx.file, &cfg).unwrap().iter().filter(|c| {
        c.tab == TAB
            && ![
                "Background Images",
                "Missing Size Attributes",
                "Incorrectly Sized Images",
            ]
            .contains(&c.filter.as_str())
    }) {
        assert_eq!(c.state, FilterState::Ready, "{}", c.filter);
    }
    // Die M1-Filter bleiben live; neue Nachlauffilter prüfen tab_images_m2.rs.
    assert!(tab.filters[..5].iter().all(|f| f.requires.is_empty()));
    // Die Seiten selbst (HTML) stehen nicht im Tab.
    assert!(all.iter().all(|a| !a.ends_with(".html") && a != "/"));
}

// spec: ONPAGE-170 [M2]
// spec: CRAWL-055
#[tokio::test]
async fn srcset_option_crawls_candidates_and_default_keeps_unfetched_detail_instances() {
    for enabled in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.extraction.srcset = enabled;
        let fx = crawl("onpage-m2-images", cfg).await;
        let cfg = fx.file.config().unwrap();
        let images = images_in(&fx, &cfg, "All");
        for path in ["/s.png", "/l.png", "/p.webp"] {
            assert_eq!(images.contains(&path.to_string()), enabled, "{path}");
            assert_eq!(
                fx.site
                    .requests()
                    .iter()
                    .any(|request| request.path() == path),
                enabled,
                "Request {path}"
            );
        }
        let rows = instances(&fx, "/");
        assert_eq!(
            rows.iter()
                .filter(|(source, _, _, _)| source == "srcset")
                .count(),
            3
        );
        for (source, _, _, has_id) in rows {
            if source == "srcset" {
                assert_eq!(has_id, enabled);
            }
        }
        assert!(!images.contains(&"/lazy.png".to_string()));
    }
}

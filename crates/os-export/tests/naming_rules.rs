//! Tests für `naming::file_name` (OUT-008).

use os_export::naming::{collisions, file_name, MAX_STEM_LEN};

fn stem_len(name: &str) -> usize {
    name.rsplit_once('.').map_or(name.len(), |(s, _)| s.len())
}

// spec: OUT-008
#[test]
fn spec_examples_are_normalized() {
    assert_eq!(file_name("Internal:All", None, "csv"), "internal_all.csv");
    assert_eq!(
        file_name("response_codes_client_error_(4xx)", None, "csv"),
        "response_codes_client_error_4xx.csv"
    );
    assert_eq!(
        file_name("Response Codes:Client Error (4xx)", None, "csv"),
        "response_codes_client_error_4xx.csv"
    );
    assert_eq!(file_name("All Inlinks", None, "csv"), "all_inlinks.csv");
    assert_eq!(
        file_name("Redirect Chains", None, "xlsx"),
        "redirect_chains.xlsx"
    );
    assert_eq!(
        file_name("Crawl Overview", None, "csv"),
        "crawl_overview.csv"
    );
    assert_eq!(
        file_name("Links:Non-Indexable Page Inlinks Only", None, "csv"),
        "links_non-indexable_page_inlinks_only.csv"
    );
}

// spec: OUT-008
#[test]
fn only_allowed_characters_and_collapsed_underscores() {
    let n = file_name("  __Ärger / Über:  ?*<>|\"Größe\"__  ", None, "csv");
    let stem = n.strip_suffix(".csv").unwrap();
    assert!(
        stem.chars()
            .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.')),
        "{n}"
    );
    assert!(!stem.contains("__"), "{n}");
    assert!(!stem.starts_with('_') && !stem.ends_with('_'), "{n}");
    assert_eq!(file_name("a   b\t\tc", None, "csv"), "a_b_c.csv");
    assert_eq!(file_name("___a___", None, "csv"), "a.csv");
    assert_eq!(file_name("", None, "csv"), "export.csv");
    assert_eq!(file_name("???", None, "csv"), "export.csv");
}

// spec: OUT-008
#[test]
fn long_names_are_cut_before_the_extension_and_stay_unique() {
    let a = format!("{}A", "x".repeat(300));
    let b = format!("{}B", "x".repeat(300));
    let na = file_name(&a, None, "csv");
    let nb = file_name(&b, None, "csv");
    assert!(stem_len(&na) <= MAX_STEM_LEN, "{na}");
    assert!(stem_len(&nb) <= MAX_STEM_LEN, "{nb}");
    assert!(na.ends_with(".csv"));
    assert_ne!(na, nb, "gekürzte Namen müssen eindeutig bleiben");
    // genau an der Grenze wird nichts verändert
    let exact = "y".repeat(MAX_STEM_LEN);
    assert_eq!(file_name(&exact, None, "csv"), format!("{exact}.csv"));
    let over = "y".repeat(MAX_STEM_LEN + 1);
    assert!(stem_len(&file_name(&over, None, "csv")) <= MAX_STEM_LEN);
}

// spec: OUT-008
#[test]
fn windows_reserved_names_are_avoided() {
    for reserved in [
        "con",
        "PRN",
        "aux",
        "Nul",
        "com1",
        "COM9",
        "lpt1",
        "LPT9",
        "nul.backup",
    ] {
        let n = file_name(reserved, None, "csv");
        let first = n.split('.').next().unwrap().to_ascii_lowercase();
        assert!(
            !matches!(first.as_str(), "con" | "prn" | "aux" | "nul")
                && !(first.len() == 4
                    && (first.starts_with("com") || first.starts_with("lpt"))
                    && first.as_bytes()[3].is_ascii_digit()),
            "{reserved} -> {n}"
        );
    }
    // ähnlich aussehende, aber gültige Namen bleiben unverändert
    assert_eq!(file_name("console", None, "csv"), "console.csv");
    assert_eq!(file_name("com10", None, "csv"), "com10.csv");
    assert_eq!(file_name("com", None, "csv"), "com.csv");
}

// spec: OUT-008
#[test]
fn segment_exports_get_segment_suffix() {
    assert_eq!(
        file_name("Internal:All", Some("Blog Posts"), "csv"),
        "internal_all_segment_blog_posts.csv"
    );
    assert_eq!(
        file_name("Internal:All", Some("en-US"), "csv"),
        "internal_all_segment_en-us.csv"
    );
    // verschiedene Segmente ohne ASCII-Zeichen ergeben verschiedene Dateien
    let a = file_name("Internal:All", Some("日本"), "csv");
    let b = file_name("Internal:All", Some("中国"), "csv");
    assert_ne!(a, b);
    assert!(a.contains("_segment_"));
    // lange Namen: das Segmentsuffix bleibt sichtbar, die Länge hält
    let long = "q".repeat(200);
    let n = file_name(&long, Some("Blog"), "csv");
    assert!(stem_len(&n) <= MAX_STEM_LEN, "{n}");
    assert!(n.ends_with("_segment_blog.csv"), "{n}");
    let n2 = file_name(&format!("{long}2"), Some("Blog"), "csv");
    assert_ne!(n, n2);
    // sehr langes Segment
    let n3 = file_name("Internal:All", Some(&"s".repeat(300)), "csv");
    assert!(stem_len(&n3) <= MAX_STEM_LEN, "{n3}");
}

// spec: OUT-008
#[test]
fn no_two_registered_exports_share_a_file_name() {
    let mut names: Vec<String> = Vec::new();
    for tab in os_analyze::tabs::all() {
        for f in &tab.filters {
            names.push(format!("{}:{}", tab.id, f.id));
        }
    }
    assert!(
        names.len() > 20,
        "Platzhalter-Registry sollte Tabs enthalten"
    );
    let dups = collisions(names.iter().map(String::as_str));
    assert!(dups.is_empty(), "Kollisionen: {dups:?}");

    // und der Kollisionsdetektor erkennt echte Kollisionen
    let dups = collisions(["Internal:All", "internal all", "Other"]);
    assert_eq!(dups.len(), 1);
    assert_eq!(dups[0].0, "internal_all.csv");
}

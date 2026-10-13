//! Tests des Vergleichs-Kerns: Schlüsselabgleich, Normalisierer, Toleranzen, Abweichungen, Schwellen.

use os_parity::csvio::{parse_csv, Table};
use os_parity::deviations::Deviations;
use os_parity::diff::{compare, evaluate, CompareInput};
use os_parity::mapping::Mapping;
use os_parity::normalize::BaseUrls;
use os_parity::report::{render_markdown, Report};
use os_parity::thresholds::Thresholds;

const MAPPING: &str = r#"
key = ["Address"]
drop_sf_columns = ["Description", "How To Fix"]
[columns]
"Address" = "Address"
"Status Code" = "Status Code"
"Title 1" = "Title 1"
"Title 1 Pixel Width" = { os = "Title 1 Pixel Width", tolerance_pct = 3.0 }
"Response Time" = { ignore = true, reason = "nicht deterministisch" }
"Indexable" = { os = "Indexability", bool = true }
"Indexability Status" = { os = "Indexability Status", map = { "Canonicalised" = "Canonical" } }
"Size (bytes)" = { os = "Size", number_decimals = 0 }
"Canonical Link Element 1" = "Canonical Link Element 1"
[normalize]
base_url = true
trim = true
bool = "case-insensitive"
number_decimals = 3
empty_equals_null = true
"#;

// Referenz: BOM, CRLF, Semikolon; Beschreibungsspalten mit "geheimen" Texten; Port 8902.
fn sf_csv() -> Vec<u8> {
    let mut v = vec![0xEF, 0xBB, 0xBF];
    v.extend_from_slice(
        concat!(
            "Address;Status Code;Title 1;Title 1 Pixel Width;Response Time;Indexable;Indexability Status;Size (bytes);Canonical Link Element 1;Description;How To Fix;Meta Robots 1\r\n",
            "http://127.0.0.1:8902/;200;Start;300;0.12;Yes;;1000.0;http://127.0.0.1:8902/;geheimer Text A;geheimer Text B;\r\n",
            "http://127.0.0.1:8902/a;200;Seite A;200;0.30;yes;;2048;http://localhost:8902/extern;geheimer Text A;geheimer Text B;noindex\r\n",
            "http://127.0.0.1:8902/b;200;Seite B;400;0.10;No;Canonicalised;10;;geheimer Text A;geheimer Text B;\r\n",
            "http://127.0.0.1:8902/c;404;;0;0.05;No;Client Error;0;;x;y;\r\n",
            "http://127.0.0.1:8902/gone;200;Weg;100;0.01;Yes;;5;;x;y;\r\n",
        )
        .as_bytes(),
    );
    v
}

// OpenSpider: anderer Port, Komma, LF; Erweiterungsspalte "Why It Matters".
fn os_csv() -> &'static str {
    concat!(
        "Address,Status Code,Title 1,Title 1 Pixel Width,Response Time,Indexability,Indexability Status,Size,Canonical Link Element 1,Why It Matters\n",
        "http://127.0.0.1:41234/,200,Start,309,0.99,TRUE,,1000,http://127.0.0.1:41234/,Erweiterung\n",
        "http://127.0.0.1:41234/a,200,Seite A,205,0.99,true,,2048,http://localhost:41234/extern,Erweiterung\n",
        "http://127.0.0.1:41234/b,200,Seite B anders,400,0.99,false,Canonical,10,,Erweiterung\n",
        "http://127.0.0.1:41234/c,404,,0,0.99,false,Client Error,0,null,Erweiterung\n",
        "http://127.0.0.1:41234/new,200,Neu,10,0.99,true,,1,,Erweiterung\n",
    )
}

fn report_with(
    mapping: &Mapping,
    sf: &Table,
    os: &Table,
    devs: &[os_parity::deviations::Deviation],
) -> Report {
    compare(&CompareInput {
        export: "internal_all",
        site: Some("parity-onpage"),
        sf,
        os,
        mapping,
        sf_bases: &BaseUrls::from_base("http://127.0.0.1:8902/"),
        os_bases: &BaseUrls::from_base("http://127.0.0.1:41234"),
        deviations: devs,
    })
    .unwrap()
}

fn scenario() -> (Mapping, Table, Table) {
    let mapping = Mapping::parse(MAPPING).unwrap();
    let sf = parse_csv(&sf_csv(), &mapping.drop_sf_columns).unwrap();
    let os = parse_csv(os_csv().as_bytes(), &[]).unwrap();
    (mapping, sf, os)
}

#[test]
fn key_matching_reports_missing_and_extra_rows() {
    let (m, sf, os) = scenario();
    let r = report_with(&m, &sf, &os, &[]);
    assert_eq!(r.sf_rows, 5);
    assert_eq!(r.os_rows, 5);
    assert_eq!(r.matched, 4);
    assert_eq!(r.missing_in_os, ["{BASE}/gone"]);
    assert_eq!(r.extra_in_os, ["{BASE}/new"]);
    assert!(!r.url_set_identical);
}

#[test]
fn cell_rules_apply_per_column() {
    let (m, sf, os) = scenario();
    let r = report_with(&m, &sf, &os, &[]);
    let col = |name: &str| r.columns.iter().find(|c| c.sf_column == name).unwrap();
    // Toleranz 3 %: 300 vs 309 (3,0 %) ok, 200 vs 205 (2,5 %) ok.
    assert_eq!(col("Title 1 Pixel Width").errors, 0);
    // Titel: "Seite B" vs "Seite B anders" -> genau ein Fehler mit Beispiel.
    assert_eq!(col("Title 1").errors, 1);
    let ex = &col("Title 1").examples[0];
    assert_eq!(ex.key, "{BASE}/b");
    assert_eq!(ex.sf, "Seite B");
    assert_eq!(ex.os, "Seite B anders");
    // Bool: Yes/TRUE, yes/true, No/false gleich.
    assert_eq!(col("Indexable").errors, 0);
    // Wertabbildung Canonicalised -> Canonical; Zahl 1000.0 vs 1000.
    assert_eq!(col("Indexability Status").errors, 0);
    assert_eq!(col("Size (bytes)").errors, 0);
    assert_eq!(col("Size (bytes)").os_column, "Size");
    // {EXT}-Normalisierung und empty_equals_null ("" vs "null").
    assert_eq!(col("Canonical Link Element 1").errors, 0);
    // Spalte "Status Code" fehlerfrei.
    assert_eq!(col("Status Code").errors, 0);
    assert_eq!(col("Status Code").compared, 4);
}

#[test]
fn counts_agreement_and_percentage() {
    let (m, sf, os) = scenario();
    let r = report_with(&m, &sf, &os, &[]);
    // 7 verglichene Spalten x 4 zugeordnete Zeilen; ein Fehler (Titel von Seite B).
    assert_eq!(r.cells_compared, 28);
    assert_eq!(r.cells_equal, 27);
    assert_eq!(r.agreement_pct, 96.429);
}

#[test]
fn unmapped_ignored_and_dropped_columns_are_listed_not_compared() {
    let (m, sf, os) = scenario();
    let r = report_with(&m, &sf, &os, &[]);
    assert_eq!(r.unmapped_os_columns, ["Why It Matters"]);
    assert_eq!(r.unmapped_sf_columns, ["Meta Robots 1"]);
    assert_eq!(r.dropped_sf_columns, ["Description", "How To Fix"]);
    assert_eq!(r.ignored_columns.len(), 1);
    assert_eq!(r.ignored_columns[0].column, "Response Time");
    assert_eq!(
        r.ignored_columns[0].reason.as_deref(),
        Some("nicht deterministisch")
    );
    assert!(r.columns.iter().all(|c| c.sf_column != "Response Time"));
    assert!(r.columns.iter().all(|c| c.os_column != "Why It Matters"));
}

#[test]
fn reference_description_texts_never_reach_the_report() {
    let (m, sf, os) = scenario();
    let r = report_with(&m, &sf, &os, &[]);
    let json = serde_json::to_string(&r).unwrap();
    let md = render_markdown(&r);
    assert!(!json.contains("geheimer"));
    assert!(!md.contains("geheimer"));
}

#[test]
fn composite_key_with_running_number_matches_duplicates() {
    let mapping = Mapping::parse(
        "key = [\"Source\", \"Destination\", \"Type\", \"#\"]\n[columns]\n\"Anchor\" = \"Anchor\"\n",
    )
    .unwrap();
    let sf = parse_csv(
        b"Type,Source,Destination,Anchor\nHyperlink,http://127.0.0.1:1/a,http://127.0.0.1:1/b,eins\nHyperlink,http://127.0.0.1:1/a,http://127.0.0.1:1/b,zwei\nImage,http://127.0.0.1:1/a,http://127.0.0.1:1/i.png,\n",
        &[],
    )
    .unwrap();
    let os_full = parse_csv(
        b"Type,Source,Destination,Anchor\nImage,http://127.0.0.1:2/a,http://127.0.0.1:2/i.png,\nHyperlink,http://127.0.0.1:2/a,http://127.0.0.1:2/b,eins\nHyperlink,http://127.0.0.1:2/a,http://127.0.0.1:2/b,zwei\n",
        &[],
    )
    .unwrap();
    let run = |os: &Table| {
        compare(&CompareInput {
            export: "all_inlinks",
            site: None,
            sf: &sf,
            os,
            mapping: &mapping,
            sf_bases: &BaseUrls::from_base("http://127.0.0.1:1"),
            os_bases: &BaseUrls::from_base("http://127.0.0.1:2"),
            deviations: &[],
        })
        .unwrap()
    };
    let r = run(&os_full);
    assert_eq!(r.matched, 3);
    assert!(r.url_set_identical);
    assert_eq!(r.cells_equal, 3);

    // Fehlt der zweite gleiche Link, steht die laufende Nummer im Schlüssel.
    let os_short = parse_csv(
        b"Type,Source,Destination,Anchor\nHyperlink,http://127.0.0.1:2/a,http://127.0.0.1:2/b,eins\nImage,http://127.0.0.1:2/a,http://127.0.0.1:2/i.png,\n",
        &[],
    )
    .unwrap();
    let r = run(&os_short);
    assert_eq!(r.matched, 2);
    assert_eq!(r.missing_in_os, ["{BASE}/a | {BASE}/b | Hyperlink | #2"]);

    // Gleiche Reihenfolge-unabhängige Zuordnung: Reihenfolge der Zeilen spielt keine Rolle.
    assert_eq!(run(&os_full).cells_compared, 3);
}

#[test]
fn deviations_remove_cells_and_rows_from_the_comparison() {
    let (m, sf, os) = scenario();
    let devs = Deviations::parse(
        r#"
[[deviation]]
id = "DEV-1"
spec = "ONPAGE-004"
kind = "OS-Festlegung"
note = "Titel Seite B ist absichtlich anders"
column = "Title 1"
key = "{BASE}/b"

[[deviation]]
id = "DEV-2"
spec = "CRAWL-050"
kind = "Annahme"
scope = "missing_in_os"
key_prefix = "{BASE}/go"

[[deviation]]
id = "DEV-3"
spec = "CRAWL-051"
kind = "Paritätsabweichung"
scope = "extra_in_os"
key = "{BASE}/new"

[[deviation]]
id = "DEV-4"
spec = "X-1"
kind = "Annahme"
column = "Status Code"
note = "wird nie ausgelöst"
"#,
    )
    .unwrap();
    let items = devs.for_export(Some("parity-onpage"), "internal_all");
    let r = report_with(&m, &sf, &os, &items);
    assert!(r.missing_in_os.is_empty());
    assert!(r.extra_in_os.is_empty());
    assert!(r.url_set_identical);
    assert_eq!(r.cells_compared, 27, "entschuldigte Zelle zählt nicht mit");
    assert_eq!(r.cells_equal, 27);
    assert_eq!(r.agreement_pct, 100.0);
    let ids: Vec<_> = r.applied_deviations.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["DEV-1", "DEV-2", "DEV-3"], "nur ausgelöste Einträge");
    assert_eq!(r.applied_deviations[0].count, 1);
    assert_eq!(r.applied_deviations[0].spec, "ONPAGE-004");
    assert_eq!(r.applied_deviations[1].keys, ["{BASE}/gone"]);
}

#[test]
fn thresholds_decide_pass_or_fail() {
    let (m, sf, os) = scenario();
    let mut r = report_with(&m, &sf, &os, &[]);
    evaluate(&mut r, &Thresholds::builtin());
    assert!(!r.passed);
    assert_eq!(r.threshold_pct, 99.0);
    assert!(r.failures.iter().any(|f| f.contains("Zellübereinstimmung")));
    assert!(r.failures.iter().any(|f| f.contains("URL-Menge")));

    // Niedrigere Zellschwelle und URL-Menge nicht gefordert -> bestanden.
    let t =
        Thresholds::parse("url_set_identical = false\n[agreement_pct]\n\"internal_*\" = 96.0\n")
            .unwrap();
    let mut r = report_with(&m, &sf, &os, &[]);
    evaluate(&mut r, &t);
    assert!(r.passed, "{:?}", r.failures);
    assert_eq!(r.threshold_pct, 96.0);

    // Genau an der Schwelle zählt als erfüllt.
    let t = Thresholds::parse(
        "url_set_identical = false\n[agreement_pct]\n\"internal_*\" = 96.42857142857143\n",
    )
    .unwrap();
    let mut r = report_with(&m, &sf, &os, &[]);
    evaluate(&mut r, &t);
    assert!(r.passed);
}

#[test]
fn missing_os_column_counts_as_empty_values() {
    let mapping = Mapping::parse(
        "key = [\"Address\"]\n[columns]\n\"Title 1\" = \"Title 1\"\n\"Meta 1\" = \"Meta 1\"\n\"Nur SF\" = \"Nur SF\"\n",
    )
    .unwrap();
    let sf = parse_csv(b"Address,Title 1,Meta 1\nhttp://127.0.0.1:1/,T,\n", &[]).unwrap();
    let os = parse_csv(b"Address,Title 1\nhttp://127.0.0.1:1/,T\n", &[]).unwrap();
    let r = compare(&CompareInput {
        export: "x",
        site: None,
        sf: &sf,
        os: &os,
        mapping: &mapping,
        sf_bases: &BaseUrls::from_base("http://127.0.0.1:1"),
        os_bases: &BaseUrls::from_base("http://127.0.0.1:1"),
        deviations: &[],
    })
    .unwrap();
    assert_eq!(r.missing_os_columns, ["Meta 1"]);
    assert_eq!(r.missing_sf_columns, ["Nur SF"]);
    assert_eq!(r.cells_compared, 2);
    assert_eq!(r.cells_equal, 2, "leer gegen fehlende Spalte ist gleich");
}

#[test]
fn missing_key_column_and_empty_files_are_errors() {
    let mapping = Mapping::parse("key = [\"Address\"]\n").unwrap();
    let ok = parse_csv(b"Address\nx\n", &[]).unwrap();
    let no_key = parse_csv(b"Url\nx\n", &[]).unwrap();
    let empty = Table::default();
    let b = BaseUrls::from_base("http://127.0.0.1:1");
    let run = |sf: &Table, os: &Table| {
        compare(&CompareInput {
            export: "x",
            site: None,
            sf,
            os,
            mapping: &mapping,
            sf_bases: &b,
            os_bases: &b,
            deviations: &[],
        })
    };
    assert!(run(&no_key, &ok).is_err());
    assert!(run(&ok, &no_key).is_err());
    assert!(run(&empty, &ok).is_err());
    assert!(run(&ok, &empty).is_err());
}

#[test]
fn identical_files_give_full_agreement_and_pass() {
    let mapping = Mapping::parse("key = [\"Address\"]\n[columns]\n\"A\" = \"A\"\n").unwrap();
    let t = parse_csv(b"Address,A\nhttp://127.0.0.1:1/x,1\n", &[]).unwrap();
    let b = BaseUrls::from_base("http://127.0.0.1:1");
    let mut r = compare(&CompareInput {
        export: "crawl_overview",
        site: None,
        sf: &t,
        os: &t,
        mapping: &mapping,
        sf_bases: &b,
        os_bases: &b,
        deviations: &[],
    })
    .unwrap();
    evaluate(&mut r, &Thresholds::builtin());
    assert_eq!(r.agreement_pct, 100.0);
    assert_eq!(
        r.threshold_pct, 100.0,
        "Crawl-Overview-Zähler müssen identisch sein"
    );
    assert!(r.passed);
}

#[test]
fn example_report_matches_golden() {
    let (m, sf, os) = scenario();
    let mut r = report_with(&m, &sf, &os, &[]);
    evaluate(&mut r, &Thresholds::builtin());
    let json = serde_json::to_string_pretty(&r).unwrap() + "\n";
    let md = render_markdown(&r);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/golden");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("example-report.json"), &json).unwrap();
        std::fs::write(dir.join("example-report.md"), &md).unwrap();
    }
    assert_eq!(
        json,
        std::fs::read_to_string(dir.join("example-report.json")).unwrap(),
        "report.json weicht vom Golden ab (UPDATE_GOLDEN=1 zum Erneuern, Änderung begründen)"
    );
    assert_eq!(
        md,
        std::fs::read_to_string(dir.join("example-report.md")).unwrap(),
        "report.md weicht vom Golden ab"
    );
}

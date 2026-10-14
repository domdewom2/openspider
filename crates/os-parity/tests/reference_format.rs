//! Anpassungen an das reale Referenzformat (K-53, M1-P29): Blockformat des Crawl Overview, Dezimalkomma, nicht
//! geschriebene nummerierte Spalten, zusammengefasste Referenzzeilen, Zusatzregeln der Zellen, erweiterte
//! Abweichungen und der Beleg `verified`. Alle Beispiele sind selbst erfunden (keine Zellinhalte der Referenz).

use std::collections::BTreeMap;

use os_parity::csvio::{parse_blocks, parse_csv, read_reference, Table};
use os_parity::deviations::{Deviation, Deviations, Scope};
use os_parity::diff::{compare, is_numbered_column, CompareInput};
use os_parity::mapping::{Mapping, RefFormat};
use os_parity::normalize::{normalize_decimal_comma, xpath_tag_sequence, BaseUrls};

const OVERVIEW: &str = concat!(
    "\u{feff}\"Site Crawled\",\"http://127.0.0.1:8900/\"\n",
    "\"Start Date\",\"2026-10-14\"\n",
    "\"\"\n",
    "\"Summary\",\"URLs\",\"% of Total\",\"Total URLs\",\"Total URLs Description\"\n",
    "\"Total URLs Encountered\",\"20\",\"100,00%\",\"20\",\"geheime Beschreibung\"\n",
    "\"\"\n",
    "\"URLs Displayed\",\"20\",\"100,00%\",\"20\",\"geheime Beschreibung\"\n",
    "\"\"\n",
    "\"Internal\"\n",
    "\"All\",\"18\",\"100,00%\",\"18\",\"geheime Beschreibung\"\n",
    "\"HTML\",\"12\",\"66,67%\",\"18\",\"geheime Beschreibung\"\n",
    "\"\"\n",
    "\"\"\n",
    "\"Depth (Clicks from Start URL)\"\n",
    "\"0\",\"1\",\"8,33%\",\"12\",\"geheime Beschreibung\"\n",
    "\"\"\n",
    "\"Response Time (Seconds)\",\"URLs\",\"% of Total\"\n",
    "\"1\",\"44\",\"100,00%\"\n",
);

fn section_map() -> BTreeMap<String, String> {
    BTreeMap::from([(
        "Depth (Clicks from Start URL)".to_string(),
        "Crawl Depth".to_string(),
    )])
}

#[test]
fn the_block_format_becomes_a_table_without_descriptions() {
    let t = parse_blocks(OVERVIEW.as_bytes(), &section_map()).unwrap();
    assert_eq!(t.columns, ["Section", "Item", "URLs", "% of Total"]);
    let rows: Vec<String> = t.rows.iter().map(|r| r.join("|")).collect();
    assert_eq!(
        rows,
        [
            "Summary|Total URLs Encountered|20|100,00%",
            // Eintrag ohne Kopf nach einer Leerzeile setzt den Abschnitt fort.
            "Summary|URLs Displayed|20|100,00%",
            "Internal|All|18|100,00%",
            "Internal|HTML|12|66,67%",
            "Crawl Depth|0|1|8,33%",
            "Response Time (Seconds)|1|44|100,00%",
        ]
    );
    assert!(!t.rows.iter().flatten().any(|c| c.contains("geheim")));
    assert!(
        !t.rows.iter().any(|r| r[0] == "Site Crawled"),
        "die Kopfzeilen vor der ersten Leerzeile sind keine Zähler"
    );
    assert!(parse_blocks(b"", &BTreeMap::new()).is_err());
}

#[test]
fn a_mapping_with_the_blocks_format_reads_the_reference_through_read_reference() {
    let m = Mapping::parse(
        "format = \"blocks\"\nkey = [\"Section\", \"Item\"]\n[blocks]\nsection_map = { \"Depth (Clicks from Start URL)\" = \"Crawl Depth\" }\n[columns]\n\"Section\" = \"Section\"\n\"Item\" = \"Item\"\n\"URLs\" = \"URLs\"\n",
    )
    .unwrap();
    assert_eq!(m.format, RefFormat::Blocks);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crawl_overview.csv");
    std::fs::write(&path, OVERVIEW).unwrap();
    let t = read_reference(&path, &m).unwrap();
    assert_eq!(t.rows.len(), 6);
    assert!(Mapping::parse("format = \"quatsch\"\nkey = [\"A\"]\n").is_err());
}

#[test]
fn decimal_comma_and_percent_signs_are_normalized() {
    assert_eq!(normalize_decimal_comma("91,67%"), "91.67");
    assert_eq!(normalize_decimal_comma("20,768"), "20.768");
    assert_eq!(normalize_decimal_comma("-1,5"), "-1.5");
    assert_eq!(normalize_decimal_comma("100%"), "100");
    // Text mit Komma, leere und gemischte Werte bleiben unverändert.
    assert_eq!(normalize_decimal_comma("a,b"), "a,b");
    assert_eq!(normalize_decimal_comma("1,2,3"), "1,2,3");
    assert_eq!(normalize_decimal_comma(""), "");
    assert_eq!(normalize_decimal_comma("Titel 3,5"), "Titel 3,5");
}

#[test]
fn xpath_sequences_ignore_indexes_and_the_html_root() {
    assert_eq!(xpath_tag_sequence("//body/nav/a[2]"), ["body", "nav", "a"]);
    assert_eq!(
        xpath_tag_sequence("/html[1]/body[1]/nav[1]/a[2]"),
        ["body", "nav", "a"]
    );
    assert_ne!(
        xpath_tag_sequence("//head/link"),
        xpath_tag_sequence("//body/link")
    );
}

fn bases() -> (BaseUrls, BaseUrls) {
    (
        BaseUrls::from_base("http://127.0.0.1:8902/"),
        BaseUrls::from_base("http://127.0.0.1:41000"),
    )
}

fn compare_with(
    mapping: &str,
    sf: &str,
    os: &str,
    devs: &[Deviation],
) -> os_parity::report::Report {
    let m = Mapping::parse(mapping).unwrap();
    let sf = parse_csv(sf.as_bytes(), &m.drop_sf_columns).unwrap();
    let os = parse_csv(os.as_bytes(), &[]).unwrap();
    let (sf_bases, os_bases) = bases();
    compare(&CompareInput {
        export: "demo",
        site: Some("s"),
        sf: &sf,
        os: &os,
        mapping: &m,
        sf_bases: &sf_bases,
        os_bases: &os_bases,
        deviations: devs,
    })
    .unwrap()
}

#[test]
fn numbered_columns_the_reference_does_not_write_count_as_empty() {
    assert!(is_numbered_column("Title 2"));
    assert!(is_numbered_column("H1-2 Length"));
    assert!(is_numbered_column("HTML hreflang 3 URL"));
    assert!(!is_numbered_column("Title"));
    assert!(!is_numbered_column("Status Code"));
    let mapping = "key = [\"Address\"]\n[columns]\n\"Address\" = \"Address\"\n\"Title 1\" = \"Title 1\"\n\"Title 2\" = \"Title 2\"\n\"Sitemaps\" = \"Sitemaps\"\n";
    // Die Referenz hat weder `Title 2` noch `Sitemaps`; OpenSpider liefert beide.
    let sf = "Address,Title 1\nhttp://127.0.0.1:8902/a,Eins\nhttp://127.0.0.1:8902/b,Zwei\n";
    let os = "Address,Title 1,Title 2,Sitemaps\nhttp://127.0.0.1:41000/a,Eins,,3\nhttp://127.0.0.1:41000/b,Zwei,Extra,3\n";
    let r = compare_with(mapping, sf, os, &[]);
    let title2 = r.columns.iter().find(|c| c.sf_column == "Title 2").unwrap();
    assert_eq!(
        (title2.compared, title2.errors),
        (2, 1),
        "leer = leer, Extra != leer"
    );
    assert_eq!(title2.examples[0].os, "Extra");
    // Eine nicht nummerierte fehlende Spalte wird nicht verglichen, sondern gemeldet.
    assert!(r.columns.iter().all(|c| c.sf_column != "Sitemaps"));
    assert_eq!(r.missing_sf_columns, ["Sitemaps"]);
}

#[test]
fn reference_rows_with_the_same_key_collapse_when_dedupe_is_on() {
    let sf = "Address,Source,Final Status Code\nhttp://127.0.0.1:8902/r,http://127.0.0.1:8902/,200\nhttp://127.0.0.1:8902/r,http://127.0.0.1:8902/x,200\n";
    let os = "Address,Final Status Code\nhttp://127.0.0.1:41000/r,200\n";
    let columns = "[columns]\n\"Address\" = \"Address\"\n\"Source\" = { ignore = true, reason = \"Quelle\" }\n\"Final Status Code\" = \"Final Status Code\"\n";
    let on = compare_with(
        &format!("key = [\"Address\"]\ndedupe_reference = true\n{columns}"),
        sf,
        os,
        &[],
    );
    assert!(on.url_set_identical, "{:?}", on.missing_in_os);
    assert_eq!(on.matched, 1);
    let off = compare_with(&format!("key = [\"Address\"]\n{columns}"), sf, os, &[]);
    assert!(!off.url_set_identical, "ohne dedupe fehlt die zweite Zeile");
}

#[test]
fn cell_transforms_cover_case_zero_hex_and_paths() {
    let mapping = "key = [\"Address\"]\n[columns]\n\"Address\" = \"Address\"\n\"Status\" = { os = \"Status\", transform = [\"ignore_case\"] }\n\"Len\" = { os = \"Len\", transform = [\"zero_as_empty\"] }\n\"Hash\" = { os = \"Hash\", transform = [\"hex_trim_zeros\"] }\n\"Path\" = { os = \"Path\", transform = [\"xpath_tags\"] }\n\"Share\" = { os = \"Share\", transform = [\"decimal_comma\"] }\n";
    let sf = "Address,Status,Len,Hash,Path,Share\nhttp://127.0.0.1:8902/a,Blocked by robots.txt,0,886e,//body/nav/a[1],\"91,670\"\n";
    let os = "Address,Status,Len,Hash,Path,Share\nhttp://127.0.0.1:41000/a,Blocked by Robots.txt,,0886e,/html[1]/body[1]/nav[1]/a[1],91.67\n";
    let r = compare_with(mapping, sf, os, &[]);
    assert_eq!((r.cells_compared, r.cells_equal), (5, 5), "{r:?}");
    assert!(Mapping::parse(
        "key = [\"A\"]\n[columns]\n\"A\" = { os = \"A\", transform = [\"unbekannt\"] }\n"
    )
    .is_err());
}

#[test]
fn keys_can_be_compared_case_insensitively() {
    let mapping = "key = [\"Section\", \"Item\"]\n[columns]\n\"Section\" = { os = \"Section\", transform = [\"ignore_case\"] }\n\"Item\" = { os = \"Item\", transform = [\"ignore_case\"] }\n\"URLs\" = \"URLs\"\n";
    let sf = "Section,Item,URLs\nResponse Codes,Success (2xx),4\n";
    let os = "Section,Item,URLs\nResponse Codes,Success (2XX),4\n";
    let r = compare_with(mapping, sf, os, &[]);
    assert!(
        r.url_set_identical,
        "{:?} {:?}",
        r.missing_in_os, r.extra_in_os
    );
    assert_eq!(r.cells_equal, 1);
}

#[test]
fn the_overview_compares_only_m1_filters_and_tolerates_os_only_rows() {
    let dir = tempfile::tempdir().unwrap();
    let plan = dir.path().join("docs/plan");
    let maps = dir.path().join("tests/parity/mappings");
    std::fs::create_dir_all(&plan).unwrap();
    std::fs::create_dir_all(&maps).unwrap();
    std::fs::write(
        plan.join("M1-filters.toml"),
        "[[tab]]\nid = \"Internal\"\nfilters = [ { id = \"All\" }, { id = \"HTML\" } ]\n",
    )
    .unwrap();
    std::fs::write(
        maps.join("crawl_overview.toml"),
        "format = \"blocks\"\nkey = [\"Section\", \"Item\"]\n[blocks]\nm1_filters = \"docs/plan/M1-filters.toml\"\nos_only_rows_ok = true\n[columns]\n\"Section\" = { os = \"Section\", transform = [\"ignore_case\"] }\n\"Item\" = { os = \"Item\", transform = [\"ignore_case\"] }\n\"URLs\" = \"URLs\"\n",
    )
    .unwrap();
    let m = Mapping::load(&maps.join("crawl_overview.toml")).unwrap();
    assert!(m
        .blocks
        .m1_filters
        .as_ref()
        .unwrap()
        .contains(&("internal".to_string(), "html".to_string())));
    let sf = parse_blocks(OVERVIEW.as_bytes(), &BTreeMap::new()).unwrap();
    // OpenSpider kennt Internal/All, aber nicht Internal/HTML (M1-Filter fehlt) und hat eine eigene Programmzeile.
    let os = parse_csv(
        b"Section,Item,URLs,% of Total\nInternal,All,18,100.00\nSummary,OpenSpider Version,0.1.0,\n",
        &[],
    )
    .unwrap();
    let (sf_bases, os_bases) = bases();
    let r = compare(&CompareInput {
        export: "crawl_overview",
        site: None,
        sf: &sf,
        os: &os,
        mapping: &m,
        sf_bases: &sf_bases,
        os_bases: &os_bases,
        deviations: &[],
    })
    .unwrap();
    // Internal/HTML ist ein M1-Filter und fehlt in OpenSpider → Fehler; die übrigen Referenzzeilen (Summary,
    // Response Time ...) sind kein M1-Filter und werden nur gezählt.
    assert_eq!(r.missing_in_os, ["internal | html"]);
    assert!(r
        .ignored_sf_rows
        .iter()
        .any(|k| k == "summary | total urls encountered"));
    assert_eq!(r.os_only_rows, ["summary | openspider version"]);
    assert!(r.extra_in_os.is_empty());
}

fn deviation(toml_text: &str) -> Deviation {
    Deviations::parse(&format!(
        "[[deviation]]\nspec=\"A-1\"\nkind=\"Annahme\"\n{toml_text}"
    ))
    .unwrap()
    .items
    .remove(0)
}

#[test]
fn deviations_support_deltas_column_lists_prefix_lists_and_export_lists() {
    let d = deviation(
        "column = \"URLs|Items\"\ndelta = 1\nkey_prefixes = [\"internal | \", \"summary | \"]\n",
    );
    assert!(d.excuses_cell("internal | all", "URLs", "18", "19"));
    assert!(d.excuses_cell("summary | total", "Items", "5", "6"));
    assert!(
        !d.excuses_cell("internal | all", "URLs", "18", "20"),
        "Differenz 2 ≠ 1"
    );
    assert!(!d.excuses_cell("internal | all", "Other", "18", "19"));
    assert!(!d.excuses_cell("security | all", "URLs", "18", "19"));
    assert!(
        !d.excuses_cell("internal | all", "URLs", "x", "19"),
        "nicht numerisch"
    );

    let n = deviation("delta = -1\n");
    assert!(n.excuses_cell("k", "c", "85", "84"));

    let e = deviation("export = \"all_inlinks|all_error_inlinks\"\nsite = \"a|b\"\n");
    assert!(e.applies_to(Some("a"), "all_error_inlinks"));
    assert!(e.applies_to(Some("b"), "all_inlinks"));
    assert!(!e.applies_to(Some("c"), "all_inlinks"));
    assert!(!e.applies_to(Some("a"), "internal_all"));
    let rows = deviation("scope = \"extra_in_os\"\nkey_prefixes = [\"{BASE}/robots\"]\n");
    assert!(rows.excuses_row(Scope::ExtraInOs, "{BASE}/robots.txt"));
}

#[test]
fn applied_deviations_are_reported_and_free_restrictions_are_kept_apart() {
    let mapping =
        "key = [\"Address\"]\n[columns]\n\"Address\" = \"Address\"\n\"URLs\" = \"URLs\"\n";
    let devs = Deviations::parse(
        "[[deviation]]\nid=\"DEV-001\"\nspec=\"CRAWL-098\"\nkind=\"OS-Festlegung\"\nscope=\"extra_in_os\"\nkey_prefix=\"{BASE}/robots\"\n[[deviation]]\nid=\"SF-FREE-01\"\nspec=\"TECH-210\"\nkind=\"Annahme\"\ncolumn=\"URLs\"\nreason=\"SF-Free-Einschränkung (Spider-Konfiguration gesperrt): ohne Sitemap\"\n",
    )
    .unwrap();
    let sf = "Address,URLs\nhttp://127.0.0.1:8902/a,5\n";
    let os = "Address,URLs\nhttp://127.0.0.1:41000/a,0\nhttp://127.0.0.1:41000/robots.txt,1\n";
    let r = compare_with(mapping, sf, os, &devs.items);
    assert!(r.url_set_identical);
    assert_eq!(r.cells_compared, 0, "entschuldigte Zellen zählen nicht");
    let ids: Vec<&str> = r.applied_deviations.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["DEV-001", "SF-FREE-01"]);
    let md = os_parity::report::render_markdown(&r);
    let free_at = md
        .find("## Referenzeinschränkungen SF Free")
        .expect("eigener Abschnitt");
    let plain_at = md
        .find("## Angewandte Abweichungen")
        .expect("begründete Abweichungen");
    assert!(plain_at < free_at);
    let free_part = &md[free_at..];
    assert!(free_part.contains("SF-FREE-01") && !free_part.contains("DEV-001"));
    assert!(md[plain_at..free_at].contains("DEV-001"));
}

#[test]
fn an_export_with_a_skip_reason_is_reported_as_out_of_scope() {
    use os_parity::manifest::Manifest;
    let m = Manifest::parse(
        "site=\"a\"\n[[export]]\nsf_file=\"x.csv\"\nos=\"report:Redirects:Redirects to Errors\"\nmapping=\"m\"\nstatus=\"present\"\nskip=\"OUT-084 ist P1\"\n",
    )
    .unwrap();
    assert_eq!(m.export[0].skip.as_deref(), Some("OUT-084 ist P1"));
}

#[test]
fn table_default_format_is_unchanged() {
    let m = Mapping::parse("key = [\"A\"]\n").unwrap();
    assert_eq!(m.format, RefFormat::Table);
    assert!(!m.dedupe_reference);
    let t: Table = parse_csv(b"A\n1\n", &[]).unwrap();
    assert_eq!(t.rows.len(), 1);
}

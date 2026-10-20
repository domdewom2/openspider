//! Lädt die echten Eingabedateien des Repositories (Besitz P10/Planer) mit den Parsern von `os-parity`:
//! `docs/plan/M1-packages.json`, `tests/parity/{deviations,thresholds}.toml`, alle Mappings unter
//! `tests/parity/mappings` und alle `manifest.toml` unter `tests/golden/parity`. Die Dateien sind
//! maßgeblich; bei Abweichungen wird der Parser angepasst, nicht die Datei.

use std::path::{Path, PathBuf};

use os_parity::coverage::load_packages;
use os_parity::deviations::Deviations;
use os_parity::manifest::{Manifest, OsExport};
use os_parity::mapping::Mapping;
use os_parity::thresholds::Thresholds;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn toml_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    v.sort();
    v
}

#[test]
fn real_packages_file_loads_with_spec_and_extra_ids() {
    let p = load_packages(&repo().join("docs/plan/M1-packages.json")).unwrap();
    assert!(p.specs.len() > 200, "{} IDs", p.specs.len());
    for id in ["CRAWL-128", "TECH-008", "CRAWL-075", "TECH-030", "OUT-084"] {
        assert!(p.extra.contains_key(id), "{id} fehlt in extra_spec_ids");
        assert!(
            !p.specs.contains_key(id),
            "{id} darf nicht in extra und specs stehen"
        );
    }
    assert!(
        p.extra["TECH-030"].iter().any(|x| x == "M1-P06"),
        "{:?}",
        p.extra["TECH-030"]
    );
}

#[test]
fn real_deviations_file_loads_without_warnings() {
    let d = Deviations::load(&repo().join("tests/parity/deviations.toml")).unwrap();
    assert!(d.warnings.is_empty(), "{:#?}", d.warnings);
}

/// Das Schema steht als Kommentarblock in der Datei (die Liste selbst ist bis zum ersten echten Vergleich
/// leer): der auskommentierte Beispieleintrag muss mit dem Parser ohne Warnung und mit allen Feldern lesbar
/// sein.
#[test]
fn deviations_schema_example_from_the_real_file_parses() {
    let text = std::fs::read_to_string(repo().join("tests/parity/deviations.toml")).unwrap();
    let mut lines = text.lines().skip_while(|l| l.trim() != "# [[deviation]]");
    let example: Vec<String> = lines
        .by_ref()
        .take_while(|l| l.starts_with("# "))
        .map(|l| l[2..].to_string())
        .collect();
    assert!(
        example.len() >= 8,
        "Schema-Block nicht gefunden: {example:?}"
    );
    let d = Deviations::parse(&example.join("\n")).unwrap();
    assert_eq!(d.items.len(), 1);
    assert!(d.warnings.is_empty(), "{:#?}", d.warnings);
    let item = &d.items[0];
    assert_eq!(item.id, "DEV-001");
    assert_eq!(item.spec, "ONPAGE-081");
    assert_eq!(item.kind, "Annahme");
    assert_eq!(item.export.as_deref(), Some("internal_all"));
    assert_eq!(item.site.as_deref(), Some("*"));
    assert_eq!(item.column.as_deref(), Some("Title 1 Pixel Width"));
    assert_eq!(item.key.as_deref(), Some("{BASE}/x.html"));
    assert!(
        !item.note.is_empty(),
        "`reason` wird als Begründung gelesen"
    );
    assert!(item.excuses_cell("{BASE}/x.html", "Title 1 Pixel Width", "1", "2"));
    assert_eq!(d.for_export(Some("parity-onpage"), "internal_all").len(), 1);
}

#[test]
fn real_thresholds_file_loads_and_covers_the_a3_exports() {
    let t = Thresholds::load(&repo().join("tests/parity/thresholds.toml")).unwrap();
    assert_eq!(t.min_sites_present, 4);
    assert_eq!(t.min_legacy_sites_present, 4);
    assert_eq!(t.min_m2_sites_present, 4);
    for name in ["redirects_to_errors", "all_error_inlinks"] {
        assert!(t.agreement_pct.contains_key(name), "{name}");
        assert_eq!(t.agreement_for(name), 97.0, "{name}");
    }
    assert!(t.agreement_pct.contains_key("crawl_overview"));
    assert_eq!(t.agreement_for("crawl_overview"), 100.0);
    assert_eq!(t.agreement_for("internal_all"), 99.0);
    assert_eq!(t.agreement_for("redirect_chains"), 97.0);
    // Nur berichtet, keine Schwelle in der Datei: Standardwert.
    assert_eq!(t.agreement_for("sitemaps_all"), t.default_agreement_pct);
}

#[test]
fn real_mappings_and_manifests_load_and_reference_each_other() {
    let root = repo();
    let mappings = toml_files(&root.join("tests/parity/mappings"));
    assert!(mappings.len() >= 20, "{} Mappings", mappings.len());
    let mut names = Vec::new();
    for m in &mappings {
        Mapping::load(m).unwrap_or_else(|e| panic!("{}: {e}", m.display()));
        names.push(m.file_stem().unwrap().to_string_lossy().into_owned());
    }
    // Das Overview-Mapping löst M1-/M2-Platzhalter mit CrawlConfig-Defaults auf.
    let filters = Mapping::load(&root.join("tests/parity/mappings/crawl_overview.toml"))
        .unwrap()
        .blocks
        .m1_filters
        .unwrap();
    let sites_dir = root.join("tests/golden/parity");
    let mut sites = 0;
    for entry in std::fs::read_dir(&sites_dir).unwrap() {
        let dir = entry.unwrap().path();
        let manifest = dir.join("manifest.toml");
        if !manifest.is_file() {
            continue;
        }
        sites += 1;
        let m = Manifest::load(&manifest).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            m.site,
            dir.file_name().unwrap().to_string_lossy(),
            "{}",
            manifest.display()
        );
        assert!(!m.export.is_empty(), "{}", manifest.display());
        for e in &m.export {
            if let OsExport::Tab { tab, filter } = OsExport::parse(&e.os)
                .unwrap_or_else(|err| panic!("{}: {:?}: {err}", manifest.display(), e.os))
            {
                assert!(
                    filters.contains(&(tab.to_lowercase(), filter.to_lowercase())),
                    "{}: unbekannter Filter {:?}",
                    manifest.display(),
                    e.os
                );
            }
            assert!(
                names.contains(&e.mapping),
                "{}: Mapping {:?} existiert nicht",
                manifest.display(),
                e.mapping
            );
        }
    }
    assert_eq!(sites, 10, "erwartet: fünf M1- und fünf M2-Sites");
}

// spec: OUT-080
#[test]
fn overview_includes_m2_default_labels_and_contract_columns_are_not_ignored() {
    let root = repo();
    let overview = Mapping::load(&root.join("tests/parity/mappings/crawl_overview.toml")).unwrap();
    let filters = overview.blocks.m1_filters.unwrap();
    for (tab, filter) in [
        ("page titles", "over 561 pixels"),
        ("meta description", "below 400 pixels"),
        ("meta keywords", "multiple"),
        ("links", "pages with high internal outlinks"),
        ("validation", "html document over 2mb"),
    ] {
        assert!(
            filters.contains(&(tab.to_string(), filter.to_string())),
            "{tab}:{filter}"
        );
    }
    for (name, columns) in [
        (
            "directives_all",
            vec!["Indexability", "Indexability Status"],
        ),
        ("hreflang_all", vec!["Indexability", "Indexability Status"]),
        ("links_all", vec!["Inlinks", "Link Score"]),
    ] {
        let mapping =
            Mapping::load(&root.join(format!("tests/parity/mappings/{name}.toml"))).unwrap();
        for column in columns {
            assert!(!mapping.column(column).unwrap().ignore, "{name}:{column}");
        }
    }
}

// spec: OUT-015
// spec: OUT-023
// spec: OUT-133
#[test]
fn new_manifests_preserve_list_source_sitemap_variants_and_crawl_path_target() {
    let root = repo();
    let listed =
        Manifest::load(&root.join("tests/golden/parity/parity-m2-list/manifest.toml")).unwrap();
    assert_eq!(listed.list_source.as_deref(), Some("list.txt"));
    let mapping = Mapping::load(&root.join("tests/parity/mappings/list_internal.toml")).unwrap();
    assert!(mapping.ordered && mapping.key.iter().any(|k| k == "#"));
    let links =
        Manifest::load(&root.join("tests/golden/parity/parity-m2-links/manifest.toml")).unwrap();
    let path = links
        .export
        .iter()
        .find(|e| e.ref_file == "crawl_path_deep8.csv")
        .unwrap();
    assert_eq!(path.select["Target Address"], "/deep/8.html");
    let onpage =
        Manifest::load(&root.join("tests/golden/parity/parity-m2-onpage/manifest.toml")).unwrap();
    let xml: Vec<_> = onpage
        .export
        .iter()
        .filter(|e| matches!(OsExport::parse(&e.os).unwrap(), OsExport::Sitemap(_)))
        .collect();
    assert_eq!(xml.len(), 3);
    assert!(xml.iter().any(|e| e
        .config
        .get("sitemap_gen.changefreq.basis")
        .is_some_and(|s| s == "depth")));
    for e in onpage
        .export
        .iter()
        .chain(links.export.iter())
        .chain(listed.export.iter())
    {
        assert_eq!(e.status, os_parity::manifest::ExportStatus::Pending);
    }
}

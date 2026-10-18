//! All HTTP references from HTTPS pages, including external targets (OUT-097).
use crate::catalog::{TableEntry, TableJob};
use crate::error::ExportError;
use crate::types::TableWriter;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Insecure Content",
    aliases: &[],
    file_stem: "Insecure Content",
    header: crate::catalog::LINK_INTERNAL_HEADER,
    header_fn: None,
    requires: &[],
    owner: "M2-D11",
    run,
}];

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    crate::catalog::bulk_security::stream_references(job, writer,
        "s.is_internal=1 AND s.in_tables=1 AND s.url LIKE 'https://%' AND COALESCE(t.url,l.resolved,l.target_raw) LIKE 'http://%'", true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Format;
    use os_engine::testing::crawl_fixture_from;
    use os_model::CrawlConfig;

    // spec: OUT-097
    #[tokio::test]
    async fn report_includes_external_unfetched_targets_and_all_reference_types() {
        let mut cfg = CrawlConfig::default();
        cfg.spider.external.crawl = false;
        let fx = crawl_fixture_from("onpage-m2-security", "https://site.test/", cfg.clone()).await;
        let tabs = os_analyze::tabs::all();
        let entry = &entries()[0];
        let job = TableJob {
            entry_name: entry.name,
            tabs: &tabs,
            file: &fx.file,
            cfg: &cfg,
        };
        let path = fx.dir.path().join("insecure.csv");
        crate::catalog::export_table(entry, &job, &path, Format::Csv).unwrap();
        let mut csv = csv::Reader::from_path(&path).unwrap();
        assert_eq!(
            csv.headers().unwrap().iter().collect::<Vec<_>>(),
            crate::catalog::LINK_INTERNAL_HEADER
        );
        let rows: Vec<_> = csv
            .records()
            .map(Result::unwrap)
            .filter(|r| r[1].ends_with("report.html"))
            .collect();
        assert_eq!(rows.len(), 7);
        for (typ, dest, internal) in [
            ("Hyperlink", "http://site.test/insecure", "True"),
            ("Hyperlink", "http://social.test/profile", "False"),
            ("Image", "http://cdn.test/p.png", "False"),
            ("JavaScript", "http://cdn.test/a.js", "False"),
            ("Canonical", "http://site.test/canonical", "True"),
            ("Rel Next", "http://site.test/next", "True"),
            ("Hreflang", "http://site.test/de", "True"),
        ] {
            let row = rows.iter().find(|r| r[0] == *typ && r[2] == *dest).unwrap();
            assert_eq!(&row[15], internal);
            if internal == "False" {
                assert_eq!(&row[6], "");
            }
        }
        let fx = crawl_fixture_from("onpage-m2-security", "http://site.test/", cfg.clone()).await;
        let job = TableJob {
            entry_name: entry.name,
            tabs: &tabs,
            file: &fx.file,
            cfg: &cfg,
        };
        let path = fx.dir.path().join("http.csv");
        assert_eq!(
            crate::catalog::export_table(entry, &job, &path, Format::Csv)
                .unwrap()
                .rows,
            0
        );
    }
}

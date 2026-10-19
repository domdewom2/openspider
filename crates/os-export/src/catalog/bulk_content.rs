//! Content: Duplikat-, Ähnlichkeits- und Rechtschreib-Exporte (OUT-052).
//!
//! Duplikatpaare, Soft-404-Inlinks und gespeicherte Sprachbefunde werden gestreamt; die Sprachprüfung liefert M2-D19.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::TableEntry;
use crate::{error::ExportError, types::TableWriter};
use os_analyze::ColumnKind;
use os_model::Value;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Content:Exact Duplicates Pairs",
        aliases: &["Content:Exact Duplicates"],
        file_stem: "Content:Exact Duplicates Pairs",
        header: &["Address", "Exact Duplicate Address", "Hash"],
        header_fn: None,
        requires: &[],
        owner: "M2-D02",
        run: exact_pairs,
    },
    TableEntry {
        name: "Content:Near Duplicates Pairs",
        aliases: &["Content:Near Duplicates"],
        file_stem: "Content:Near Duplicates Pairs",
        header: &["Address", "Near Duplicate Address", "Similarity"],
        header_fn: None,
        requires: &[ReportRequirement::CrawlAnalysis],
        owner: "M2-D02",
        run: near_pairs,
    },
    TableEntry {
        name: "Content:Spelling Errors Details",
        aliases: &["Content:Spelling Errors"],
        file_stem: "Content:Spelling Errors",
        header: &[
            "Address",
            "Language",
            "Type",
            "Error",
            "Detail",
            "Suggestion",
            "Page Section",
            "Rule ID",
        ],
        requires: &[ReportRequirement::Config {
            key: "spelling.enabled",
            what: "spell checking",
        }],
        owner: "M2-D02",
        run: language_findings,
        header_fn: None,
    },
    TableEntry {
        name: "Content:Grammar Errors Details",
        aliases: &["Content:Grammar Errors"],
        file_stem: "Content:Grammar Errors Details",
        header: &[
            "Address",
            "Language",
            "Type",
            "Error",
            "Detail",
            "Suggestion",
            "Page Section",
            "Rule ID",
        ],
        requires: &[ReportRequirement::Config {
            key: "grammar.enabled",
            what: "grammar checking",
        }],
        owner: "M2-D02",
        run: language_findings,
        header_fn: None,
    },
    TableEntry {
        name: "Content:Spelling & Grammar Errors",
        aliases: &[],
        file_stem: "Content:Spelling & Grammar Errors",
        header: &[
            "Address",
            "Language",
            "Type",
            "Error",
            "Detail",
            "Suggestion",
            "Page Section",
            "Rule ID",
        ],
        requires: &[],
        owner: "M2-D02",
        run: language_findings,
        header_fn: None,
    },
    TableEntry {
        name: "Content:Spelling & Grammar Errors Summary",
        aliases: &[],
        file_stem: "Content:Spelling & Grammar Errors Summary",
        header: &["Error", "Type", "URLs", "Instances"],
        header_fn: None,
        requires: &[],
        owner: "M2-D02",
        run: language_findings,
    },
    TableEntry {
        name: "Content:Soft 404 Inlinks",
        aliases: &[],
        file_stem: "Content:Soft 404 Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D02",
        run: soft_inlinks,
    },
];

fn exact_pairs(
    job: &super::TableJob<'_>,
    writer: &mut dyn TableWriter,
) -> Result<u64, ExportError> {
    if !job.cfg.extraction.hash || !job.cfg.duplicates.exact_enabled {
        return Ok(0);
    }
    pairs(job,writer,"SELECT a.address,b.address,a.hash_md5 FROM urls a JOIN urls b ON a.hash_md5=b.hash_md5 AND a.id<>b.id WHERE a.hash_md5<>'' AND a.is_internal=1 AND b.is_internal=1 AND a.in_tables=1 AND b.in_tables=1 AND a.content_class='HTML' AND b.content_class='HTML' AND a.status_code BETWEEN 200 AND 299 AND b.status_code BETWEEN 200 AND 299 AND (?1=0 OR (COALESCE(a.indexability,'')<>'Non-Indexable' AND COALESCE(b.indexability,'')<>'Non-Indexable')) ORDER BY a.address COLLATE BINARY,b.address COLLATE BINARY",false)
}
fn near_pairs(job: &super::TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    if os_analyze::views::state_of(job.file, job.cfg, "Content", "Near Duplicates")?
        != os_model::FilterState::Ready
    {
        return Ok(0);
    }
    pairs(job,writer,"SELECT a.address,b.address,CAST(ROUND(n.similarity*100) AS INTEGER) FROM near_duplicates n JOIN urls a ON a.id=n.url_id JOIN urls b ON b.id=n.other_id WHERE (?1=0 OR (COALESCE(a.indexability,'')<>'Non-Indexable' AND COALESCE(b.indexability,'')<>'Non-Indexable')) ORDER BY a.address COLLATE BINARY,b.address COLLATE BINARY",true)
}
fn pairs(
    job: &super::TableJob<'_>,
    writer: &mut dyn TableWriter,
    sql: &str,
    near: bool,
) -> Result<u64, ExportError> {
    let mut stmt = crate::links_export::sql(job.file.conn().prepare(sql))?;
    let mut rows = crate::links_export::sql(stmt.query([job.cfg.duplicates.only_indexable]))?;
    let kinds = [
        ColumnKind::Url,
        ColumnKind::Url,
        if near {
            ColumnKind::Int
        } else {
            ColumnKind::Hash
        },
    ];
    let mut count = 0;
    while let Some(row) = crate::links_export::sql(rows.next())? {
        let third = if near {
            Value::Int(crate::links_export::sql(row.get(2))?)
        } else {
            Value::Text(crate::links_export::sql(row.get(2))?)
        };
        writer.row(
            &kinds,
            &[
                Value::Text(crate::links_export::sql(row.get(0))?),
                Value::Text(crate::links_export::sql(row.get(1))?),
                third,
            ],
        )?;
        count += 1;
    }
    Ok(count)
}
fn soft_inlinks(
    job: &super::TableJob<'_>,
    writer: &mut dyn TableWriter,
) -> Result<u64, ExportError> {
    let predicate = os_analyze::tabs::content::SOFT404;
    let where_sql=format!("l.target_id IN (SELECT u.id FROM urls u WHERE u.is_internal=1 AND u.in_tables=1 AND {predicate} AND (:ignore_non_indexable=0 OR COALESCE(u.indexability,'')<>'Non-Indexable'))");
    let phrases = job
        .cfg
        .content
        .soft404_phrases
        .iter()
        .map(|s| {
            s.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        })
        .collect::<Vec<_>>();
    crate::links_export::stream_links(
        job.file,
        &where_sql,
        &[
            (
                ":soft404_phrases".into(),
                Value::Text(serde_json::to_string(&phrases).expect("phrases")),
            ),
            (
                ":ignore_non_indexable".into(),
                Value::Bool(job.cfg.issues.ignore_non_indexable),
            ),
        ],
        crate::links_export::LinkOrder::Inlinks,
        &crate::links_export::link_kinds(),
        writer,
    )
}

/// Gemeinsamer Vertrag der Befunde: M2-D19 schreibt, der Katalog exportiert.
fn language_findings(
    job: &super::TableJob<'_>,
    writer: &mut dyn TableWriter,
) -> Result<u64, ExportError> {
    let name = job.entry_name.to_ascii_lowercase();
    let summary = name.contains("summary");
    let spelling = job.cfg.spelling.enabled && !name.contains("content:grammar");
    let grammar = job.cfg.grammar.enabled && !name.contains("content:spelling errors");
    let select = if summary {
        "SELECT f.text,f.kind,COUNT(DISTINCT f.url_id),COUNT(*) FROM spelling_findings f JOIN urls u ON u.id=f.url_id WHERE u.is_internal=1 AND u.in_tables=1 AND u.status_code BETWEEN 200 AND 299 AND (u.content_class='HTML' OR (u.content_class='PDF' AND ?3)) AND ((LOWER(f.kind)='spelling' AND ?1) OR (LOWER(f.kind)='grammar' AND ?2)) GROUP BY f.text,f.kind ORDER BY COUNT(DISTINCT f.url_id) DESC,f.text COLLATE BINARY,f.kind LIMIT 100"
    } else {
        "SELECT u.address,f.language,f.kind,f.text,f.detail,f.suggestion,f.section,f.rule_id FROM spelling_findings f JOIN urls u ON u.id=f.url_id WHERE u.is_internal=1 AND u.in_tables=1 AND u.status_code BETWEEN 200 AND 299 AND (u.content_class='HTML' OR (u.content_class='PDF' AND ?3)) AND ((LOWER(f.kind)='spelling' AND ?1) OR (LOWER(f.kind)='grammar' AND ?2)) ORDER BY u.address COLLATE BINARY,f.id"
    };
    let mut stmt = crate::links_export::sql(job.file.conn().prepare(select))?;
    let mut rows = crate::links_export::sql(stmt.query((spelling, grammar, job.cfg.pdf.store)))?;
    let columns = if summary { 4 } else { 8 };
    let kinds = (0..columns)
        .map(|i| {
            if summary && i >= 2 {
                ColumnKind::Int
            } else {
                ColumnKind::Text
            }
        })
        .collect::<Vec<_>>();
    let mut count = 0;
    while let Some(row) = crate::links_export::sql(rows.next())? {
        let values = (0..columns)
            .map(|i| {
                if summary && i >= 2 {
                    crate::links_export::sql(row.get::<_, i64>(i)).map(Value::Int)
                } else {
                    crate::links_export::sql(row.get::<_, Option<String>>(i))
                        .map(|v| v.map_or(Value::Null, Value::Text))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        writer.row(&kinds, &values)?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    // spec: OUT-052
    // spec: ONPAGE-165
    #[tokio::test]
    async fn content_exports_stream_symmetric_pairs_and_soft404_link_instances() {
        let mut cfg = os_model::CrawlConfig::default();
        cfg.duplicates.near_enabled = true;
        cfg.pdf.store = true;
        let fx = os_engine::testing::crawl_fixture("onpage-m2-content", cfg).await;
        let cfg = fx.file.config().unwrap();
        let cancel = os_model::CancelToken::new();
        os_analyze::analysis::steps::near_duplicates::run(&fx.file, &cfg, &cancel).unwrap();
        os_analyze::analysis::persist::write_step(
            &fx.file,
            os_model::AnalysisStep::NearDuplicates,
            os_model::StepState::Done,
            None,
            Some(&os_analyze::state::step_config_hash(
                os_model::AnalysisStep::NearDuplicates,
                &cfg,
            )),
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        for (name, expected_rows, golden) in [
            ("Content:Exact Duplicates", 2, "exact-pairs.csv"),
            ("Content:Near Duplicates", 6, "near-pairs.csv"),
            ("Content:Soft 404 Inlinks", 1, "soft404-inlinks.csv"),
        ] {
            let files =
                crate::export_bulk(&fx.file, &cfg, name, dir.path(), crate::Format::Csv).unwrap();
            let text = std::fs::read_to_string(&files[0].path).unwrap();
            assert_eq!(text.lines().count(), expected_rows + 1, "{name}: {text}");
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/onpage-m2-content/golden/bulk")
                .join(golden);
            let normalized = os_testsite::golden::normalize(&text, &[&fx.site.base_url()]);
            os_testsite::golden::assert_golden(path, &normalized);
            if name.contains("Near") {
                assert!(text.contains("near-a.html"));
                assert!(text.contains("near-b.html"));
            }
            if name.contains("Soft") {
                assert!(text.contains("area.html"));
            }
        }
    }
    // spec: OUT-052
    // spec: ONPAGE-165
    #[tokio::test]
    async fn language_summary_ranks_urls_caps_one_hundred_and_filters_disabled_checks() {
        let fx = os_engine::testing::crawl_fixture(
            "onpage-m2-content",
            os_model::CrawlConfig::default(),
        )
        .await;
        let mut cfg = fx.file.config().unwrap();
        cfg.spelling.enabled = true;
        let ids: Vec<i64> = fx.file.conn().prepare("SELECT id FROM urls WHERE content_class='HTML' AND status_code=200 ORDER BY id LIMIT 2").unwrap()
            .query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
        for n in 0..150 {
            fx.file.conn().execute("INSERT INTO spelling_findings(url_id,kind,language,section,text) VALUES (?1,'Spelling','en-GB','Body',?2)", (ids[0], format!("error{n:03}"))).unwrap();
        }
        for id in [ids[0], ids[0], ids[1]] {
            fx.file.conn().execute("INSERT INTO spelling_findings(url_id,kind,language,section,text) VALUES (?1,'Spelling','en-GB','Body','shared')", [id]).unwrap();
        }
        fx.file.conn().execute("INSERT INTO spelling_findings(url_id,kind,language,section,text) VALUES (?1,'Grammar','en-GB','Body','disabled')", [ids[0]]).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let files = crate::export_bulk(
            &fx.file,
            &cfg,
            "Content:Spelling & Grammar Errors Summary",
            dir.path(),
            crate::Format::Csv,
        )
        .unwrap();
        let csv = std::fs::read_to_string(&files[0].path).unwrap();
        let lines = csv.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 101);
        assert_eq!(lines[1], "shared,Spelling,2,3");
        assert_eq!(lines[2], "error000,Spelling,1,1");
        assert!(!csv.contains("disabled"));
    }
}

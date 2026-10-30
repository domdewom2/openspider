//! Images: Exporte je Filter (OUT-053).
use crate::catalog::reports::ReportRequirement;
use crate::catalog::TableEntry;
use os_analyze::tabs::images::{self, ImageFilter};

/// Numerische Menü-/CLI-Namen zusätzlich zu den stabilen X-Vertragsnamen (OUT-053).
pub fn dynamic_entries(cfg: &os_model::CrawlConfig) -> Vec<super::DynamicEntry> {
    [(3, cfg.image.alt_max_chars), (4, cfg.image.max_kb)]
        .into_iter()
        .map(|(index, value)| {
            let entry = &ENTRIES[index];
            let name = entry.name.replace(" X ", &format!(" {value} "));
            let runner = entry.run;
            super::DynamicEntry {
                file_stem: name.clone(),
                name,
                label_key: None,
                header: crate::catalog::IMAGE_HEADER
                    .iter()
                    .map(|s| (*s).into())
                    .collect(),
                run: Box::new(runner),
            }
        })
        .collect()
}

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Images:All Image Inlinks",
        aliases: &[],
        file_stem: "Images:All Image Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D03",
        run: all,
    },
    TableEntry {
        name: "Images:Images Missing Alt Text Inlinks",
        aliases: &[],
        file_stem: "Images:Images Missing Alt Text Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D03",
        run: missing_alt_text,
    },
    TableEntry {
        name: "Images:Images Missing Alt Attribute Inlinks",
        aliases: &[],
        file_stem: "Images:Images Missing Alt Attribute Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D03",
        run: missing_alt_attribute,
    },
    TableEntry {
        name: "Images:Images With Alt Text Over X Characters Inlinks",
        aliases: &[],
        file_stem: "Images:Images With Alt Text Over X Characters Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D03",
        run: long_alt,
    },
    TableEntry {
        name: "Images:Images Over X KB Inlinks",
        aliases: &[],
        file_stem: "Images:Images Over X KB Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D03",
        run: over_size,
    },
    TableEntry {
        name: "Images:Images Missing Size Attributes Inlinks",
        aliases: &[],
        file_stem: "Images:Images Missing Size Attributes Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D03",
        run: missing_size,
    },
    TableEntry {
        name: "Images:Incorrectly Sized Images Inlinks",
        aliases: &[],
        file_stem: "Images:Incorrectly Sized Images Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-D03",
        run: incorrect_size,
    },
    TableEntry {
        name: "Images:Background Images Inlinks",
        aliases: &["Images:Background Images"],
        file_stem: "Images:Background Images Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-D03",
        run: background,
    },
];

macro_rules! runners {
    ($($name:ident => $filter:ident),* $(,)?) => {$(
        fn $name(job: &crate::catalog::TableJob<'_>, writer: &mut dyn crate::types::TableWriter) -> Result<u64,crate::ExportError> {
            run(job,writer,ImageFilter::$filter)
        }
    )*};
}
runners! { all=>All, missing_alt_text=>MissingAltText, missing_alt_attribute=>MissingAltAttribute,
long_alt=>LongAlt, over_size=>OverSize, missing_size=>MissingSize, incorrect_size=>IncorrectSize, background=>Background }

fn run(
    job: &crate::catalog::TableJob<'_>,
    writer: &mut dyn crate::types::TableWriter,
    filter: ImageFilter,
) -> Result<u64, crate::ExportError> {
    let state = os_analyze::views::state_of(job.file, job.cfg, "Images", filter.tab_filter())?;
    if !state.yields_rows() {
        tracing::warn!(
            "{} requires Images:{} ({state:?})",
            job.entry_name,
            filter.tab_filter()
        );
        return Ok(0);
    }
    let kinds: Vec<_> = images::INSTANCE_COLUMNS[..12]
        .iter()
        .map(|(_, k)| *k)
        .collect();
    images::stream_instances(job.file, job.cfg, filter, |row| {
        writer.row(&kinds, &row[..12])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        catalog::{self, ExportKind, TableJob},
        Format,
    };
    use os_model::{
        time::{IdSource, SeededIds},
        AnalysisStep, CrawlConfig, CrawlMode, RenderingMode, StepState, Timestamp,
    };
    use os_store::{CrawlFile, CrawlInit, StorageProfile};
    fn setup() -> (tempfile::TempDir, CrawlFile, CrawlConfig) {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = CrawlConfig::default();
        cfg.rendering.mode = RenderingMode::JavaScript;
        let at = Timestamp::from_db("2026-10-18T00:00:00.000Z").unwrap();
        let file = CrawlFile::create(
            &dir.path().join("images.ospider"),
            &CrawlInit {
                crawl_id: SeededIds::new(3).new_crawl_id(at),
                mode: CrawlMode::Spider,
                start_url: Some("https://site.test/".into()),
                list_source: None,
                project: "Images".into(),
                name: "Fixture".into(),
                config: cfg.clone(),
                created_at: at,
            },
            StorageProfile::Balanced,
        )
        .unwrap();
        file.conn()
            .execute_batch(include_str!(
                "../../../../tests/fixtures/onpage-m2-images/analysis/facts.sql"
            ))
            .unwrap();
        os_analyze::analysis::steps::images::run(&file, &cfg, &os_model::CancelToken::new())
            .unwrap();
        os_analyze::analysis::persist::write_step(
            &file,
            AnalysisStep::Images,
            StepState::Done,
            None,
            None,
        )
        .unwrap();
        (dir, file, cfg)
    }
    // spec: ONPAGE-181
    // spec: OUT-053
    #[test]
    fn every_export_selects_only_matching_instances_in_csv_and_xlsx() {
        use calamine::{Reader, Xlsx};
        let (dir, file, cfg) = setup();
        let tabs = os_analyze::tabs::all();
        for (entry, count) in entries().iter().zip([18, 1, 1, 1, 3, 3, 6, 2]) {
            let job = TableJob {
                entry_name: entry.name,
                tabs: &tabs,
                file: &file,
                cfg: &cfg,
            };
            for format in [Format::Csv, Format::Xlsx] {
                let path = dir.path().join(entry.file_name(if format == Format::Csv {
                    "csv"
                } else {
                    "xlsx"
                }));
                let info = catalog::export_table(entry, &job, &path, format).unwrap();
                assert_eq!(info.rows, count, "{}", entry.name);
                if format == Format::Csv {
                    let text = std::fs::read_to_string(path).unwrap();
                    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
                    let mut reader = csv::Reader::from_reader(text.as_bytes());
                    assert_eq!(
                        reader.headers().unwrap().iter().collect::<Vec<_>>(),
                        catalog::IMAGE_HEADER
                    );
                    assert_eq!(reader.records().count() as u64, count);
                    if entry.name == "Images:Images Missing Alt Text Inlinks" {
                        assert_eq!(text,include_str!("../../../../tests/fixtures/onpage-m2-images/export/missing-alt-text.csv"));
                    }
                } else {
                    let mut workbook: Xlsx<_> = calamine::open_workbook(path).unwrap();
                    let range = workbook.worksheet_range_at(0).unwrap().unwrap();
                    assert_eq!(range.height() as u64, count + 1);
                    assert_eq!(range.width(), 12);
                }
            }
        }
    }
    // spec: OUT-053
    #[test]
    fn numeric_and_template_names_resolve_with_configured_thresholds() {
        let mut cfg = CrawlConfig::default();
        cfg.image.max_kb = 200;
        cfg.image.alt_max_chars = 150;
        for (template, numeric) in [
            (
                "Images:Images Over X KB Inlinks",
                "Images:Images Over 200 KB Inlinks",
            ),
            (
                "Images:Images With Alt Text Over X Characters Inlinks",
                "Images:Images With Alt Text Over 150 Characters Inlinks",
            ),
        ] {
            assert!(catalog::resolve_with(ExportKind::Bulk, template, &cfg).is_ok());
            assert!(catalog::resolve_with(ExportKind::Bulk, numeric, &cfg).is_ok());
            assert!(catalog::list_with(ExportKind::Bulk, &cfg).contains(&numeric.to_string()));
        }
        let (dir, file, cfg) = setup();
        for (template, numeric, count) in [
            (
                "Images:Images Over X KB Inlinks",
                "Images:Images Over 100 KB Inlinks",
                3,
            ),
            (
                "Images:Images With Alt Text Over X Characters Inlinks",
                "Images:Images With Alt Text Over 100 Characters Inlinks",
                1,
            ),
        ] {
            let templated = crate::export_bulk(
                &file,
                &cfg,
                template,
                &dir.path().join("template"),
                Format::Csv,
            )
            .unwrap();
            let numbered = crate::export_bulk(
                &file,
                &cfg,
                numeric,
                &dir.path().join("numeric"),
                Format::Csv,
            )
            .unwrap();
            assert_eq!(numbered[0].rows, count);
            assert_eq!(
                std::fs::read(&templated[0].path).unwrap(),
                std::fs::read(&numbered[0].path).unwrap()
            );
        }
        assert_eq!(
            catalog::resolve(ExportKind::Bulk, "Images:Background Images").unwrap(),
            catalog::resolve(ExportKind::Bulk, "Images:Background Images Inlinks").unwrap()
        );
    }
    // spec: OUT-053
    // spec: OUT-112
    #[test]
    fn post_analysis_exports_report_missing_prerequisites_and_inline_backgrounds_work() {
        use calamine::{Reader, Xlsx};
        let (dir, file, mut cfg) = setup();
        let tabs = os_analyze::tabs::all();
        os_analyze::analysis::persist::write_step(
            &file,
            AnalysisStep::Images,
            StepState::Pending,
            None,
            None,
        )
        .unwrap();
        for mode in [RenderingMode::Text, RenderingMode::JavaScript] {
            cfg.rendering.mode = mode;
            for entry in &entries()[5..] {
                let job = TableJob {
                    entry_name: entry.name,
                    tabs: &tabs,
                    file: &file,
                    cfg: &cfg,
                };
                for format in [Format::Csv, Format::Xlsx] {
                    let path = dir.path().join(entry.file_name(if format == Format::Csv {
                        "csv"
                    } else {
                        "xlsx"
                    }));
                    let info = catalog::export_table(entry, &job, &path, format).unwrap();
                    let expected_rows =
                        if entry.name == "Images:Images Missing Size Attributes Inlinks" {
                            3
                        } else {
                            0
                        };
                    assert_eq!(info.rows, expected_rows);
                    if mode == RenderingMode::Text
                        && entry.requires.contains(&ReportRequirement::Rendering)
                    {
                        assert_eq!(
                            info.warnings,
                            [format!(
                                "report '{}' requires JavaScript rendering",
                                entry.name
                            )]
                        );
                    }
                    if format == Format::Csv {
                        let text = std::fs::read_to_string(path).unwrap();
                        let mut reader = csv::Reader::from_reader(
                            text.trim_start_matches('\u{feff}').as_bytes(),
                        );
                        assert_eq!(
                            reader.headers().unwrap().iter().collect::<Vec<_>>(),
                            catalog::IMAGE_HEADER
                        );
                        assert_eq!(reader.records().count() as u64, expected_rows);
                    } else {
                        let mut workbook: Xlsx<_> = calamine::open_workbook(path).unwrap();
                        let range = workbook.worksheet_range_at(0).unwrap().unwrap();
                        assert_eq!(range.height() as u64, expected_rows + 1);
                        assert_eq!(range.width(), 12);
                    }
                }
            }
        }
        cfg.rendering.mode = RenderingMode::Text;
        cfg.images.parse_inline_css_backgrounds = true;
        os_analyze::analysis::persist::write_step(
            &file,
            AnalysisStep::Images,
            StepState::Done,
            None,
            None,
        )
        .unwrap();
        let entry = &entries()[7];
        let job = TableJob {
            entry_name: entry.name,
            tabs: &tabs,
            file: &file,
            cfg: &cfg,
        };
        let info =
            catalog::export_table(entry, &job, &dir.path().join("background.csv"), Format::Csv)
                .unwrap();
        assert_eq!(info.rows, 2);
    }
    // spec: ONPAGE-181
    #[test]
    fn bulk_stream_is_not_limited_to_a_detail_page() {
        let (dir, file, cfg) = setup();
        file.conn().execute_batch("WITH RECURSIVE rows(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM rows WHERE n<600)
            INSERT INTO image_instances(page_id,image_id,src_raw,source,alt_present,alt,alt_len,position)
            SELECT 1,10,'mixed.jpg','img',1,'',0,n FROM rows;").unwrap();
        let entry = &entries()[1];
        let tabs = os_analyze::tabs::all();
        let job = TableJob {
            entry_name: entry.name,
            tabs: &tabs,
            file: &file,
            cfg: &cfg,
        };
        let info =
            catalog::export_table(entry, &job, &dir.path().join("many.csv"), Format::Csv).unwrap();
        assert_eq!(info.rows, 601);
    }
}

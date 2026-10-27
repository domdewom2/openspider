//! Local screenshot files and their URL index (OUT-043).
use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::{ExportError, ExportFileInfo, TableWriter};
use os_analyze::defs::ColumnKind;
use os_model::{CrawlConfig, RenderingMode, Value};
use os_store::CrawlFile;
use std::path::Path;

pub const NAME: &str = "Web:Screenshots";
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}
const ENTRIES: &[TableEntry] = &[TableEntry {
    name: NAME,
    aliases: &[],
    file_stem: NAME,
    header: &["Address", "File"],
    header_fn: None,
    requires: &[
        ReportRequirement::Rendering,
        ReportRequirement::Config {
            key: "rendering.screenshots.enabled",
            what: "saved screenshots",
        },
    ],
    owner: "M2-R02",
    run: index,
}];
fn filename(address: &str) -> String {
    let stem = crate::naming::file_name(address, None, "");
    format!(
        "{}_{}.png",
        crate::naming::cut(&stem, 110),
        crate::fs_paths::short_hash(address)
    )
}
fn index(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    if job.cfg.rendering.mode != RenderingMode::JavaScript || !job.cfg.rendering.screenshots.enabled
    {
        return Ok(0);
    }
    let mut statement=crate::links_export::sql(job.file.conn().prepare("SELECT u.address FROM urls u WHERE EXISTS (SELECT 1 FROM screenshots s WHERE s.url_id=u.id) ORDER BY u.address COLLATE BINARY,u.id"))?;
    let mut rows = crate::links_export::sql(statement.query([]))?;
    let mut count = 0;
    while let Some(row) = crate::links_export::sql(rows.next())? {
        let address: String = crate::links_export::sql(row.get(0))?;
        writer.row(
            &[ColumnKind::Url, ColumnKind::Text],
            &[
                Value::Text(address.clone()),
                Value::Text(filename(&address)),
            ],
        )?;
        count += 1;
    }
    Ok(count)
}
/// Exports stored PNGs without network access or any remote destination.
pub fn export(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    out_dir: &Path,
    ctx: Option<&crate::types::ExportCtx<'_>>,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    let warnings =
        crate::reports::prereq::requirement_warnings(NAME, ENTRIES[0].requires, cfg, &[]);
    let directory = out_dir.join("screenshots");
    if let Some(ctx) = ctx {
        ctx.checkpoint()?;
    }
    if !warnings.is_empty() {
        // Named exports retain the standard header-and-warning contract for CLI/MCP.
        // The interactive catalogue disables this entry when prerequisites are missing.
        return Ok(vec![write_index(file, cfg, &directory)?]);
    }
    let invalid: bool = crate::links_export::sql(file.conn().query_row(
        "SELECT EXISTS(SELECT 1 FROM screenshots WHERE tile<>0 OR substr(png,1,8)<>X'89504E470D0A1A0A')", [], |r|r.get(0)))?;
    if invalid {
        return Err(ExportError::Unsupported("Screenshots require one stored PNG per URL; select PNG in rendering settings before crawling".into()));
    }
    let empty: bool = crate::links_export::sql(file.conn().query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM screenshots)",
        [],
        |r| r.get(0),
    ))?;
    if empty && ctx.is_some_and(|ctx| ctx.skip_empty) {
        return Ok(vec![ExportFileInfo {
            path: directory.join("index.csv"),
            rows: 0,
            bytes: 0,
            warnings: vec![],
        }]);
    }
    std::fs::create_dir_all(&directory).map_err(|source| ExportError::Io {
        path: directory.clone(),
        source,
    })?;
    let mut statement=crate::links_export::sql(file.conn().prepare("SELECT u.address,s.png FROM screenshots s JOIN urls u ON u.id=s.url_id ORDER BY u.address COLLATE BINARY,u.id,s.tile"))?;
    let mut rows = crate::links_export::sql(statement.query([]))?;
    let mut count = 0;
    while let Some(row) = crate::links_export::sql(rows.next())? {
        if let Some(ctx) = ctx {
            ctx.checkpoint()?;
        }
        let address: String = crate::links_export::sql(row.get(0))?;
        let bytes: Vec<u8> = crate::links_export::sql(row.get(1))?;
        let path = directory.join(filename(&address));
        let pending = crate::output::PendingFile::new(&path)?;
        std::fs::write(crate::fs_paths::fs_path(&pending.path), &bytes).map_err(|source| {
            ExportError::Io {
                path: path.clone(),
                source,
            }
        })?;
        pending.publish(ExportFileInfo {
            path,
            rows: 1,
            bytes: bytes.len() as u64,
            warnings: vec![],
        })?;
        count += 1;
        if let Some(ctx) = ctx {
            (ctx.progress)(count);
        }
    }
    let mut info = write_index(file, cfg, &directory)?;
    info.rows = count;
    Ok(vec![info])
}
fn write_index(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    directory: &Path,
) -> Result<ExportFileInfo, ExportError> {
    std::fs::create_dir_all(directory).map_err(|source| ExportError::Io {
        path: directory.into(),
        source,
    })?;
    let tabs = os_analyze::tabs::all();
    let job = TableJob {
        entry_name: NAME,
        tabs: &tabs,
        file,
        cfg,
    };
    crate::catalog::export_table(
        &ENTRIES[0],
        &job,
        &directory.join("index.csv"),
        crate::Format::Csv,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{
        time::{IdSource, SeededIds},
        CrawlMode, Timestamp,
    };
    use os_store::{CrawlInit, StorageProfile};
    fn setup() -> (tempfile::TempDir, CrawlFile, CrawlConfig) {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = CrawlConfig::default();
        cfg.rendering.mode = RenderingMode::JavaScript;
        cfg.rendering.screenshots.enabled = true;
        let at = Timestamp::from_db("2026-10-21T00:00:00.000Z").unwrap();
        let file = CrawlFile::create(
            &dir.path().join("js.ospider"),
            &CrawlInit {
                crawl_id: SeededIds::new(2).new_crawl_id(at),
                mode: CrawlMode::Spider,
                start_url: Some("https://site.test/".into()),
                list_source: None,
                project: "JavaScript".into(),
                name: "Fixture".into(),
                config: cfg.clone(),
                created_at: at,
            },
            StorageProfile::Balanced,
        )
        .unwrap();
        file.conn()
            .execute_batch(include_str!(
                "../../../../tests/fixtures/tech-m2-javascript/facts.sql"
            ))
            .unwrap();
        let png = include_bytes!("../../../../tests/fixtures/tech-m2-javascript/screenshot.png");
        for id in [1, 2, 3] {
            file.conn()
                .execute(
                    "INSERT INTO screenshots(url_id,tile,png,width,height) VALUES (?1,0,?2,32,24)",
                    (id, png.as_slice()),
                )
                .unwrap();
        }
        (dir, file, cfg)
    }
    // spec: OUT-043
    #[test]
    fn bulk_route_writes_one_png_per_url_and_a_deterministic_local_index() {
        let (dir, file, cfg) = setup();
        let files = crate::export_bulk(&file, &cfg, NAME, dir.path(), crate::Format::Xlsx).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].rows, 3);
        assert_eq!(files[0].path, dir.path().join("screenshots/index.csv"));
        let bytes = std::fs::read(&files[0].path).unwrap();
        let mut reader = csv::Reader::from_reader(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap());
        assert_eq!(
            reader.headers().unwrap().iter().collect::<Vec<_>>(),
            ["Address", "File"]
        );
        let rows: Vec<_> = reader.records().map(Result::unwrap).collect();
        assert_eq!(rows.len(), 3);
        for row in rows {
            let name = &row[1];
            assert!(!name.contains(['/', '\\']));
            assert!(name.ends_with(".png"));
            assert_eq!(
                name.rsplit('_')
                    .next()
                    .unwrap()
                    .strip_suffix(".png")
                    .unwrap()
                    .len(),
                8
            );
            assert_eq!(
                std::fs::read(dir.path().join("screenshots").join(name)).unwrap(),
                include_bytes!("../../../../tests/fixtures/tech-m2-javascript/screenshot.png")
            );
        }
        assert_ne!(
            filename("https://site.test/A?b"),
            filename("https://site.test/a/b")
        );
        assert!(filename(&"a".repeat(500)).len() <= 124);
        crate::export_bulk(&file, &cfg, NAME, dir.path(), crate::Format::Csv).unwrap();
        assert_eq!(std::fs::read(&files[0].path).unwrap(), bytes);
    }
    // spec: OUT-043
    #[test]
    fn disabled_capture_warns_without_pngs_and_cancellation_leaves_no_files() {
        let (dir, file, mut cfg) = setup();
        let out = dir.path().join("out");
        cfg.rendering.mode = RenderingMode::Text;
        let result = crate::export_bulk(&file, &cfg, NAME, &out, crate::Format::Csv).unwrap();
        assert_eq!(result[0].rows, 0);
        assert!(!result[0].warnings.is_empty());
        assert!(result[0].path.is_file());
        assert_eq!(
            std::fs::read_dir(out.join("screenshots")).unwrap().count(),
            1
        );
        cfg.rendering.mode = RenderingMode::JavaScript;
        cfg.rendering.screenshots.enabled = false;
        let result = crate::export_bulk(&file, &cfg, NAME, &out, crate::Format::Csv).unwrap();
        assert_eq!(result[0].rows, 0);
        assert!(result[0].path.is_file());
        assert_eq!(
            std::fs::read_dir(out.join("screenshots")).unwrap().count(),
            1
        );
        cfg.rendering.screenshots.enabled = true;
        let out = dir.path().join("cancelled");
        let cancel = os_model::CancelToken::new();
        cancel.cancel();
        let ctx = crate::types::ExportCtx {
            cancel: &cancel,
            progress: &|_| {},
            skip_empty: false,
            workbook: None,
            skipped: vec![],
        };
        assert!(export(&file, &cfg, &out, Some(&ctx)).is_err());
        assert!(!out.exists());
    }

    // spec: OUT-043
    #[test]
    fn batch_routes_files_outside_workbooks_and_can_skip_an_empty_capture() {
        let (dir, file, cfg) = setup();
        let selections = [crate::types::ExportSelection {
            kind: "bulk".into(),
            name: NAME.into(),
        }];
        let cancel = os_model::CancelToken::new();
        let mut ctx = crate::types::ExportCtx {
            cancel: &cancel,
            progress: &|_| {},
            skip_empty: true,
            workbook: None,
            skipped: vec![],
        };
        let options = crate::pipeline::BatchOptions {
            directory: dir.path(),
            format: crate::Format::Xlsx,
            crawl_name: "Fixture",
            single_workbook: true,
        };
        let result =
            crate::pipeline::export_batch(&file, &cfg, &selections, &options, &mut ctx).unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].rows, 3);
        assert!(dir.path().join("screenshots/index.csv").is_file());
        assert!(!dir.path().join("fixture_export.xlsx").exists());
        file.conn().execute("DELETE FROM screenshots", []).unwrap();
        let empty = dir.path().join("empty");
        let options = crate::pipeline::BatchOptions {
            directory: &empty,
            ..options
        };
        let result =
            crate::pipeline::export_batch(&file, &cfg, &selections, &options, &mut ctx).unwrap();
        assert!(result.files.is_empty());
        assert_eq!(result.skipped, [NAME]);
        assert!(!empty.join("screenshots").exists());
    }
    // spec: OUT-043
    #[test]
    fn invalid_capture_is_rejected_before_creating_any_output() {
        let (dir, file, cfg) = setup();
        file.conn()
            .execute("UPDATE screenshots SET png=X'00' WHERE url_id=3", [])
            .unwrap();
        let out = dir.path().join("invalid");
        assert!(crate::export_bulk(&file, &cfg, NAME, &out, crate::Format::Csv).is_err());
        assert!(!out.exists());
    }
}

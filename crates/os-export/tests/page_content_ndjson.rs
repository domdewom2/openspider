//! AGENT-019: streaming content exports against existing clean-room site fixtures.
use os_engine::testing::crawl_fixture_from as crawl_fixture;
use os_export::{catalog, export_bulk, Format};
use os_model::CrawlConfig;
use serde_json::Value;

// spec: AGENT-019
#[tokio::test]
async fn ndjson_has_one_record_per_html_url_and_matches_core_text_extraction() {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.store_html = true;
    let fx = crawl_fixture("out-basic-site", "http://out-basic-site.test/", cfg).await;
    let cfg = fx.file.config().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let info = catalog::page_content::export(&fx.file, &cfg, dir.path(), true).unwrap();
    let bytes = std::fs::read(&info.path).unwrap();
    assert_eq!(bytes.len() as u64, info.bytes);
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.ends_with('\n'));
    assert!(!text.starts_with('\u{feff}'));
    let records: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let count: u64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE content_class='HTML'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(records.len() as u64, count);
    assert_eq!(info.rows, count);
    let home = records
        .iter()
        .find(|r| r["url"] == "http://out-basic-site.test/")
        .unwrap();
    assert_eq!(home["status_code"], 200);
    assert_eq!(home["title"], "Basic Site - Home");
    let html = home["html"].as_str().unwrap();
    let expected =
        os_parse::content::area_text(&os_parse::dom::Document::parse(html), &cfg.content.area);
    assert_eq!(home["text"], expected);
    assert!(expected.contains("Welcome to the basic fixture site"));
    assert!(!expected.contains("<script"));
    assert_eq!(home["source"], "original");
    // The ordinary bulk pipeline resolves the same name and emits NDJSON, not table cells.
    let files = export_bulk(
        &fx.file,
        &cfg,
        catalog::page_content::NAME,
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert!(files[0]
        .warnings
        .iter()
        .any(|w| w.contains("requested csv format is ignored")));
    let xlsx = export_bulk(
        &fx.file,
        &cfg,
        catalog::page_content::NAME,
        dir.path(),
        Format::Xlsx,
    )
    .unwrap();
    assert!(xlsx[0]
        .warnings
        .iter()
        .any(|w| w.contains("requested xlsx format is ignored")));
    let default = std::fs::read_to_string(&files[0].path).unwrap();
    assert!(default.lines().all(|l| serde_json::from_str::<Value>(l)
        .unwrap()
        .get("html")
        .is_none()));
    let again = catalog::page_content::export(&fx.file, &cfg, dir.path(), false).unwrap();
    assert_eq!(std::fs::read_to_string(again.path).unwrap(), default);
    // The app/CLI batch dispatcher must also produce this file rather than trying a table writer.
    let cancel = os_model::CancelToken::new();
    let progress = std::cell::Cell::new(0u64);
    let report = |rows| progress.set(rows);
    let mut ctx = os_export::types::ExportCtx {
        cancel: &cancel,
        progress: &report,
        skip_empty: false,
        workbook: None,
        skipped: vec![],
    };
    let selection = [os_export::types::ExportSelection {
        kind: "bulk".into(),
        name: catalog::page_content::NAME.into(),
    }];
    let options = os_export::pipeline::BatchOptions {
        format: Format::Xlsx,
        directory: dir.path(),
        crawl_name: "fixture",
        single_workbook: true,
    };
    let batch =
        os_export::pipeline::export_batch(&fx.file, &cfg, &selection, &options, &mut ctx).unwrap();
    assert_eq!(batch.files.len(), 1);
    assert!(batch.files[0]
        .warnings
        .iter()
        .any(|w| w.contains("requested xlsx format is ignored")));
    assert!(batch
        .warnings
        .iter()
        .any(|w| w.contains("requested xlsx format is ignored")));
    assert_eq!(batch.files[0].path.extension().unwrap(), "ndjson");
    assert_eq!(
        std::fs::read_to_string(&batch.files[0].path).unwrap(),
        default
    );
    assert_eq!(progress.get(), count);
    cancel.cancel();
    assert!(
        catalog::page_content::export_with(&fx.file, &cfg, dir.path(), false, Some(&ctx)).is_err()
    );
    let mid_cancel = os_model::CancelToken::new();
    let stop_after_first = |_| mid_cancel.cancel();
    let mid_ctx = os_export::types::ExportCtx {
        cancel: &mid_cancel,
        progress: &stop_after_first,
        skip_empty: false,
        workbook: None,
        skipped: vec![],
    };
    assert!(
        catalog::page_content::export_with(&fx.file, &cfg, dir.path(), false, Some(&mid_ctx))
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(&batch.files[0].path).unwrap(),
        default
    );
    assert!(std::fs::read_dir(dir.path()).unwrap().all(|entry| entry
        .unwrap()
        .path()
        .extension()
        .unwrap()
        != "part"));
    fx.file.conn().execute("DELETE FROM urls", []).unwrap();
    let empty_cancel = os_model::CancelToken::new();
    let mut empty_ctx = os_export::types::ExportCtx {
        cancel: &empty_cancel,
        progress: &|_| {},
        skip_empty: true,
        workbook: None,
        skipped: vec![],
    };
    let empty =
        os_export::pipeline::export_batch(&fx.file, &cfg, &selection, &options, &mut empty_ctx)
            .unwrap();
    assert!(empty.files.is_empty());
    assert_eq!(empty.skipped, [catalog::page_content::NAME]);
    assert_eq!(
        std::fs::read_to_string(&batch.files[0].path).unwrap(),
        default
    );
}

// spec: AGENT-019
#[tokio::test]
async fn missing_storage_keeps_html_urls_and_explains_null_content() {
    let fx = crawl_fixture(
        "out-basic-site",
        "http://out-basic-site.test/",
        CrawlConfig::default(),
    )
    .await;
    let cfg = fx.file.config().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let info = catalog::page_content::export(&fx.file, &cfg, dir.path(), false).unwrap();
    assert!(info.rows > 0);
    assert!(info.warnings[0].contains("extraction.store_html"));
    for line in std::fs::read_to_string(info.path).unwrap().lines() {
        let record: Value = serde_json::from_str(line).unwrap();
        assert!(record["text"].is_null());
        assert!(record["message"]
            .as_str()
            .unwrap()
            .contains("HTML speichern"));
    }
}

// spec: AGENT-017
// spec: AGENT-019
#[tokio::test]
async fn saved_rendered_dom_wins_and_empty_html_is_not_missing() {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.store_html = true;
    let fx = crawl_fixture("out-basic-site", "http://out-basic-site.test/", cfg).await;
    let row = os_store::queries::url_by_address(fx.file.conn(), "http://out-basic-site.test/")
        .unwrap()
        .unwrap();
    fx.file
        .conn()
        .execute(
            "INSERT INTO render_pages(url_id,status,html) VALUES(?1,'success',?2)",
            (
                row.id.0,
                b"<html><body>Rendered \"text\"\nsecond line</body></html>".as_slice(),
            ),
        )
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cfg = fx.file.config().unwrap();
    let info = catalog::page_content::export(&fx.file, &cfg, dir.path(), true).unwrap();
    let records: Vec<Value> = std::fs::read_to_string(info.path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let home = records
        .iter()
        .find(|r| r["url"] == "http://out-basic-site.test/")
        .unwrap();
    assert_eq!(home["source"], "rendered");
    assert!(home["text"].as_str().unwrap().contains("Rendered \"text\""));
    fx.file
        .conn()
        .execute(
            "UPDATE render_pages SET html=x'' WHERE url_id=?1",
            [row.id.0],
        )
        .unwrap();
    let saved = catalog::page_content::read(&fx.file, &cfg, row.id)
        .unwrap()
        .unwrap();
    assert_eq!(saved.html, "");
    assert_eq!(saved.text, "");
}

// spec: AGENT-018
#[test]
fn png_preview_preserves_pixels_in_a_bounded_top_left_excerpt() {
    let png = include_bytes!("../../../tests/fixtures/out-basic-site/assets/logo.png");
    let (preview, w, h) = catalog::page_content::screenshot_preview(png, 4, 4096).unwrap();
    assert_eq!((w, h), (4, 4));
    assert_eq!(u32::from_be_bytes(preview[16..20].try_into().unwrap()), 4);
    assert_eq!(u32::from_be_bytes(preview[20..24].try_into().unwrap()), 4);
    assert!(preview.len() <= 4096);
    // Cropping a valid generated PNG again decodes the result and preserves its pixels/bytes.
    assert_eq!(
        catalog::page_content::screenshot_preview(&preview, 4, 4096)
            .unwrap()
            .0,
        preview
    );
    assert!(catalog::page_content::screenshot_preview(&png[..30], 4, 4096).is_none());
    assert!(catalog::page_content::screenshot_preview(png, 4, 10).is_none());
    assert!(catalog::page_content::screenshot_preview(png, 0, 4096).is_none());
}

// spec: AGENT-018
#[test]
fn png_preview_decodes_all_five_scanline_filters_before_cropping() {
    let plain: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 4, 0, 0, 0, 5, 8, 2,
        0, 0, 0, 237, 207, 218, 140, 0, 0, 0, 76, 73, 68, 65, 84, 120, 156, 1, 65, 0, 190, 255, 0,
        0, 63, 126, 31, 94, 157, 62, 125, 188, 93, 156, 219, 0, 11, 74, 137, 42, 105, 168, 73, 136,
        199, 104, 167, 230, 0, 22, 85, 148, 53, 116, 179, 84, 147, 210, 115, 178, 241, 0, 33, 96,
        159, 64, 127, 190, 95, 158, 221, 126, 189, 252, 0, 44, 107, 170, 75, 138, 201, 106, 169,
        232, 137, 200, 7, 128, 248, 29, 211, 124, 233, 52, 186, 0, 0, 0, 0, 73, 69, 78, 68, 174,
        66, 96, 130,
    ];
    let filtered: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 4, 0, 0, 0, 5, 8, 2,
        0, 0, 0, 237, 207, 218, 140, 0, 0, 0, 41, 73, 68, 65, 84, 120, 156, 99, 96, 176, 175, 147,
        143, 155, 107, 87, 187, 39, 118, 206, 109, 70, 110, 175, 78, 121, 24, 96, 226, 70, 2, 204,
        98, 102, 161, 162, 48, 192, 130, 44, 3, 0, 177, 63, 9, 136, 100, 60, 233, 39, 0, 0, 0, 0,
        73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let expected = catalog::page_content::screenshot_preview(plain, 5, 4096).unwrap();
    let actual = catalog::page_content::screenshot_preview(filtered, 5, 4096).unwrap();
    assert_eq!(actual, expected);
    assert_eq!((actual.1, actual.2), (4, 5));
    assert_eq!(
        catalog::page_content::screenshot_preview(plain, 3, 4096),
        catalog::page_content::screenshot_preview(filtered, 3, 4096)
    );
}

// A deterministic RGBA fixture, using uncompressed deflate blocks so the test
// does not need an additional image/codec dependency.
fn rgba_png_fixture(width: u32, height: u32, noisy: bool) -> Vec<u8> {
    fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], bytes: &[u8]) {
        output.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        output.extend_from_slice(kind);
        output.extend_from_slice(bytes);
        let mut crc = u32::MAX;
        for byte in kind.iter().chain(bytes) {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        output.extend_from_slice(&(!crc).to_be_bytes());
    }
    let stride = width as usize * 4 + 1;
    let mut raw = vec![0; stride * height as usize];
    let mut random = 123456789u32;
    for row in raw.chunks_exact_mut(stride) {
        for channel in &mut row[1..] {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            *channel = if noisy { random as u8 } else { 255 };
        }
    }
    let mut zlib = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(bytes) = chunks.next() {
        zlib.push(u8::from(chunks.peek().is_none()));
        let len = bytes.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(bytes);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for byte in raw {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &zlib);
    chunk(&mut png, b"IEND", &[]);
    png
}

// spec: AGENT-018
#[test]
fn wide_rgba_preview_uses_compressed_size_and_remains_decodable_at_the_byte_limit() {
    let limit = 2 * 1024 * 1024;
    let png = rgba_png_fixture(1280, 3000, false);
    let (preview, width, height) =
        catalog::page_content::screenshot_preview(&png, 4096, limit).unwrap();
    assert_eq!((width, height), (1280, 3000));
    assert!(height > 409);
    assert!(preview.len() <= limit);
    assert_eq!(
        catalog::page_content::screenshot_preview(&preview, 4096, limit)
            .unwrap()
            .0,
        preview
    );
    let (_, width, height) = catalog::page_content::screenshot_preview(&png, 1000, limit).unwrap();
    assert_eq!((width, height), (1000, 1000));

    let png = rgba_png_fixture(1280, 600, true);
    let (preview, width, height) =
        catalog::page_content::screenshot_preview(&png, 4096, limit).unwrap();
    assert_eq!(width, 1280);
    assert!(height > 0 && height < 600);
    assert!(preview.len() <= limit);
    assert!(preview.len() > limit - 8192);
    assert_eq!(
        catalog::page_content::screenshot_preview(&preview, 4096, limit)
            .unwrap()
            .0,
        preview
    );
    assert!(catalog::page_content::screenshot_preview(&png, 4096, 4096).is_none());
}

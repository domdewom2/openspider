mod common;

use std::io::Read;
use std::time::Duration;

use common::{dechunk, get, get_at, parse, read_all, request_bytes};
use os_testsite::{fixtures_root, Scenario, TestSite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn site() -> TestSite {
    TestSite::start("testsite-routes").await.unwrap()
}

// spec: TECH-007
#[tokio::test]
async fn reason_phrase() {
    let site = site().await;
    let r = get(&site, "x", "/reason").await;
    assert_eq!((r.status, r.reason.as_str()), (200, "Alles Klar"));
    // Ohne Angabe gilt der Standardtext, auch für 404.
    let r = get(&site, "x", "/nope").await;
    assert_eq!((r.status, r.reason.as_str()), (404, "Not Found"));
}

// spec: TECH-007
#[tokio::test]
async fn repeated_headers_via_arrays() {
    let site = site().await;
    let r = get(&site, "x", "/multi-header").await;
    assert_eq!(
        r.header_all("link"),
        vec![
            "<https://a.example/>; rel=preload",
            "<https://b.example/>; rel=preload"
        ]
    );
    assert_eq!(r.header_all("x-single"), vec!["1"]);
}

// spec: TECH-007
#[tokio::test]
async fn set_cookie_single_and_multiple() {
    let site = site().await;
    let r = get(&site, "x", "/cookies").await;
    assert_eq!(
        r.header_all("set-cookie"),
        vec!["a=1; Path=/", "b=2; Path=/; HttpOnly"]
    );
}

// spec: TECH-007
#[tokio::test]
async fn omits_content_type_on_request() {
    let site = site().await;
    let r = get(&site, "x", "/no-ct").await;
    assert_eq!(r.status, 200);
    assert_eq!(r.header("content-type"), None);
    assert_eq!(r.text(), "ohne Content-Type");
    // Die übrigen Routen behalten ihren Content-Type.
    assert!(get(&site, "x", "/reason")
        .await
        .header("content-type")
        .is_some());
}

// spec: TECH-007
#[tokio::test]
async fn content_encodings() {
    let site = site().await;
    let text = |p: &str| -> Vec<u8> {
        let src =
            std::fs::read_to_string(fixtures_root().join("testsite-routes/scenario.toml")).unwrap();
        let scenario = Scenario::parse(&src).unwrap();
        scenario
            .routes
            .iter()
            .find(|r| r.path == p)
            .and_then(|r| r.body.clone())
            .unwrap()
            .into_bytes()
    };
    let mut out = Vec::new();

    let r = get(&site, "x", "/gzip").await;
    assert_eq!(r.header("content-encoding"), Some("gzip"));
    assert_eq!(
        r.header("content-length"),
        Some(r.body.len().to_string().as_str())
    );
    flate2::read::GzDecoder::new(&r.body[..])
        .read_to_end(&mut out)
        .unwrap();
    assert_eq!(out, text("/gzip"));

    out.clear();
    let r = get(&site, "x", "/deflate").await;
    assert_eq!(r.header("content-encoding"), Some("deflate"));
    flate2::read::ZlibDecoder::new(&r.body[..])
        .read_to_end(&mut out)
        .unwrap();
    assert_eq!(out, text("/deflate"));

    // deflate-raw: derselbe Header, aber ohne zlib-Kopf – ZlibDecoder scheitert, Raw gelingt.
    out.clear();
    let r = get(&site, "x", "/deflate-raw").await;
    assert_eq!(r.header("content-encoding"), Some("deflate"));
    assert!(flate2::read::ZlibDecoder::new(&r.body[..])
        .read_to_end(&mut Vec::new())
        .is_err());
    flate2::read::DeflateDecoder::new(&r.body[..])
        .read_to_end(&mut out)
        .unwrap();
    assert_eq!(out, text("/deflate-raw"));

    out.clear();
    let r = get(&site, "x", "/br").await;
    assert_eq!(r.header("content-encoding"), Some("br"));
    brotli::Decompressor::new(&r.body[..], 4096)
        .read_to_end(&mut out)
        .unwrap();
    assert_eq!(out, text("/br"));

    out.clear();
    let r = get(&site, "x", "/broken-gzip").await;
    assert_eq!(r.header("content-encoding"), Some("gzip"));
    assert_eq!(&r.body[..2], &[0x1f, 0x8b], "gültiger gzip-Kopf");
    assert!(flate2::read::GzDecoder::new(&r.body[..])
        .read_to_end(&mut out)
        .is_err());
}

// spec: TECH-007
#[tokio::test]
async fn chunked_transfer_encoding() {
    let site = site().await;
    let r = get(&site, "x", "/chunked").await;
    assert_eq!(r.header("transfer-encoding"), Some("chunked"));
    assert_eq!(r.header("content-length"), None);
    assert_eq!(r.body.len(), 128);
    assert!(r.text().starts_with("0123456789abcdef"));
    // Der Body kommt in mehreren Stücken, nicht als ein einziger Chunk.
    let pieces = String::from_utf8_lossy(&r.raw_body).matches("\r\n").count();
    assert!(pieces >= 4, "{pieces}");
    assert_eq!(dechunk(&r.raw_body).unwrap(), r.body);
}

// spec: TECH-007
#[tokio::test]
async fn trickle_sends_body_at_the_given_rate() {
    let site = site().await;
    let mut stream = TcpStream::connect(site.addr()).await.unwrap();
    stream
        .write_all(&request_bytes("x", "/trickle", &[]))
        .await
        .unwrap();
    let start = std::time::Instant::now();
    let (raw, err) = read_all(&mut stream).await;
    let elapsed = start.elapsed();
    assert!(err.is_none());
    let r = parse(&raw).unwrap();
    assert_eq!(r.body.len(), 95);
    // 95 Byte bei 100 Byte/s = 10 Stücke à 10 Byte im Abstand von 100 ms: mindestens ~0,9 s.
    assert!(elapsed >= Duration::from_millis(800), "{elapsed:?}");
}

// spec: TECH-007
#[tokio::test]
async fn hang_never_answers() {
    let site = site().await;
    let mut stream = TcpStream::connect(site.addr()).await.unwrap();
    stream
        .write_all(&request_bytes("x", "/hang", &[]))
        .await
        .unwrap();
    let mut buf = [0u8; 64];
    let waited = tokio::time::timeout(Duration::from_millis(400), stream.read(&mut buf)).await;
    assert!(waited.is_err(), "der Server darf nichts senden");
    // Der Request ist angekommen und gilt als in Bearbeitung.
    assert_eq!(site.count("x", "/hang"), 1);
    assert_eq!(site.max_concurrency(), 1);
    // Andere Verbindungen werden weiter bedient.
    assert_eq!(get(&site, "x", "/reason").await.status, 200);
}

// spec: TECH-007
#[tokio::test]
async fn abort_after_headers_closes_without_body() {
    let site = site().await;
    let mut stream = TcpStream::connect(site.addr()).await.unwrap();
    stream
        .write_all(&request_bytes("x", "/abort", &[]))
        .await
        .unwrap();
    let (raw, err) = read_all(&mut stream).await;
    assert!(
        err.is_none(),
        "sauberes Ende der Verbindung erwartet: {err:?}"
    );
    let r = parse(&raw).unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(r.header("content-length"), Some("1000"));
    assert!(r.body.is_empty(), "kein Body trotz Content-Length 1000");
}

// spec: TECH-007
#[tokio::test]
async fn reset_mid_body_cuts_the_body_with_a_reset() {
    let site = site().await;
    let mut stream = TcpStream::connect(site.addr()).await.unwrap();
    stream
        .write_all(&request_bytes("x", "/reset", &[]))
        .await
        .unwrap();
    let (raw, err) = read_all(&mut stream).await;
    // Ein Reset ist ein Fehler, kein sauberes Ende.
    assert!(err.is_some(), "Reset erwartet, Antwort: {} Byte", raw.len());
    if let Some(r) = parse(&raw) {
        assert_eq!(r.header("content-length"), Some("20000"));
        assert!(r.body.len() < 20000);
    }
}

// spec: TECH-007
#[tokio::test]
async fn sequence_returns_each_status_in_turn_then_repeats_the_last() {
    let site = site().await;
    let statuses: Vec<u16> = {
        let mut v = Vec::new();
        for _ in 0..5 {
            v.push(get(&site, "x", "/flaky").await.status);
        }
        v
    };
    assert_eq!(statuses, vec![503, 503, 200, 200, 200]);
    assert_eq!(get(&site, "x", "/flaky").await.text(), "endlich");
}

// spec: TECH-007
#[tokio::test]
async fn fail_first_n_fails_then_recovers() {
    let site = site().await;
    let mut statuses = Vec::new();
    for _ in 0..4 {
        statuses.push(get(&site, "x", "/fail-first").await.status);
    }
    assert_eq!(statuses, vec![502, 502, 200, 200]);
    // Ein frisch gestarteter Server beginnt von vorn.
    let fresh = TestSite::start("testsite-routes").await.unwrap();
    assert_eq!(get(&fresh, "x", "/fail-first").await.status, 502);
}

// spec: TECH-007
#[tokio::test]
async fn echo_returns_request_headers_and_cookies() {
    let site = site().await;
    let r = get_at(
        site.addr(),
        "Echo.test",
        "/echo?q=1",
        &[
            ("Cookie", "sid=abc; theme=dark"),
            ("X-Custom", "a"),
            ("X-Custom", "b"),
        ],
    )
    .await;
    assert_eq!(r.header("content-type"), Some("application/json"));
    let json: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    assert_eq!(json["method"], "GET");
    assert_eq!(json["host"], "echo.test");
    assert_eq!(json["path"], "/echo?q=1");
    assert_eq!(json["headers"]["x-custom"], serde_json::json!(["a", "b"]));
    assert_eq!(
        json["headers"]["cookie"],
        serde_json::json!(["sid=abc; theme=dark"])
    );
    assert_eq!(
        json["cookies"],
        serde_json::json!({"sid": "abc", "theme": "dark"})
    );
    assert_eq!(json["tls"], false);
}

// spec: TECH-007
#[tokio::test]
async fn body_size_is_exact() {
    let site = site().await;
    let r = get(&site, "x", "/sized").await;
    assert_eq!(r.body.len(), 5000);
    assert_eq!(r.header("content-length"), Some("5000"));
    // Auch für Routen mit eigenem Body: kürzen und auffüllen.
    let src = "[[route]]\npath = \"/pad\"\nbody = \"abc\"\nbody_size = 6\n\n[[route]]\npath = \"/cut\"\nbody = \"abcdef\"\nbody_size = 2\n";
    let dir = std::env::temp_dir().join(format!("os-testsite-size-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("scenario.toml"), src).unwrap();
    let s = TestSite::start_dir(&dir, 0).await.unwrap();
    assert_eq!(get(&s, "x", "/pad").await.text(), "abc   ");
    assert_eq!(get(&s, "x", "/cut").await.text(), "ab");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = flate2::Crc::new();
    c.update(data);
    c.sum()
}

// spec: TECH-007
#[tokio::test]
async fn png_size_is_exact_and_valid() {
    let site = site().await;
    let r = get(&site, "x", "/pic.png").await;
    assert_eq!(r.header("content-type"), Some("image/png"));
    let png = &r.body;
    assert_eq!(png.len(), 2048);
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    // Chunks durchlaufen: Längen, CRCs, Reihenfolge IHDR … IEND.
    let mut pos = 8;
    let mut kinds = Vec::new();
    let mut idat = Vec::new();
    while pos < png.len() {
        let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = &png[pos + 4..pos + 8];
        let data = &png[pos + 8..pos + 8 + len];
        let crc = u32::from_be_bytes(png[pos + 8 + len..pos + 12 + len].try_into().unwrap());
        assert_eq!(crc, crc32(&png[pos + 4..pos + 8 + len]), "CRC von {kind:?}");
        kinds.push(String::from_utf8_lossy(kind).into_owned());
        if kind == b"IHDR" {
            assert_eq!(&data[..8], &[0, 0, 0, 1, 0, 0, 0, 1], "1x1 Pixel");
        }
        if kind == b"IDAT" {
            idat = data.to_vec();
        }
        pos += 12 + len;
    }
    assert_eq!(pos, png.len());
    assert_eq!(kinds, vec!["IHDR", "IDAT", "paDd", "IEND"]);
    let mut pixels = Vec::new();
    flate2::read::ZlibDecoder::new(&idat[..])
        .read_to_end(&mut pixels)
        .unwrap();
    assert_eq!(pixels.len(), 2, "Filterbyte + ein Pixel");
}

// spec: TECH-007
#[tokio::test]
async fn requires_chromium_is_parsed_and_ignored_by_the_server() {
    let src =
        std::fs::read_to_string(fixtures_root().join("testsite-routes/scenario.toml")).unwrap();
    let scenario = Scenario::parse(&src).unwrap();
    scenario.validate().unwrap();
    let flagged: Vec<&str> = scenario
        .routes
        .iter()
        .filter(|r| r.requires_chromium)
        .map(|r| r.path.as_str())
        .collect();
    assert_eq!(flagged, vec!["/render-me"]);
    let site = site().await;
    let r = get(&site, "x", "/render-me").await;
    assert_eq!(r.status, 200);
    assert!(r.text().contains("document.title"));
}

// spec: TECH-007
#[tokio::test]
async fn delay_ms_is_unchanged() {
    let site = site().await;
    let start = std::time::Instant::now();
    let r = get(&site, "x", "/slow").await;
    assert_eq!(r.text(), "langsam");
    assert!(start.elapsed() >= Duration::from_millis(280));
}

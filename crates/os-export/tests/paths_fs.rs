//! Tests für `fs_paths` (OUT-268): Unicode-Pfade, Windows-Langpfade, case-insensitive Kollisionen.

use std::fs;
use std::path::PathBuf;

use calamine::{open_workbook, Reader, Xlsx};
use os_analyze::ColumnKind;
use os_export::fs_paths::{
    fs_path, hashed_file_name, is_windows_reserved, short_hash, valid_component,
    windows_extended_path, NameRegistry,
};
use os_export::naming;
use os_export::output::{open_writer, prepare, OutputMode};
use os_export::{Format, WriterOptions};
use os_model::{FixedClock, Timestamp, Value};

fn write_one(path: &std::path::Path, format: Format) {
    let mut w = open_writer(path, format, "Internal", &WriterOptions::default()).unwrap();
    w.header(&["Address".into()]).unwrap();
    w.row(
        &[ColumnKind::Url],
        &[Value::Text("https://example.com/ü".into())],
    )
    .unwrap();
    assert_eq!(w.finish().unwrap().rows, 1);
}

// spec: OUT-268
#[test]
fn unicode_directories_work_for_csv_and_xlsx_output() {
    let tmp = tempfile::tempdir().unwrap();
    for dir in [
        "Jürgen",
        "日本",
        "Ordner mit Leerzeichen",
        "emoji-🕷",
        "e\u{301}cole",
    ] {
        let base = tmp.path().join(dir);
        let clock = FixedClock::new(
            Timestamp::from_db("2026-10-13T07:35:00Z").unwrap(),
            std::time::Duration::ZERO,
        );
        // Ordner anlegen (Overwrite-Modus) und Zeitstempelordner darunter
        let out = prepare(&base, OutputMode::Overwrite, &clock).unwrap();
        let csv = out.join("internal_all.csv");
        let xlsx = out.join("internal_all.xlsx");
        write_one(&csv, Format::Csv);
        write_one(&xlsx, Format::Xlsx);
        assert!(fs::read_to_string(&csv)
            .unwrap()
            .contains("https://example.com/ü"));
        let wb: Xlsx<_> = open_workbook(&xlsx).unwrap();
        assert_eq!(wb.sheet_names(), vec!["Internal"]);

        let stamped = prepare(&base, OutputMode::Timestamped, &clock).unwrap();
        assert!(stamped.starts_with(&base) && stamped.is_dir());
    }
    // Dateien mit Unicode im Namen
    let name = tmp.path().join("Größe_日本.csv");
    write_one(&name, Format::Csv);
    assert!(name.is_file());
}

// spec: OUT-268
#[test]
fn unicode_component_names_are_made_valid_without_losing_characters() {
    assert_eq!(valid_component("Jürgen"), "Jürgen");
    assert_eq!(valid_component("日本/サイト:一覧"), "日本_サイト_一覧");
    assert_eq!(valid_component("a<b>c|d?e*f\"g"), "a_b_c_d_e_f_g");
    assert_eq!(valid_component("ende. . "), "ende");
    assert_eq!(valid_component("  vorn"), "vorn");
    assert_eq!(valid_component("CON"), "CON_");
    assert_eq!(valid_component("aux.txt"), "aux_.txt");
    assert_eq!(valid_component("..."), "_");
    assert!(valid_component(&"ü".repeat(500)).chars().count() <= 200);
    assert!(
        is_windows_reserved("Com3") && !is_windows_reserved("com10") && !is_windows_reserved("co")
    );
}

// spec: OUT-268
#[test]
fn windows_long_paths_get_the_extended_prefix() {
    assert_eq!(
        windows_extended_path(r"C:\Users\Jürgen\out"),
        r"\\?\C:\Users\Jürgen\out"
    );
    assert_eq!(windows_extended_path("D:/a/b/./c/../d"), r"\\?\D:\a\b\d");
    assert_eq!(
        windows_extended_path(r"\\fileserver\share\Ordner\日本"),
        r"\\?\UNC\fileserver\share\Ordner\日本"
    );
    // schon erweitert oder relativ: unverändert
    assert_eq!(windows_extended_path(r"\\?\C:\x\y"), r"\\?\C:\x\y");
    assert_eq!(windows_extended_path(r"relativ\pfad"), r"relativ\pfad");
    assert_eq!(windows_extended_path("relativ/pfad"), "relativ/pfad");

    let long: String = r"abcdefghij\".repeat(40);
    let input = format!(r"C:\{long}datei.csv");
    let out = windows_extended_path(&input);
    assert!(out.starts_with(r"\\?\C:\abcdefghij\") && out.ends_with(r"\datei.csv"));
    assert!(out.len() > 260);
}

// spec: OUT-268
#[test]
fn deep_paths_beyond_260_characters_can_be_written() {
    let tmp = tempfile::tempdir().unwrap();
    let mut dir: PathBuf = tmp.path().to_path_buf();
    // 6 x 50 Zeichen: deutlich über MAX_PATH, jede Komponente aber unter 255 Zeichen
    for i in 0..6 {
        dir.push(format!("{i}-{}", "ä".repeat(48)));
    }
    assert!(dir.to_string_lossy().chars().count() > 260);
    let out = prepare(
        &dir,
        OutputMode::Overwrite,
        &FixedClock::new(Timestamp::UNIX_EPOCH, std::time::Duration::ZERO),
    )
    .unwrap();
    let file = out.join("internal_all.csv");
    write_one(&file, Format::Csv);
    write_one(&out.join("internal_all.xlsx"), Format::Xlsx);
    assert!(fs_path(&file).is_file());
    assert!(fs::read_to_string(fs_path(&file))
        .unwrap()
        .contains("Address"));
}

// spec: OUT-268
#[test]
fn case_insensitive_collisions_get_a_hash_suffix() {
    // Zwei URLs bereinigen sich zu Namen, die sich nur in der Schreibweise unterscheiden.
    let urls = [
        "https://example.com/Page",
        "https://example.com/page",
        "https://example.com/PAGE",
        "https://example.com/other",
    ];
    let mut reg = NameRegistry::new();
    let mut names = Vec::new();
    for u in urls {
        let stem = u.trim_start_matches("https://example.com/");
        names.push(reg.claim(&format!("{stem}.png"), u));
    }
    assert_eq!(names[0], "Page.png", "der erste behält seinen Namen");
    assert_eq!(names[1], format!("page_{}.png", short_hash(urls[1])));
    assert_eq!(names[2], format!("PAGE_{}.png", short_hash(urls[2])));
    assert_eq!(names[3], "other.png");
    // wiederholte Anfrage liefert denselben Namen
    assert_eq!(reg.claim("page.png", urls[1]), names[1]);

    // auf der Platte entstehen auch auf macOS/Windows (case-insensitiv) vier verschiedene Dateien
    let tmp = tempfile::tempdir().unwrap();
    for n in &names {
        fs::write(tmp.path().join(n), n).unwrap();
    }
    assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 4);
    for n in &names {
        assert_eq!(&fs::read_to_string(tmp.path().join(n)).unwrap(), n);
    }

    // gleicher Name und gleicher Schlüssel, aber anderer Besitzer -> Zähler nach dem Hash
    let mut reg = NameRegistry::new();
    assert_eq!(reg.claim("a.png", "x"), "a.png");
    let h = short_hash("y");
    assert_eq!(reg.claim("A.png", "y"), format!("A_{h}.png"));
}

// spec: OUT-268
#[test]
fn file_names_from_urls_are_valid_on_both_file_systems() {
    let urls = [
        "https://example.com/a/b?c=d&e=f#frag",
        "https://example.com/日本語/ページ.html",
        "https://example.com/CON",
        "https://example.com/",
        &format!("https://example.com/{}", "long/".repeat(80)),
    ];
    let mut seen = std::collections::HashSet::new();
    for u in urls {
        let n = hashed_file_name(u, "png");
        assert!(n.ends_with(".png"));
        let stem = n.strip_suffix(".png").unwrap();
        assert!(stem.chars().count() <= naming::MAX_STEM_LEN, "{n}");
        assert!(
            stem.chars()
                .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.')),
            "{n}"
        );
        assert!(stem.ends_with(&short_hash(u)), "{n}");
        assert!(seen.insert(n.to_lowercase()), "Kollision: {n}");
    }
    // zwei URLs, die sich bereinigt gleichen, bleiben durch den Hash unterscheidbar
    assert_ne!(
        hashed_file_name("https://example.com/a?b", "png"),
        hashed_file_name("https://example.com/a/b", "png")
    );
}

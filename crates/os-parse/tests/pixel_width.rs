//! Pixelbreiten-Engine (UI-038, ONPAGE-004): Testvektoren der Spec (kalibrierte Standardprofile 20 px und 14 px),
//! Rundung des Liberation-Sans-Modells, Bold, Fallback, Grapheme sowie ein Abgleich der eingecheckten Tabellen
//! gegen die Schriftdateien.

use os_model::ExtractedText;
use os_parse::pixel::{annotate, width_px};

const TITLE: f64 = 20.0;
const DESC: f64 = 14.0;

fn px(text: &str, size: f64) -> u32 {
    width_px(text, size, false)
}

// spec: UI-038
#[test]
fn title_vectors_20px_regular() {
    // kalibrierte ganzzahlige Zeichenbreiten (K-64): „W“ = 18 px, „i“ = 5 px
    assert_eq!(px("W", TITLE), 18);
    assert_eq!(px("WWWWW", TITLE), 90);
    assert_eq!(px("iiiii", TITLE), 25);
    assert_eq!(px(&"W".repeat(29), TITLE), 522);
    assert_eq!(px(&"W".repeat(30), TITLE), 540);
    // Wiederholungs-Fixtures der Paritäts-Sites: 70 × „i“ = 350 px, 60 × „W“ = 1080 px
    assert_eq!(px(&"i".repeat(70), TITLE), 350);
    assert_eq!(px(&"W".repeat(60), TITLE), 1080);
}

// spec: UI-038
#[test]
fn description_vectors_14px_regular() {
    // kalibrierte ganzzahlige Zeichenbreiten (K-64): „a“ = 8 px, „W“ = 12 px, „i“ = 3 px
    assert_eq!(px(&"a".repeat(100), DESC), 800);
    assert_eq!(px(&"a".repeat(126), DESC), 1008);
    assert_eq!(px(&"a".repeat(127), DESC), 1016);
    assert_eq!(px(&"W".repeat(150), DESC), 1800);
    assert_eq!(px(&"i".repeat(160), DESC), 480);
}

// spec: UI-038
#[test]
fn calibrated_profiles_add_integer_character_widths_without_final_rounding() {
    // Titel 20 px: ganzzahlige Breiten je Zeichen, die Summe wird nie gerundet („ii“ = 10, nicht 9)
    assert_eq!(px("i", TITLE), 5);
    assert_eq!(px("ii", TITLE), 10);
    assert_eq!(px("a b", TITLE), 11 + 5 + 11);
    assert_eq!(
        px("Titel – Seite 1", TITLE),
        px("Titel", TITLE) + 5 + 13 + 5 + px("Seite", TITLE) + 5 + 11
    );
    // Beschreibung 14 px
    assert_eq!(px("i", DESC), 3);
    assert_eq!(px("a b, c.", DESC), 8 + 3 + 8 + 3 + 3 + 7 + 4);
    // nicht gemessene Zeichen: auf ganze Pixel gerundete Liberation-Sans-Breite („ä“ wie „a“, „ß“ eigener Wert)
    assert_eq!(px("ä", TITLE), px("a", TITLE));
    assert_eq!(px("Ä", TITLE), px("A", TITLE));
    assert!(px("ß", TITLE) > 0 && px("ß", DESC) > 0);
    // andere Größen und Bold rechnen mit Liberation Sans (einmal gerundet), nicht mit den Profilen
    assert_eq!(width_px("W", 20.5, false), 19);
    assert_eq!(
        width_px("W", 20.0, true),
        19,
        "Bold: Liberation Sans Bold, einmal gerundet"
    );
    assert_ne!(
        width_px(&"i".repeat(10), 20.5, false),
        px(&"i".repeat(10), TITLE)
    );
}

// spec: UI-038, ONPAGE-004
#[test]
fn rounding_happens_once_at_the_end() {
    // Liberation-Sans-Modell (Größen außerhalb der kalibrierten Profile): „ii“ = 2 × 4,443 = 8,89 → 9
    // (nicht 2 × gerundete 4 = 8)
    assert_eq!(px("ii", 20.01), 9);
    assert_eq!(px("i", 20.01), 4);
    // Bei Schriftgröße 2048 px ist die Breite exakt die Advance-Width in Schrifteinheiten („a“ = 1139).
    assert_eq!(width_px("a", 2048.0, false), 1139);
    // Kaufmännisch: ab 0,5 auf. Das Leerzeichen hat 569 Einheiten: 569/2048 × 1,8 = 0,5001 → 1; × 1,7 = 0,4723 → 0.
    assert_eq!(px(" ", 1.8), 1);
    assert_eq!(px(" ", 1.7), 0);
    // Genau 0,5 rundet auf: „c“ hat 1024 Einheiten = 0,5 em, bei 1 px also exakt 0,5 → 1; zwei „c“ = 1,0 → 1.
    assert_eq!(px("c", 1.0), 1);
    assert_eq!(px("cc", 1.0), 1);
    assert_eq!(px("ccc", 1.0), 2); // 1,5 → 2
}

// spec: UI-038
#[test]
fn empty_text_is_zero_and_space_has_width() {
    assert_eq!(px("", TITLE), 0);
    assert!(px(" ", TITLE) > 0);
    assert!(px("a b", TITLE) > px("ab", TITLE));
}

// spec: UI-038
#[test]
fn bold_is_at_least_as_wide_as_regular() {
    let text = "Schuhe kaufen";
    let regular = width_px(text, TITLE, false);
    let bold = width_px(text, TITLE, true);
    assert!(bold >= regular, "{bold} < {regular}");
    assert!(bold > regular, "Bold muss eine eigene Metrik nutzen");
    assert_eq!(width_px("", TITLE, true), 0);
}

// spec: UI-038
#[test]
fn fallback_and_zero_width() {
    // „漢“ hat keine Glyphe: 1 em = 20 px.
    assert_eq!(px("漢", TITLE), 20);
    assert_eq!(px("漢字", TITLE), 40);
    assert_eq!(px("😀", TITLE), 20);
    // Steuerzeichen und Zero-Width-Zeichen: 0.
    for c in [
        '\u{200B}', '\u{200D}', '\u{FE0F}', '\u{0000}', '\u{0007}', '\u{000A}', '\u{007F}',
        '\u{0085}', '\u{FEFF}', '\u{2060}', '\u{00AD}',
    ] {
        assert_eq!(px(&c.to_string(), TITLE), 0, "U+{:04X}", c as u32);
        assert_eq!(
            px(&format!("a{c}b"), TITLE),
            px("ab", TITLE),
            "U+{:04X}",
            c as u32
        );
    }
}

// spec: UI-038
#[test]
fn graphemes_count_once() {
    // Familien-Emoji mit ZWJ = ein Graphem = eine Fallback-Breite.
    assert_eq!(px("👨\u{200D}👩\u{200D}👧", TITLE), 20);
    // Emoji mit Variationsselektor und mit Hautton.
    assert_eq!(px("❤\u{FE0F}", TITLE), 20);
    assert_eq!(px("👍🏽", TITLE), 20);
    // Flagge (zwei Regional Indicators) = ein Graphem.
    assert_eq!(px("🇩🇪", TITLE), 20);
    // Kombinierende Zeichen erweitern die Basis: „é“ als e + U+0301 = „é“ (precomposed).
    assert_eq!(px("e\u{0301}", TITLE), px("é", TITLE));
    assert_eq!(px("e\u{0301}", TITLE), px("e", TITLE));
    // CRLF ist ein Graphem aus zwei Steuerzeichen.
    assert_eq!(px("a\r\nb", TITLE), px("ab", TITLE));
}

// spec: UI-038
#[test]
fn umlauts_and_common_symbols_have_glyph_widths() {
    // Latin-1 und typische Titelzeichen kommen aus der Tabelle, nicht aus dem Fallback.
    for c in "äöüÄÖÜßéèêñç€–„“‚‘’·&@#%".chars() {
        let w = width_px(&c.to_string(), 2048.0, false);
        assert!(
            w > 0 && w != 2048,
            "{c} sollte eine Glyphenbreite haben, nicht den Fallback: {w}"
        );
    }
    assert_eq!(width_px("ä", 2048.0, false), width_px("a", 2048.0, false));
    assert_eq!(width_px("Ö", 2048.0, false), width_px("O", 2048.0, false));
}

// spec: UI-038
#[test]
fn non_positive_or_non_finite_sizes_are_zero() {
    assert_eq!(width_px("abc", 0.0, false), 0);
    assert_eq!(width_px("abc", -5.0, false), 0);
    assert_eq!(width_px("abc", f64::NAN, false), 0);
    assert_eq!(width_px("abc", f64::INFINITY, false), 0);
}

// spec: UI-038, ONPAGE-004
#[test]
fn annotate_sets_px_for_all_texts() {
    let mut list: Vec<ExtractedText> = ["W", "ii", ""]
        .iter()
        .map(|t| ExtractedText {
            text: (*t).to_string(),
            len: t.chars().count() as u32,
            px: None,
            in_head: true,
        })
        .collect();
    annotate(&mut list, 20.0, false);
    assert_eq!(
        list.iter().map(|t| t.px).collect::<Vec<_>>(),
        [Some(18), Some(10), Some(0)]
    );
    annotate(&mut list, 20.0, true);
    assert_eq!(list[0].px, Some(width_px("W", 20.0, true)));
    let mut empty: Vec<ExtractedText> = Vec::new();
    annotate(&mut empty, 20.0, false);
    assert!(empty.is_empty());
}

// spec: UI-038
#[test]
#[ignore = "Benchmark (nur Bericht): cargo test -p os-parse --release --test pixel_width throughput -- --ignored --nocapture"]
fn throughput_report() {
    let titles: Vec<String> = (0..1000)
        .map(|i| format!("Schuhe kaufen {i} – günstig & schnell im Online Shop | Marke {i}"))
        .collect();
    let start = std::time::Instant::now();
    let mut total = 0u64;
    let rounds = 1000;
    for _ in 0..rounds {
        for t in &titles {
            total += u64::from(width_px(t, TITLE, false));
        }
    }
    let secs = start.elapsed().as_secs_f64();
    let per_min = (rounds * titles.len()) as f64 / secs * 60.0;
    println!("{:.1} Mio. Titel/min ({} px Summe)", per_min / 1e6, total);
}

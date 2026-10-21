//! Pixelbreiten-Engine (UI-038): Zwei kalibrierte Standardprofile und ein Liberation-Sans-Modell für alles andere.
//!
//! **Standardprofile (K-64, Paritätsbefund N-04):** Titel 20 px und Beschreibung 14 px, Regular. Die Referenz rechnet
//! mit ganzzahligen Breiten je Zeichen; [`calibrated`] hält die an den Paritäts-Fixtures gemessenen Tabellen. Die
//! Breite ist die Summe dieser ganzzahligen Zeichenbreiten je Graphem, ohne Kerning und ohne Ligaturen und ohne
//! Rundung am Ende. Zeichen, die in der Tabelle fehlen, zählen mit der auf ganze Pixel gerundeten Breite aus
//! Liberation Sans, Zeichen ohne Glyphe (CJK, Emoji, …) mit 1 em.
//!
//! **Alle anderen Größen und Bold** (SERP-Hervorhebung, Mobil-/Tablet-Profile): Schriftmetriken von Liberation Sans
//! (metrisch Arial-kompatibel), Regular und Bold. Version und Lizenz der Schriftdateien stehen in `NOTICE` und im Kopf
//! von [`metrics`]. Breite = Summe der Advance-Widths aller Grapheme (`Einheiten / 2048 × Schriftgröße`), in `f64`
//! summiert, ohne Kerning und ohne Ligaturen, **einmal** am Ende kaufmännisch auf die nächste ganze Zahl gerundet
//! (0,5 rundet auf). Damit ist das Ergebnis auf allen Plattformen bitgleich und unabhängig von Systemschriften. Die
//! Tabellen stehen in [`metrics`] (generiert von `examples/gen_pixel_metrics.rs`).
//!
//! Regeln je Graphem (erweiterter Graphem-Cluster nach UAX #29), für beide Wege:
//! * Maßgeblich ist das erste Zeichen des Clusters, das nicht zur Breite 0 zählt; die übrigen Zeichen
//!   (kombinierende Zeichen, Variationsselektoren, ZWJ-Folgen, Hautton-Modifikatoren) erweitern es und
//!   addieren nichts. Ein Familien-Emoji mit ZWJ ist damit ein Graphem mit einer Fallback-Breite.
//! * Steuerzeichen (C0, DEL, C1) und unsichtbare Formatzeichen (u. a. U+200B … U+200F, U+2060 … U+2064,
//!   U+FEFF, U+00AD, Variationsselektoren) haben Breite 0.
//! * Zeichen ohne Eintrag in der Tabelle (CJK, Emoji, …) haben die Fallback-Breite 1 em.

mod calibrated;
mod metrics;

use os_model::ExtractedText;
use unicode_segmentation::UnicodeSegmentation;

/// Fallback-Breite in Schrifteinheiten für Zeichen ohne Glyphe: 1 em.
const FALLBACK_UNITS: u16 = metrics::UNITS_PER_EM;

/// Hat das Zeichen die Breite 0 (Steuerzeichen, unsichtbare Formatzeichen)?
fn is_zero_width(c: char) -> bool {
    matches!(
        c,
        '\u{0000}'..='\u{001F}'
            | '\u{007F}'..='\u{009F}'
            | '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{115F}'..='\u{1160}'
            | '\u{17B4}'..='\u{17B5}'
            | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{2028}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFFB}'
            | '\u{E0000}'..='\u{E0FFF}'
    )
}

/// Advance-Width in Schrifteinheiten; `None` = keine Glyphe.
fn lookup(c: char, bold: bool) -> Option<u16> {
    let cp = c as u32;
    let (ascii, other): (&[u16; 128], &[(u32, u16)]) = if bold {
        (&metrics::BOLD_ASCII, &metrics::BOLD_OTHER)
    } else {
        (&metrics::REGULAR_ASCII, &metrics::REGULAR_OTHER)
    };
    if cp < 128 {
        // Im ASCII-Feld bedeutet 0 „keine Glyphe“ (alle sichtbaren ASCII-Zeichen haben Breite > 0).
        let units = ascii[cp as usize];
        return (units != 0).then_some(units);
    }
    other
        .binary_search_by_key(&cp, |&(code, _)| code)
        .ok()
        .map(|i| other[i].1)
}

/// Breite eines Graphems in Schrifteinheiten.
fn grapheme_units(grapheme: &str, bold: bool) -> u16 {
    for c in grapheme.chars() {
        if is_zero_width(c) {
            continue;
        }
        return lookup(c, bold).unwrap_or(FALLBACK_UNITS);
    }
    0
}

/// Breite eines Graphems im kalibrierten Profil in ganzen Pixeln (0 für Zero-Width-Graphem).
fn calibrated_grapheme_px(grapheme: &str, table: &[(char, u8)], size_px: f64) -> u32 {
    for c in grapheme.chars() {
        if is_zero_width(c) {
            continue;
        }
        if let Some(w) = calibrated::lookup(table, c) {
            return w;
        }
        // nicht gemessen: Breite aus Liberation Sans, auf ganze Pixel gerundet; ohne Glyphe 1 em
        let units = lookup(c, false).unwrap_or(FALLBACK_UNITS);
        let px = f64::from(units) / f64::from(metrics::UNITS_PER_EM) * size_px;
        return (px + 0.5).floor() as u32;
    }
    0
}

/// Breite eines Textes in Pixeln bei gegebener Schriftgröße (UI-038). Leerer Text → 0.
///
/// Regular bei 20 px und 14 px: kalibrierte ganzzahlige Zeichenbreiten ([`calibrated`]); alle anderen Fälle:
/// Liberation Sans, einmal am Ende gerundet.
pub fn width_px(text: &str, size_px: f64, bold: bool) -> u32 {
    if !bold {
        if let Some(table) = calibrated::table_for(size_px) {
            return text
                .graphemes(true)
                .map(|g| calibrated_grapheme_px(g, table, size_px))
                .fold(0u32, u32::saturating_add);
        }
    }
    let em = f64::from(metrics::UNITS_PER_EM);
    let mut sum = 0.0f64;
    for grapheme in text.graphemes(true) {
        let units = grapheme_units(grapheme, bold);
        if units != 0 {
            sum += f64::from(units) / em * size_px;
        }
    }
    if !sum.is_finite() || sum <= 0.0 {
        return 0;
    }
    // Einmal am Ende kaufmännisch runden (0,5 auf); `as` sättigt bei Überlauf.
    (sum + 0.5).floor() as u32
}

/// Setzt `ExtractedText.px` für alle Texte.
// Die Signatur mit `&mut Vec` ist im M1-Plan (3.6) festgeschrieben.
#[allow(clippy::ptr_arg)]
pub fn annotate(texts: &mut Vec<ExtractedText>, size_px: f64, bold: bool) {
    for t in texts.iter_mut() {
        t.px = Some(width_px(&t.text, size_px, bold));
    }
}

/// Character offset of the last complete word fitting with the suffix ` …`.
/// None means the original text already fits. Offsets count Unicode scalar values.
/// Some(0) means no complete word fits. Append ` …` to the selected prefix;
/// when even the suffix exceeds the limit, display an empty preview.
pub fn truncate_at(text: &str, max_px: u32, size_px: f64, bold: bool) -> Option<u32> {
    if width_px(text, size_px, bold) <= max_px {
        return None;
    }
    let mut cut = 0;
    for (byte, word) in text.unicode_word_indices() {
        let end = byte + word.len();
        let candidate = format!("{} …", text[..end].trim_end());
        if width_px(&candidate, size_px, bold) <= max_px {
            cut = text[..end].chars().count() as u32;
        } else {
            break;
        }
    }
    Some(cut)
}

/// Effective title/description limits and font sizes for a configured device.
pub fn serp_profile(cfg: &os_model::CrawlConfig, device: &str) -> (u32, u32, f64, f64) {
    let p = match device {
        "mobile" => &cfg.serp.mobile,
        "tablet" => &cfg.serp.tablet,
        _ => &cfg.serp.desktop,
    };
    (
        p.title_max_px.unwrap_or(cfg.title.max_px),
        p.description_max_px.unwrap_or(cfg.meta_description.max_px),
        p.title_font_px,
        p.description_font_px,
    )
}

/// A field's metrics and final preview, ready for the SERP detail service (M2-K09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerpMeasurement {
    pub chars: u32,
    pub px: u32,
    pub max_px: u32,
    /// Negative when the original text exceeds the configured limit.
    pub remaining: i64,
    /// Unicode scalar offset, matching [`truncate_at`].
    pub cut_at: Option<u32>,
    pub preview: String,
}

/// Measures original or edited text without altering stored extraction values.
pub fn measure_serp(text: &str, max_px: u32, size_px: f64, bold: bool) -> SerpMeasurement {
    let px = width_px(text, size_px, bold);
    let cut_at = truncate_at(text, max_px, size_px, bold);
    let preview = match cut_at {
        None => text.to_owned(),
        Some(_) if width_px(" …", size_px, bold) > max_px => String::new(),
        Some(cut) => format!("{} …", text.chars().take(cut as usize).collect::<String>()),
    };
    SerpMeasurement {
        chars: text.chars().count() as u32,
        px,
        max_px,
        remaining: i64::from(max_px) - i64::from(px),
        cut_at,
        preview,
    }
}

/// Mixed Regular/Bold SERP text, keeping extended grapheme clusters indivisible (UI-038).
/// Search terms are literal, case-insensitive and may overlap; callers normalize entities/whitespace first.
pub fn width_px_with_bold_terms(text: &str, size_px: f64, terms: &[String]) -> u32 {
    if terms.iter().all(String::is_empty) {
        return width_px(text, size_px, false);
    }
    let mut boundaries: Vec<_> = text.char_indices().map(|(byte, _)| byte).collect();
    boundaries.push(text.len());
    let prepared: Vec<_> = terms
        .iter()
        .filter(|t| !t.is_empty())
        .map(|t| {
            (
                t.as_str(),
                t.chars().count(),
                (!t.is_ascii()).then(|| t.to_lowercase()),
            )
        })
        .collect();
    let mut marked = vec![false; text.len()];
    let mut matched = false;
    for (i, &start) in boundaries[..boundaries.len() - 1].iter().enumerate() {
        for (term, count, lower) in &prepared {
            if let Some(&end) = boundaries.get(i + count) {
                let candidate = &text[start..end];
                if candidate.eq_ignore_ascii_case(term)
                    || lower
                        .as_ref()
                        .is_some_and(|t| candidate.to_lowercase() == *t)
                {
                    marked[start..end].fill(true);
                    matched = true;
                }
            }
        }
    }
    if !matched {
        return width_px(text, size_px, false);
    }
    let mut sum = 0.0;
    for (start, grapheme) in text.grapheme_indices(true) {
        let bold = marked[start..start + grapheme.len()].iter().any(|b| *b);
        sum += if !bold {
            calibrated::table_for(size_px)
                .map(|table| f64::from(calibrated_grapheme_px(grapheme, table, size_px)))
                .unwrap_or_else(|| {
                    f64::from(grapheme_units(grapheme, false)) / f64::from(metrics::UNITS_PER_EM)
                        * size_px
                })
        } else {
            f64::from(grapheme_units(grapheme, true)) / f64::from(metrics::UNITS_PER_EM) * size_px
        };
    }
    (sum + 0.5).floor() as u32
}

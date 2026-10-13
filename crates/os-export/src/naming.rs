//! Dateinamen der Exporte (OUT-008).
//!
//! Regeln: kanonischer Name in Kleinbuchstaben; alles außer `a-z 0-9 _ - .` wird zu `_`, mehrere `_`
//! werden zusammengefasst, führende und abschließende `_` (sowie `.` und `-`) entfallen. Der Stamm hat
//! höchstens [`MAX_STEM_LEN`] Zeichen (Kürzung vor der Endung); wird gekürzt, hängt ein 8-stelliger Hash über
//! die Eingabe am Stamm, damit die Namen eindeutig bleiben. Windows-Gerätenamen (`con`, `nul`, `com1` …)
//! werden mit `-file` entschärft. Segment-Exporte bekommen `_segment_<name>`.

use crate::fs_paths::{is_windows_reserved, short_hash};

/// Höchste Länge des Dateinamens ohne Endung.
pub const MAX_STEM_LEN: usize = 120;

/// Dateiname zu einem kanonischen Export-Namen, optional mit Segment (`_segment_<name>`).
///
/// Beispiel: `file_name("Response Codes:Client Error (4xx)", None, "csv")` →
/// `response_codes_client_error_4xx.csv`.
pub fn file_name(canonical: &str, segment: Option<&str>, ext: &str) -> String {
    let canon = sanitize(canonical);
    let seg = segment.map(|s| {
        let t = sanitize(s);
        if t.is_empty() {
            // Segmentname ganz ohne erlaubte Zeichen (z. B. `日本`): Hash statt Leerstring, damit
            // verschiedene Segmente verschiedene Dateien ergeben.
            short_hash(s)
        } else {
            t
        }
    });
    let canon = if canon.is_empty() {
        "export".to_string()
    } else {
        canon
    };
    let suffix = seg.map(|s| format!("_segment_{s}")).unwrap_or_default();
    let full_len = canon.chars().count() + suffix.chars().count();

    let stem = if full_len <= MAX_STEM_LEN {
        format!("{canon}{suffix}")
    } else {
        let hash = short_hash(&format!("{canonical}\u{0}{}", segment.unwrap_or("")));
        // `_` + 8 Hexziffern
        let hash_len = 9;
        let suffix_len = suffix.chars().count();
        if !suffix.is_empty() && MAX_STEM_LEN > suffix_len + hash_len + 8 {
            // Segmentsuffix bleibt sichtbar, der kanonische Teil wird gekürzt.
            let budget = MAX_STEM_LEN - suffix_len - hash_len;
            format!("{}_{hash}{suffix}", cut(&canon, budget))
        } else {
            let all = format!("{canon}{suffix}");
            format!("{}_{hash}", cut(&all, MAX_STEM_LEN - hash_len))
        }
    };
    let stem = defuse_reserved(stem);
    let ext = sanitize_ext(ext);
    if ext.is_empty() {
        stem
    } else {
        format!("{stem}.{ext}")
    }
}

/// Wendet die Zeichenregeln von OUT-008 auf `text` an (ohne Längenbegrenzung, ohne Endung).
pub fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let c = ch.to_ascii_lowercase();
        let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '.' | '_');
        let c = if ok { c } else { '_' };
        if c == '_' && out.ends_with('_') {
            continue;
        }
        out.push(c);
    }
    out.trim_matches(['_', '.', '-']).to_string()
}

/// Bereinigt eine Dateiendung: Kleinbuchstaben, nur `a-z 0-9 .`, ohne führenden Punkt.
pub fn sanitize_ext(ext: &str) -> String {
    let s: String = ext
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '.')
        .collect();
    s.trim_matches('.').to_string()
}

/// Kürzt einen ASCII-Stamm auf `max` Zeichen und entfernt danach abschließende `_ . -`.
pub(crate) fn cut(s: &str, max: usize) -> String {
    let t: String = s.chars().take(max).collect();
    t.trim_end_matches(['_', '.', '-']).to_string()
}

/// Windows lässt `con`, `nul`, `com1` … auch mit Endung (`con.csv`) nicht als Dateinamen zu; geprüft wird
/// der Teil vor dem ersten Punkt.
fn defuse_reserved(stem: String) -> String {
    let first = stem.split('.').next().unwrap_or("");
    if is_windows_reserved(first) {
        let first_len = first.len();
        format!("{first}-file{}", &stem[first_len..])
    } else {
        stem
    }
}

/// Prüft eine Menge kanonischer Namen auf Dateinamen-Kollisionen (Endung `csv`, ohne Segment) und liefert
/// je kollidierendem Dateinamen die beteiligten kanonischen Namen. Leer = alles eindeutig.
///
/// Dient dem Unit-Test „keine Kollision zweier Exporte auf denselben Dateinamen“ (OUT-008), den die
/// Katalogpakete über alle registrierten Namen ausführen.
pub fn collisions<'a, I>(canonical_names: I) -> Vec<(String, Vec<String>)>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut map: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for n in canonical_names {
        map.entry(file_name(n, None, "csv"))
            .or_default()
            .push(n.to_string());
    }
    map.into_iter().filter(|(_, v)| v.len() > 1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_extension() {
        assert_eq!(file_name("internal_all", None, "csv"), "internal_all.csv");
    }

    #[test]
    fn spec_examples() {
        assert_eq!(
            file_name("Response Codes:Client Error (4xx)", None, "csv"),
            "response_codes_client_error_4xx.csv"
        );
        assert_eq!(file_name("All Inlinks", None, "csv"), "all_inlinks.csv");
        assert_eq!(
            file_name("Crawl Overview", None, "xlsx"),
            "crawl_overview.xlsx"
        );
    }

    #[test]
    fn reserved_names_get_suffix() {
        assert_eq!(file_name("NUL", None, "csv"), "nul-file.csv");
        assert_eq!(file_name("com1", None, "csv"), "com1-file.csv");
        assert_eq!(
            file_name("lpt9.backup", None, "csv"),
            "lpt9-file.backup.csv"
        );
        assert_eq!(file_name("console", None, "csv"), "console.csv");
    }
}

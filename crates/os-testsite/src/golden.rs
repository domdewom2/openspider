//! Golden-Helfer: Normalisierung variabler Werte und Vergleich mit eingecheckten Dateien.
//!
//! [`normalize`] ersetzt, in dieser Reihenfolge:
//!
//! 1. jede übergebene Basis-URL (längste zuerst) durch `{BASE}`,
//! 2. UUIDs (`8-4-4-4-12` Hexziffern, also auch UUIDv7-Crawl-IDs) durch `<ID>`,
//! 3. Zeitstempel `YYYY-MM-DD[T ]HH:MM:SS[.fff][Z|±HH:MM]` durch `<TS>`,
//! 4. Dauern in Millisekunden – eine Zahl (auch mit Dezimalpunkt), optional ein Leerzeichen,
//!    dann `ms` am Wortende – durch `<MS>`.
//!
//! [`assert_golden`] vergleicht mit einer Datei; mit der Umgebungsvariable `UPDATE_GOLDEN=1`
//! schreibt es die Datei stattdessen (samt fehlender Verzeichnisse).

use std::fs;
use std::path::Path;

/// Name der Umgebungsvariable, die das Neuschreiben der Golden-Dateien auslöst.
pub const UPDATE_ENV: &str = "UPDATE_GOLDEN";

/// Normalisiert variable Werte (Basis-URLs, IDs, Zeitstempel, Millisekunden), siehe Modul-Doku.
pub fn normalize(text: &str, bases: &[&str]) -> String {
    let mut sorted: Vec<&str> = bases.iter().copied().filter(|b| !b.is_empty()).collect();
    sorted.sort_by_key(|b| std::cmp::Reverse(b.len()));
    let mut out = text.to_string();
    for base in sorted {
        out = out.replace(base, "{BASE}");
    }
    let out = replace_scanned(&out, scan_uuid, "<ID>");
    let out = replace_scanned(&out, scan_timestamp, "<TS>");
    replace_scanned(&out, scan_ms, "<MS>")
}

/// Vergleicht `actual` mit dem Inhalt von `path`; bei `UPDATE_GOLDEN=1` wird `path` geschrieben.
/// Zeilenenden werden vor dem Vergleich auf `\n` vereinheitlicht (Git-Autocrlf unter Windows).
///
/// # Panics
/// Bei Abweichung, fehlender Datei (ohne `UPDATE_GOLDEN`) oder nicht schreibbarer Datei.
pub fn assert_golden(path: impl AsRef<Path>, actual: &str) {
    let update = std::env::var(UPDATE_ENV).is_ok_and(|v| !v.is_empty() && v != "0");
    assert_golden_with(path, actual, update);
}

/// Wie [`assert_golden`], aber mit ausdrücklicher Wahl zwischen Schreiben (`update`) und
/// Vergleichen – für Tests des Helfers selbst, ohne die Prozessumgebung zu verändern.
///
/// # Panics
/// Wie [`assert_golden`].
pub fn assert_golden_with(path: impl AsRef<Path>, actual: &str, update: bool) {
    let path = path.as_ref();
    if update {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .unwrap_or_else(|e| panic!("Golden-Verzeichnis {}: {e}", dir.display()));
        }
        fs::write(path, actual)
            .unwrap_or_else(|e| panic!("Golden-Datei {} schreiben: {e}", path.display()));
        return;
    }
    let expected = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "Golden-Datei {} nicht lesbar ({e}); mit {UPDATE_ENV}=1 neu erzeugen",
            path.display()
        )
    });
    let expected = expected.replace("\r\n", "\n");
    let actual_lf = actual.replace("\r\n", "\n");
    if expected != actual_lf {
        panic!(
            "Golden-Abweichung in {}:\n{}\n(mit {UPDATE_ENV}=1 neu erzeugen, wenn beabsichtigt)",
            path.display(),
            first_difference(&expected, &actual_lf)
        );
    }
}

fn first_difference(expected: &str, actual: &str) -> String {
    let mut e = expected.lines();
    let mut a = actual.lines();
    let mut line = 1;
    loop {
        match (e.next(), a.next()) {
            (None, None) => {
                return "Unterschied nur am Dateiende (abschließender Zeilenumbruch)".into()
            }
            (x, y) if x == y => line += 1,
            (x, y) => {
                return format!(
                    "Zeile {line}:\n  erwartet: {}\n  erhalten: {}",
                    x.unwrap_or("<Dateiende>"),
                    y.unwrap_or("<Dateiende>")
                )
            }
        }
    }
}

/// Ersetzt alle Treffer von `scan`. `scan(bytes, i)` liefert die Trefferlänge ab `i`.
fn replace_scanned(text: &str, scan: fn(&[u8], usize) -> Option<usize>, with: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut copied = 0;
    while i < bytes.len() {
        // Nur an Zeichengrenzen suchen (Treffer sind reines ASCII).
        if text.is_char_boundary(i) {
            if let Some(len) = scan(bytes, i) {
                out.push_str(&text[copied..i]);
                out.push_str(with);
                i += len;
                copied = i;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&text[copied..]);
    out
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn digits(b: &[u8], i: usize, n: usize) -> bool {
    b.len() >= i + n && b[i..i + n].iter().all(u8::is_ascii_digit)
}

fn hex(b: &[u8], i: usize, n: usize) -> bool {
    b.len() >= i + n && b[i..i + n].iter().all(u8::is_ascii_hexdigit)
}

fn scan_uuid(b: &[u8], i: usize) -> Option<usize> {
    if i > 0 && (is_word(b[i - 1]) || b[i - 1] == b'-') {
        return None;
    }
    let mut pos = i;
    for (k, n) in [8usize, 4, 4, 4, 12].into_iter().enumerate() {
        if k > 0 {
            if b.get(pos) != Some(&b'-') {
                return None;
            }
            pos += 1;
        }
        if !hex(b, pos, n) {
            return None;
        }
        pos += n;
    }
    if b.get(pos).is_some_and(|c| is_word(*c) || *c == b'-') {
        return None;
    }
    Some(pos - i)
}

fn scan_timestamp(b: &[u8], i: usize) -> Option<usize> {
    if i > 0 && is_word(b[i - 1]) {
        return None;
    }
    // YYYY-MM-DD
    if !(digits(b, i, 4) && b.get(i + 4) == Some(&b'-') && digits(b, i + 5, 2)) {
        return None;
    }
    if !(b.get(i + 7) == Some(&b'-') && digits(b, i + 8, 2)) {
        return None;
    }
    let mut pos = i + 10;
    if !matches!(b.get(pos), Some(b'T' | b' ')) {
        return None;
    }
    pos += 1;
    // HH:MM:SS
    if !(digits(b, pos, 2) && b.get(pos + 2) == Some(&b':') && digits(b, pos + 3, 2)) {
        return None;
    }
    if !(b.get(pos + 5) == Some(&b':') && digits(b, pos + 6, 2)) {
        return None;
    }
    pos += 8;
    if b.get(pos) == Some(&b'.') && b.get(pos + 1).is_some_and(u8::is_ascii_digit) {
        pos += 1;
        while b.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
    }
    match b.get(pos) {
        Some(b'Z') => pos += 1,
        Some(b'+' | b'-')
            if digits(b, pos + 1, 2) && b.get(pos + 3) == Some(&b':') && digits(b, pos + 4, 2) =>
        {
            pos += 6;
        }
        _ => {}
    }
    Some(pos - i)
}

fn scan_ms(b: &[u8], i: usize) -> Option<usize> {
    if !b[i].is_ascii_digit() || (i > 0 && (is_word(b[i - 1]) || b[i - 1] == b'.')) {
        return None;
    }
    let mut pos = i;
    while b.get(pos).is_some_and(u8::is_ascii_digit) {
        pos += 1;
    }
    if b.get(pos) == Some(&b'.') && b.get(pos + 1).is_some_and(u8::is_ascii_digit) {
        pos += 1;
        while b.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
    }
    if b.get(pos) == Some(&b' ') {
        pos += 1;
    }
    if b.get(pos) == Some(&b'm') && b.get(pos + 1) == Some(&b's') {
        if b.get(pos + 2).is_some_and(|c| is_word(*c)) {
            return None;
        }
        return Some(pos + 2 - i);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bases_longest_first() {
        let t = "http://127.0.0.1:80/a http://127.0.0.1:8080/b";
        assert_eq!(
            normalize(t, &["http://127.0.0.1:80", "http://127.0.0.1:8080"]),
            "{BASE}/a {BASE}/b"
        );
    }

    #[test]
    fn timestamps_ids_and_ms() {
        let t = "at 2026-01-01T12:30:45.123Z and 2026-02-03 04:05:06+02:00; \
                 id 0192f0c1-7a3b-7c2e-9d4f-0123456789ab; took 120ms or 3.5 ms, 12 msg";
        assert_eq!(
            normalize(t, &[]),
            "at <TS> and <TS>; id <ID>; took <MS> or <MS>, 12 msg"
        );
    }

    #[test]
    fn leaves_other_numbers_alone() {
        assert_eq!(
            normalize("port 8080, 2026-01-01", &[]),
            "port 8080, 2026-01-01"
        );
    }

    #[test]
    fn non_ascii_is_preserved() {
        assert_eq!(normalize("Größe 5ms ü", &[]), "Größe <MS> ü");
    }
}

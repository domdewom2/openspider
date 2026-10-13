//! Pfade im Dateisystem: Unicode, Windows-Langpfade, Namenskollisionen (OUT-268).
//!
//! * [`windows_extended_path`] / [`fs_path`]: Langpfade (`\\?\`-Präfix) unter Windows; die Umformung ist eine
//!   reine Zeichenkettenfunktion und deshalb auf jedem System testbar.
//! * [`NameRegistry`]: erkennt Kollisionen, die nur durch Groß-/Kleinschreibung entstehen (macOS und Windows
//!   sind standardmäßig case-insensitiv), und löst sie mit einem Hash-Suffix auf.
//! * [`valid_component`]: macht beliebigen Text (auch Unicode) zu einem gültigen Dateinamen beider
//!   Dateisysteme, ohne Unicode-Zeichen zu verlieren; [`hashed_file_name`] liefert die ASCII-Form nach OUT-008
//!   mit 8-stelligem Hash (`<bereinigte URL>_<hash>.<ext>`).
//!
//! Alle Pfade werden über `Path`/`PathBuf` zusammengesetzt, nie mit festen Trennzeichen.

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use crate::naming;

/// Ab dieser Länge (Zeichen) wird ein absoluter Windows-Pfad in die erweiterte Form (`\\?\`) gebracht.
/// `MAX_PATH` ist 260 (inklusive Abschluss); Verzeichnisse dürfen höchstens 248 haben.
pub const WINDOWS_LONG_PATH_THRESHOLD: usize = 240;

/// 32-Bit-Hash (FNV-1a 64, gefaltet) als 8 Hexziffern. Stabil über Plattformen und Rust-Versionen,
/// damit Dateinamen reproduzierbar bleiben.
pub fn short_hash(text: &str) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(PRIME);
    }
    let folded = (h ^ (h >> 32)) as u32;
    format!("{folded:08x}")
}

/// Vergleichsform für case-insensitive Dateisysteme (Unicode-Kleinschreibung).
pub fn fold_case(name: &str) -> String {
    name.to_lowercase()
}

/// `true` für Gerätenamen, die Windows nicht als Dateinamen zulässt (`con`, `nul`, `com1` …),
/// unabhängig von Groß-/Kleinschreibung.
pub fn is_windows_reserved(stem: &str) -> bool {
    let up = stem.to_ascii_uppercase();
    match up.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" => true,
        _ => {
            (up.len() == 4 && (up.starts_with("COM") || up.starts_with("LPT")))
                && up.as_bytes()[3].is_ascii_digit()
        }
    }
}

/// Macht aus beliebigem Text einen gültigen Dateinamen für macOS **und** Windows und behält dabei
/// Unicode-Zeichen (`Jürgen`, `日本`). Ersetzt `< > : " / \ | ? *` und Steuerzeichen durch `_`, entfernt
/// Leerzeichen am Anfang und Leerzeichen/Punkte am Ende, entschärft Windows-Gerätenamen und kürzt auf
/// höchstens 200 Zeichen.
pub fn valid_component(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    s = s.trim_start().to_string();
    if s.chars().count() > 200 {
        s = s.chars().take(200).collect();
    }
    let trimmed = s.trim_end_matches([' ', '.']);
    let mut s = trimmed.to_string();
    if s.is_empty() {
        return "_".to_string();
    }
    let first = s.split('.').next().unwrap_or("").trim_end().to_string();
    if is_windows_reserved(&first) {
        s = format!("{first}_{}", &s[first.len()..]);
    }
    s
}

/// Dateiname aus Text (z. B. einer URL) nach den Regeln von OUT-008 plus 8-stelligem Hash über den
/// **unbereinigten** Text: `<bereinigter Text>_<hash>.<ext>`. Zwei verschiedene Eingaben, die gleich
/// bereinigt werden, bekommen so verschiedene Namen.
pub fn hashed_file_name(text: &str, ext: &str) -> String {
    let mut stem = naming::sanitize(text);
    // 120 Zeichen Gesamtlänge: Platz für `_` + 8 Hexziffern lassen.
    stem = naming::cut(&stem, naming::MAX_STEM_LEN - 9);
    let hash = short_hash(text);
    let stem = if stem.is_empty() {
        hash
    } else {
        format!("{stem}_{hash}")
    };
    let ext = naming::sanitize_ext(ext);
    if ext.is_empty() {
        stem
    } else {
        format!("{stem}.{ext}")
    }
}

/// Wandelt einen absoluten Windows-Pfad in die erweiterte Form mit `\\?\`-Präfix (Langpfade bis ~32 000
/// Zeichen). Reine Zeichenkettenfunktion ohne Dateizugriff:
///
/// * `C:\a\b` → `\\?\C:\a\b`; `/` wird zu `\`, `.`-Komponenten und doppelte Trenner entfallen, `..`
///   wird aufgelöst (Win32 tut das mit dem Präfix nicht mehr selbst).
/// * `\\server\share\x` → `\\?\UNC\server\share\x`.
/// * bereits erweiterte Pfade (`\\?\`, `\\.\`) und relative Pfade bleiben unverändert.
pub fn windows_extended_path(path: &str) -> String {
    if path.starts_with(r"\\?\") || path.starts_with(r"\\.\") {
        return path.to_string();
    }
    let norm = path.replace('/', "\\");
    let (prefix, rest, keep_two_roots) = if let Some(unc) = norm.strip_prefix(r"\\") {
        (r"\\?\UNC\".to_string(), unc.to_string(), true)
    } else {
        let b = norm.as_bytes();
        if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' {
            (
                format!(r"\\?\{}:\", &norm[..1]),
                norm[3..].to_string(),
                false,
            )
        } else {
            return path.to_string();
        }
    };
    let mut parts: Vec<&str> = Vec::new();
    // Bei UNC bilden Server und Freigabe den Wurzelteil und werden nicht durch `..` entfernt.
    let root_len = if keep_two_roots { 2 } else { 0 };
    for seg in rest.split('\\') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.len() > root_len {
                    parts.pop();
                }
            }
            other => parts.push(other),
        }
    }
    format!("{prefix}{}", parts.join("\\"))
}

/// Pfad in der Form, die Dateioperationen auf dem aktuellen System verlangen: unter Windows lange
/// absolute Pfade mit `\\?\`-Präfix, sonst unverändert.
pub fn fs_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(s) = path.to_str() {
            if s.chars().count() >= WINDOWS_LONG_PATH_THRESHOLD && path.is_absolute() {
                return PathBuf::from(windows_extended_path(s));
            }
        }
    }
    path.to_path_buf()
}

/// `fs::create_dir_all` mit Langpfad-Behandlung.
pub fn create_dir_all(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(fs_path(path))
}

/// `File::create` mit Langpfad-Behandlung (ersetzt eine vorhandene Datei gleichen Namens).
pub fn create_file(path: &Path) -> io::Result<File> {
    File::create(fs_path(path))
}

/// Verwaltet Dateinamen innerhalb eines Ordners und löst case-insensitive Kollisionen auf.
///
/// Der erste Antragsteller behält seinen Namen. Ein späterer mit demselben Namen bis auf Groß-/
/// Kleinschreibung, aber anderem `discriminator` (z. B. die Original-URL), bekommt `<stem>_<hash><.ext>`
/// mit dem Hash über den `discriminator`; kollidiert auch das, folgt `_2`, `_3` …. Derselbe Name mit
/// demselben `discriminator` liefert immer dasselbe Ergebnis (idempotent).
#[derive(Debug, Default)]
pub struct NameRegistry {
    owners: HashMap<String, String>,
}

impl NameRegistry {
    /// Leeres Register.
    pub fn new() -> Self {
        NameRegistry::default()
    }

    /// Reserviert `name` und liefert den tatsächlich zu verwendenden Dateinamen.
    pub fn claim(&mut self, name: &str, discriminator: &str) -> String {
        let key = fold_case(name);
        match self.owners.get(&key) {
            None => {
                self.owners.insert(key, discriminator.to_string());
                return name.to_string();
            }
            Some(owner) if owner == discriminator => return name.to_string(),
            Some(_) => {}
        }
        let (stem, ext) = split_ext(name);
        let hash = short_hash(discriminator);
        let mut candidate = format!("{stem}_{hash}{ext}");
        let mut n = 2u32;
        loop {
            let key = fold_case(&candidate);
            match self.owners.get(&key) {
                None => {
                    self.owners.insert(key, discriminator.to_string());
                    return candidate;
                }
                Some(owner) if owner == discriminator => return candidate,
                Some(_) => {
                    candidate = format!("{stem}_{hash}_{n}{ext}");
                    n += 1;
                }
            }
        }
    }
}

/// Teilt `name` in Stamm und Endung (Endung mit führendem Punkt, sonst leer). Ein Punkt am Anfang oder
/// Ende zählt nicht als Endungstrenner.
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable() {
        assert_eq!(short_hash("a"), short_hash("a"));
        assert_ne!(short_hash("a"), short_hash("b"));
        assert_eq!(short_hash("a").len(), 8);
    }

    #[test]
    fn extended_paths() {
        assert_eq!(windows_extended_path(r"C:\a\b"), r"\\?\C:\a\b");
        assert_eq!(windows_extended_path("C:/a/./b/../c"), r"\\?\C:\a\c");
        assert_eq!(
            windows_extended_path(r"\\srv\share\x\..\y"),
            r"\\?\UNC\srv\share\y"
        );
        assert_eq!(windows_extended_path(r"\\?\C:\x"), r"\\?\C:\x");
        assert_eq!(windows_extended_path(r"rel\path"), r"rel\path");
        assert_eq!(windows_extended_path(r"C:\.."), r"\\?\C:\");
    }

    #[test]
    fn registry_resolves_case_collisions() {
        let mut r = NameRegistry::new();
        assert_eq!(r.claim("Page.png", "u1"), "Page.png");
        let second = r.claim("page.png", "u2");
        assert_eq!(second, format!("page_{}.png", short_hash("u2")));
        assert_eq!(r.claim("Page.png", "u1"), "Page.png");
        assert_eq!(r.claim("page.png", "u2"), second);
    }

    #[test]
    fn valid_component_keeps_unicode() {
        assert_eq!(valid_component("Jürgen"), "Jürgen");
        assert_eq!(valid_component("日本"), "日本");
        assert_eq!(valid_component("a/b:c"), "a_b_c");
        assert_eq!(valid_component("nul"), "nul_");
        assert_eq!(valid_component("con.txt"), "con_.txt");
        assert_eq!(valid_component("name. "), "name");
        assert_eq!(valid_component(""), "_");
    }
}

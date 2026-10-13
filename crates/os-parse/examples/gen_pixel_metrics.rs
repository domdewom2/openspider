//! Erzeugt `src/pixel/metrics.rs` aus den eingecheckten Liberation-Sans-Schriftdateien (UI-038).
//!
//! Aufruf (aus dem Repo-Wurzelverzeichnis):
//! `cargo run -p os-parse --example gen_pixel_metrics`
//!
//! Optional: Ausgabepfad als erstes Argument; mit `--check` wird die vorhandene Datei nur mit der
//! frisch erzeugten verglichen (Exit-Code 1 bei Abweichung).
//!
//! Die Schriften liegen unter `resources/fonts/` (SIL OFL 1.1, Attribution in `NOTICE`). Je Schrift wird die
//! Horizontal-Advance-Width jedes Zeichens der Unicode-Zeichentabelle (cmap) in Schrifteinheiten
//! (`units_per_em` = 2048) ausgegeben: ASCII als dichtes Feld, alles darüber als nach Codepoint sortierte
//! Liste für die Binärsuche. Die Ausgabe ist deterministisch.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ttf_parser::Face;

const UNITS_PER_EM: u16 = 2048;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Codepoint → Advance Width (Schrifteinheiten) für alle Zeichen mit Glyphe in einer Unicode-cmap.
fn advances(font_file: &Path) -> BTreeMap<u32, u16> {
    let data = std::fs::read(font_file).unwrap_or_else(|e| panic!("{}: {e}", font_file.display()));
    let face = Face::parse(&data, 0).expect("gültige TrueType-Datei");
    assert_eq!(
        face.units_per_em(),
        UNITS_PER_EM,
        "units_per_em muss 2048 sein"
    );
    let mut map = BTreeMap::new();
    let cmap = face.tables().cmap.expect("cmap-Tabelle");
    for subtable in cmap.subtables {
        if !subtable.is_unicode() {
            continue;
        }
        subtable.codepoints(|cp| {
            let Some(ch) = char::from_u32(cp) else { return };
            if let Some(gid) = face.glyph_index(ch) {
                if let Some(adv) = face.glyph_hor_advance(gid) {
                    map.entry(cp).or_insert(adv);
                }
            }
        });
    }
    map
}

fn emit_font(out: &mut String, prefix: &str, label: &str, map: &BTreeMap<u32, u16>) {
    let _ = writeln!(
        out,
        "/// {label}: ASCII (U+0000 … U+007F), 0 = keine Glyphe."
    );
    let _ = writeln!(out, "#[rustfmt::skip]");
    let _ = writeln!(out, "pub(super) const {prefix}_ASCII: [u16; 128] = [");
    for row in 0..16u32 {
        let mut line = String::from("   ");
        for col in 0..8u32 {
            let cp = row * 8 + col;
            let _ = write!(line, " {},", map.get(&cp).copied().unwrap_or(0));
        }
        let _ = writeln!(out, "{line}");
    }
    let _ = writeln!(out, "];");
    let _ = writeln!(out);
    let rest: Vec<(u32, u16)> = map
        .iter()
        .filter(|(cp, _)| **cp >= 128)
        .map(|(cp, adv)| (*cp, *adv))
        .collect();
    let _ = writeln!(
        out,
        "/// {label}: alle übrigen Zeichen, aufsteigend nach Codepoint ({} Einträge).",
        rest.len()
    );
    let _ = writeln!(out, "#[rustfmt::skip]");
    let _ = writeln!(
        out,
        "pub(super) const {prefix}_OTHER: [(u32, u16); {}] = [",
        rest.len()
    );
    for chunk in rest.chunks(6) {
        let mut line = String::from("   ");
        for (cp, adv) in chunk {
            let _ = write!(line, " (0x{cp:04X}, {adv}),");
        }
        let _ = writeln!(out, "{line}");
    }
    let _ = writeln!(out, "];");
}

fn generate() -> String {
    let fonts = crate_dir().join("resources").join("fonts");
    let regular = advances(&fonts.join("LiberationSans-Regular.ttf"));
    let bold = advances(&fonts.join("LiberationSans-Bold.ttf"));

    let mut out = String::new();
    out.push_str(
        "//! Advance-Widths von Liberation Sans (SIL OFL 1.1), Regular und Bold, in Schrifteinheiten.\n\
         //!\n\
         //! GENERIERT von `examples/gen_pixel_metrics.rs` aus `resources/fonts/*.ttf` – nicht von Hand ändern.\n\
         //! Neu erzeugen: `cargo run -p os-parse --example gen_pixel_metrics`.\n\n",
    );
    let _ = writeln!(
        out,
        "/// Schrifteinheiten je Em.\npub(super) const UNITS_PER_EM: u16 = {UNITS_PER_EM};\n"
    );
    emit_font(&mut out, "REGULAR", "Liberation Sans Regular", &regular);
    out.push('\n');
    emit_font(&mut out, "BOLD", "Liberation Sans Bold", &bold);
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.iter().any(|a| a == "--check");
    let target = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| crate_dir().join("src").join("pixel").join("metrics.rs"));
    let generated = generate();
    if check {
        let current = std::fs::read_to_string(&target).unwrap_or_default();
        if current == generated {
            println!("{} ist aktuell", target.display());
        } else {
            eprintln!("{} weicht von der Erzeugung ab", target.display());
            std::process::exit(1);
        }
    } else {
        std::fs::write(&target, generated).unwrap_or_else(|e| panic!("{}: {e}", target.display()));
        println!("{} geschrieben", target.display());
    }
}

//! UI-038: Die eingecheckten Metriktabellen stimmen mit den Schriftdateien unter `resources/fonts/` überein,
//! und die Breiten von 50+ Titeln decken sich mit einer unabhängigen Ganzzahl-Referenzberechnung direkt aus
//! den Schriftdateien (ttf-parser). Die Ergebnisse sind plattformunabhängig, weil nur IEEE-754-`f64`-Grundrechenarten
//! auf exakt darstellbaren Werten laufen.

use os_parse::pixel::width_px;
use ttf_parser::Face;

fn font_bytes(bold: bool) -> Vec<u8> {
    let name = if bold {
        "LiberationSans-Bold.ttf"
    } else {
        "LiberationSans-Regular.ttf"
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("fonts")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn advance(face: &Face, c: char) -> Option<u32> {
    let gid = face.glyph_index(c)?;
    face.glyph_hor_advance(gid).map(u32::from)
}

/// Zeichen, die UI-038 als Breite 0 festlegt (Steuerzeichen, unsichtbare Formatzeichen).
fn is_zero_width(c: char) -> bool {
    let cp = c as u32;
    cp < 0x20
        || (0x7F..=0x9F).contains(&cp)
        || matches!(cp, 0xAD | 0x200B..=0x200F | 0x2028..=0x202E | 0x2060..=0x206F | 0xFEFF)
}

// spec: UI-038
#[test]
fn tables_match_font_advances_for_every_mapped_character() {
    for bold in [false, true] {
        let data = font_bytes(bold);
        let face = Face::parse(&data, 0).unwrap();
        assert_eq!(face.units_per_em(), 2048);
        let cmap = face.tables().cmap.expect("cmap");
        let mut checked = 0usize;
        for subtable in cmap.subtables {
            if !subtable.is_unicode() {
                continue;
            }
            subtable.codepoints(|cp| {
                let Some(c) = char::from_u32(cp) else { return };
                let Some(units) = advance(&face, c) else {
                    return;
                };
                // Ein einzelnes Zeichen ist immer ein Graphem. Bei 2048 px entspricht die Breite den Einheiten.
                let expected = if is_zero_width(c) { 0 } else { units };
                assert_eq!(
                    width_px(&c.to_string(), 2048.0, bold),
                    expected,
                    "U+{cp:04X} bold={bold}"
                );
                checked += 1;
            });
        }
        assert!(checked > 400, "zu wenige Zeichen geprüft: {checked}");
    }
}

/// Referenz: ganzzahlige Summe der Einheiten, danach `floor((Summe × Größe / 2048) + 0,5)` exakt in Ganzzahlen.
fn reference_width(face: &Face, text: &str, size: u64) -> u64 {
    let units: u64 = text
        .chars()
        .map(|c| u64::from(advance(face, c).unwrap_or_else(|| panic!("keine Glyphe für {c:?}"))))
        .sum();
    (units * size * 2 + 2048) / 4096
}

const TITLES: &[&str] = &[
    "Startseite",
    "Home",
    "Schuhe kaufen",
    "Schuhe kaufen – günstig & schnell | Shop",
    "ÄÖÜ Größenübersicht 2026",
    "Über uns – Unser Team und unsere Geschichte",
    "Kontakt",
    "Impressum | Firma GmbH",
    "WWWWWWWWWWWWWWWWWWWW",
    "iiiiiiiiiiiiiiiiiiiiiiiiiiiiii",
    "Die besten Laufschuhe 2026: Test, Vergleich und Kaufberatung",
    "Rezept: Apfelkuchen mit Zimt (einfach & lecker)",
    "FAQ – Häufig gestellte Fragen",
    "Preise ab 9,99 € – jetzt bestellen!",
    "Datenschutzerklärung",
    "Blog: 10 Tipps für besseres SEO",
    "Kleidung für Damen & Herren online kaufen",
    "Müller & Söhne – Tischlerei seit 1952",
    "AGB",
    "Versand und Zahlung: Alle Infos auf einen Blick",
    "Fahrräder, E-Bikes und Zubehör im Onlineshop",
    "Wetter Berlin: Aktuell, 7-Tage-Vorhersage und Radar",
    "Karriere bei uns – Stellenangebote",
    "Zürich Hotel ***** Luxus am See",
    "Café „Zur Sonne“ – Frühstück & Brunch",
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit",
    "XXXL Sale: bis zu 70% Rabatt auf alles!",
    "Mein Konto / Anmelden",
    "Warenkorb (3 Artikel)",
    "Neuheiten & Trends: Frühjahr/Sommer 2026",
    "Tipps zur Gartenpflege im Herbst – was jetzt zu tun ist",
    "Wohnung mieten in München – Angebote",
    "Rechner: Brutto-Netto-Gehalt 2026",
    "Versicherung vergleichen und bis zu 500 € sparen",
    "Handy-Tarife ohne Vertragsbindung",
    "Gutscheine & Aktionen",
    "SEO-Agentur Hamburg · Beratung · Content",
    "Q&A: Fragen zu Lieferzeiten",
    "Jobs in Köln, Düsseldorf und Umgebung",
    "Ärzte-Bewertung: Dr. Meier – Zahnarzt",
    "Telefon: 0800 / 123 456 789",
    "100% Bio – Naturkosmetik aus Deutschland",
    "Ünïcödé Tëst ßß",
    "Zubehör für Smartphones: Hüllen, Kabel, Ladegeräte",
    "Newsletter abonnieren",
    "The quick brown fox jumps over the lazy dog",
    "Sehr langer Titel, der garantiert über die empfohlene Pixelbreite der Suchergebnisseite hinausgeht und abgeschnitten wird",
    "Ein",
    "O'Brien's Pub – Dublin",
    "Hinweis: Bitte beachten Sie die Öffnungszeiten (Mo–Fr 9–18 Uhr)",
    "Zeit für Veränderung?",
    "Glückwunsch! Sie haben gewonnen…",
];

// spec: UI-038
#[test]
fn at_least_fifty_titles_match_the_reference_from_the_font_files() {
    assert!(TITLES.len() >= 50);
    for bold in [false, true] {
        let data = font_bytes(bold);
        let face = Face::parse(&data, 0).unwrap();
        for size in [20u64, 14] {
            for title in TITLES {
                assert_eq!(
                    u64::from(width_px(title, size as f64, bold)),
                    reference_width(&face, title, size),
                    "{title:?} {size}px bold={bold}"
                );
            }
        }
    }
}

// spec: UI-038
#[test]
fn bold_is_never_narrower_for_the_golden_titles() {
    for title in TITLES {
        assert!(
            width_px(title, 20.0, true) >= width_px(title, 20.0, false),
            "{title:?}"
        );
    }
}

// spec: UI-038
#[test]
fn golden_values_of_selected_titles_are_stable() {
    // Festgeschriebene Werte (20 px Regular, 20 px Bold, 14 px Regular). Sie stimmen mit der Referenzberechnung
    // aus den Schriftdateien überein (Test oben) und fangen versehentliche Änderungen an Tabellen, Graphem-Logik
    // oder Rundung ab; auf macOS arm64/x64 und Windows identisch.
    let golden: &[(&str, u32, u32, u32)] = &[
        ("Startseite", 84, 91, 59),
        ("Schuhe kaufen", 133, 142, 93),
        ("Schuhe kaufen – günstig & schnell | Shop", 370, 398, 259),
        ("ÄÖÜ Größenübersicht 2026", 248, 261, 174),
        (
            "Die besten Laufschuhe 2026: Test, Vergleich und Kaufberatung",
            564,
            601,
            395,
        ),
        ("Preise ab 9,99 € – jetzt bestellen!", 296, 311, 207),
        ("Café „Zur Sonne“ – Frühstück & Brunch", 352, 382, 247),
        ("The quick brown fox jumps over the lazy dog", 396, 428, 277),
        ("WWWWWWWWWWWWWWWWWWWW", 378, 0, 0),
    ];
    for (title, regular20, bold20, regular14) in golden {
        assert_eq!(width_px(title, 20.0, false), *regular20, "{title:?} 20px");
        if *bold20 != 0 {
            assert_eq!(width_px(title, 20.0, true), *bold20, "{title:?} 20px bold");
            assert_eq!(width_px(title, 14.0, false), *regular14, "{title:?} 14px");
        }
    }
}

//! Kalibrierte ganzzahlige Zeichenbreiten für die Standardprofile der Pixelbreiten-Engine (UI-038, K-64,
//! Paritätsbefund N-04).
//!
//! Die Referenz rechnet nicht mit den Advance-Widths von Liberation Sans, sondern mit ganzzahligen Breiten je
//! Zeichen (gehintete Metrik): 70 × „i“ = 350 px (5 px je Zeichen), 60 × „W“ = 1080 px (18 px je Zeichen) bei 20 px.
//! Die Breite eines Textes ist die Summe dieser Zeichenbreiten, ohne Kerning und ohne Rundung am Ende.
//!
//! **Herkunft.** Die Tabellen stammen ausschließlich aus Messungen an den eigenen Fixture-Texten der Paritäts-Sites
//! (`tests/fixtures/{out-basic-site,parity-*}`): Aus den Titeln und Beschreibungen und den Breiten, die `os-parity`
//! im Abgleich ausweist, entsteht ein lineares Gleichungssystem (Anzahl je Zeichen × Breite je Zeichen = Breite des
//! Textes). Es wurde je Profil gelöst (kleinste Quadrate, danach exakte Ganzzahlprobe); alle Gleichungen
//! (559 Titel, 574 Beschreibungen) gehen ohne Rest auf. Keine Referenzzelle ist eingebettet, keine Systemschrift
//! und keine proprietäre Metrik. Zeichen, die in den Fixtures nicht vorkommen, rechnet [`super`] mit der auf ganze
//! Pixel gerundeten Breite aus Liberation Sans.
//!
//! **Grenzen.** Die Beschreibung bei 14 px enthält die Großbuchstaben E, G, L, M, N, R, U, V und Y in den Fixtures
//! nur in einer festen Zusammensetzung; die Messung bestimmt nur die gewichtete Summe. Ihre Einzelbreiten
//! (E 8, G 11, L 7, M 11, N 10, R 9, U 10, V 9, Y 9) sind die zur Liberation-Sans-Breite nächstliegende
//! ganzzahlige Lösung. Die Tabelle ist sortiert nach Zeichen (Binärsuche).

/// Schriftgröße des Titel-Profils in Pixeln (Regular).
pub(super) const TITLE_PX: f64 = 20.0;
/// Schriftgröße des Beschreibungs-Profils in Pixeln (Regular).
pub(super) const DESCRIPTION_PX: f64 = 14.0;

/// Breite je Zeichen in Pixeln bei 20 px Regular (Titel), sortiert nach Zeichen.
pub(super) const TITLE_20: &[(char, u8)] = &[
    (' ', 5),
    ('-', 6),
    ('0', 11),
    ('1', 11),
    ('2', 11),
    ('3', 11),
    ('4', 11),
    ('5', 11),
    ('6', 11),
    ('7', 11),
    ('8', 11),
    ('9', 11),
    ('A', 13),
    ('B', 12),
    ('C', 13),
    ('D', 13),
    ('F', 11),
    ('G', 14),
    ('H', 14),
    ('I', 5),
    ('L', 11),
    ('M', 17),
    ('O', 14),
    ('P', 13),
    ('Q', 14),
    ('R', 12),
    ('S', 12),
    ('T', 12),
    ('W', 18),
    ('X', 13),
    ('a', 11),
    ('b', 11),
    ('c', 10),
    ('d', 11),
    ('e', 11),
    ('f', 7),
    ('g', 11),
    ('h', 11),
    ('i', 5),
    ('j', 5),
    ('k', 10),
    ('l', 5),
    ('m', 18),
    ('n', 11),
    ('o', 11),
    ('p', 11),
    ('q', 11),
    ('r', 7),
    ('s', 10),
    ('t', 7),
    ('u', 11),
    ('v', 10),
    ('w', 15),
    ('x', 10),
    ('y', 9),
    ('z', 10),
    ('–', 13),
];

/// Breite je Zeichen in Pixeln bei 14 px Regular (Beschreibung), sortiert nach Zeichen.
pub(super) const DESCRIPTION_14: &[(char, u8)] = &[
    (' ', 3),
    (',', 3),
    ('.', 4),
    ('0', 8),
    ('1', 8),
    ('2', 8),
    ('3', 8),
    ('4', 8),
    ('5', 8),
    ('6', 8),
    ('7', 8),
    ('8', 8),
    ('9', 8),
    ('A', 9),
    ('B', 9),
    ('C', 9),
    ('D', 9),
    ('E', 8),
    ('F', 8),
    ('G', 11),
    ('H', 10),
    ('I', 4),
    ('L', 7),
    ('M', 11),
    ('N', 10),
    ('O', 10),
    ('P', 9),
    ('R', 9),
    ('S', 8),
    ('T', 8),
    ('U', 10),
    ('V', 9),
    ('W', 12),
    ('Y', 9),
    ('a', 8),
    ('b', 8),
    ('c', 7),
    ('d', 8),
    ('e', 7),
    ('f', 5),
    ('g', 8),
    ('h', 8),
    ('i', 3),
    ('j', 3),
    ('k', 7),
    ('l', 3),
    ('m', 12),
    ('n', 8),
    ('o', 8),
    ('p', 8),
    ('q', 8),
    ('r', 5),
    ('s', 7),
    ('t', 5),
    ('u', 8),
    ('v', 7),
    ('w', 11),
    ('x', 7),
    ('y', 7),
    ('z', 7),
];

/// Die Tabelle des Profils zur Schriftgröße (nur Regular); `None` für alle anderen Größen.
pub(super) fn table_for(size_px: f64) -> Option<&'static [(char, u8)]> {
    if size_px == TITLE_PX {
        Some(TITLE_20)
    } else if size_px == DESCRIPTION_PX {
        Some(DESCRIPTION_14)
    } else {
        None
    }
}

/// Breite des Zeichens in der Tabelle.
pub(super) fn lookup(table: &[(char, u8)], c: char) -> Option<u32> {
    table
        .binary_search_by_key(&c, |&(ch, _)| ch)
        .ok()
        .map(|i| u32::from(table[i].1))
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: UI-038
    #[test]
    fn tables_are_sorted_unique_and_plausible() {
        for (name, table) in [("20 px", TITLE_20), ("14 px", DESCRIPTION_14)] {
            assert!(
                table.windows(2).all(|w| w[0].0 < w[1].0),
                "{name}: nach Zeichen sortiert und ohne Dopplung (Binärsuche)"
            );
            assert!(table.len() >= 50, "{name}: {} Zeichen", table.len());
            assert!(table.iter().all(|&(_, w)| w >= 1), "{name}: Breite > 0");
        }
        // Ganzzahlen je Zeichen, die die Wiederholungs-Fixtures belegen
        assert_eq!(lookup(TITLE_20, 'i'), Some(5));
        assert_eq!(lookup(TITLE_20, 'W'), Some(18));
        assert_eq!(lookup(DESCRIPTION_14, 'a'), Some(8));
        assert_eq!(lookup(TITLE_20, '漢'), None);
        assert!(table_for(20.0).is_some() && table_for(14.0).is_some());
        assert!(table_for(16.0).is_none() && table_for(20.000_001).is_none());
    }
}

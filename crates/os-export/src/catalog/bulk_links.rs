//! Bulk-Exporte der Links (All Inlinks/Outlinks, Response-Codes-Inlinks; OUT-031/032/050).
//!
//! Kanonische Namen (`--bulk-export`): `Links:All Inlinks`, `Links:All Outlinks` und
//! `Response Codes:<Bereich>:<Eintrag>` mit den Bereichen `Internal & External`, `Internal`, `External`
//! und den zwölf Einträgen aus OUT-050. Der Eintrag `All` heißt hier `All Inlinks` (Alias `All` bleibt
//! gültig): `…:Internal:All` ergäbe sonst denselben Dateinamen (`response_codes_internal_all.csv`) wie der
//! Tab-Export `Response Codes:Internal All` (OUT-008, keine Kollision zweier Exporte).
//!
//! Jeder Eintrag beschreibt die Auswahl der **Ziel-URLs** ([`Selection`]); exportiert werden alle Links auf
//! diese Ziele im Standard-Linkformat. Auswahlen über Filter des Tabs „Response Codes“ lesen dessen Prädikate
//! über `views::plan_in` (eine Quelle der Wahrheit mit der Filteransicht und ihren Zählern). Einträge ohne
//! Filter in M1 (`Blocked Resource`, `Redirection (JavaScript)`) liefern die Kopfzeile ohne Zeilen (K-14).

use std::sync::OnceLock;

use os_analyze::NameError;

use crate::links_export::LinkOrder;

/// Tab, dessen Filter die Response-Codes-Exporte auswerten.
pub const RESPONSE_CODES_TAB: &str = "Response Codes";

/// Welche Ziel-URLs ein Link-Bulk-Export umfasst.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Link-level selection defined by the Links specifications.
    LinkSpec(&'static str),
    /// Alle Links, unabhängig vom Ziel (`Links:All Inlinks`/`All Outlinks`).
    All,
    /// Links auf Ziele, die in mindestens einem der Filter liegen (Vereinigung).
    Filters {
        /// Tab der Filter.
        tab: String,
        /// Kanonische Filterkennungen.
        filters: Vec<String>,
    },
    /// Links auf Ziele des Filters, die zusätzlich die SQL-Bedingung `extra` (über `u`) erfüllen.
    FilterWhere {
        /// Tab des Filters.
        tab: String,
        /// Kanonische Filterkennung (Bereich und Population).
        filter: String,
        /// Zusatzbedingung über den Alias `u`.
        extra: String,
    },
    /// Kein Filter in M1: leere Datei mit Kopfzeile (K-14).
    Empty,
}

/// Ein Eintrag des Katalogs der Link-Bulk-Exporte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkLinkEntry {
    /// Kanonischer Name, z. B. `Response Codes:Internal:Client Error (4xx) Inlinks`.
    pub name: String,
    /// Weitere gültige Schreibweisen.
    pub aliases: Vec<String>,
    /// Auswahl der Ziele.
    pub selection: Selection,
    /// Reihenfolge der Zeilen.
    pub order: LinkOrder,
    /// Der Eintrag braucht JavaScript-Rendering (K-14: in M1 ohne Zeilen).
    pub requires_rendering: bool,
}

/// Die drei Bereiche der Response-Codes-Exporte: Anzeigename im Exportnamen und Präfix der Filterkennung.
const SCOPES: [(&str, &str); 3] = [
    ("Internal & External", ""),
    ("Internal", "Internal "),
    ("External", "External "),
];

fn filter(prefix: &str, base: &str) -> String {
    format!("{prefix}{base}")
}

fn response_code_entries() -> Vec<BulkLinkEntry> {
    let mut out = Vec::new();
    for (scope, prefix) in SCOPES {
        let name = |entry: &str| format!("{RESPONSE_CODES_TAB}:{scope}:{entry}");
        let by_filter = |entry: &str, base: &str| BulkLinkEntry {
            name: name(entry),
            aliases: Vec::new(),
            selection: Selection::Filters {
                tab: RESPONSE_CODES_TAB.to_string(),
                filters: vec![filter(prefix, base)],
            },
            order: LinkOrder::Inlinks,
            requires_rendering: false,
        };
        let rendering_only = |entry: &str| BulkLinkEntry {
            name: name(entry),
            aliases: Vec::new(),
            selection: Selection::Empty,
            order: LinkOrder::Inlinks,
            requires_rendering: true,
        };

        let mut all = by_filter("All Inlinks", "All");
        all.aliases.push(name("All"));
        out.push(all);
        out.push(by_filter(
            "Blocked by Robots.txt Inlinks",
            "Blocked by Robots.txt",
        ));
        out.push(rendering_only("Blocked Resource Inlinks"));
        out.push(by_filter("No Response Inlinks", "No Response"));
        out.push(by_filter("Success (2xx) Inlinks", "Success (2XX)"));
        out.push(by_filter("Redirection (3xx) Inlinks", "Redirection (3XX)"));
        out.push(rendering_only("Redirection (JavaScript) Inlinks"));
        out.push(by_filter(
            "Redirection (Meta Refresh) Inlinks",
            "Redirection (Meta Refresh)",
        ));
        // Die Antwort 200 mit `Refresh`-Header hat in M1 keinen eigenen Filter des Tabs; die Ziele stehen
        // über `urls.redirect_type` fest (Bereich über den Filter „… All“).
        out.push(BulkLinkEntry {
            name: name("Redirection (HTTP Refresh) Inlinks"),
            aliases: Vec::new(),
            selection: Selection::FilterWhere {
                tab: RESPONSE_CODES_TAB.to_string(),
                filter: filter(prefix, "All"),
                extra: "u.redirect_type = 'http_refresh'".to_string(),
            },
            order: LinkOrder::Inlinks,
            requires_rendering: false,
        });
        out.push(by_filter(
            "Client Error (4xx) Inlinks",
            "Client Error (4XX)",
        ));
        out.push(by_filter(
            "Server Error (5xx) Inlinks",
            "Server Error (5XX)",
        ));
        out.push(super::bulk_response_codes::error_inlinks(scope, prefix));
    }
    out
}

/// Alle Einträge in Anzeigereihenfolge.
pub fn entries() -> &'static [BulkLinkEntry] {
    static ENTRIES: OnceLock<Vec<BulkLinkEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let mut v = vec![
            BulkLinkEntry {
                name: "Links:All Inlinks".to_string(),
                aliases: Vec::new(),
                selection: Selection::All,
                order: LinkOrder::Inlinks,
                requires_rendering: false,
            },
            BulkLinkEntry {
                name: "Links:All Outlinks".to_string(),
                aliases: Vec::new(),
                selection: Selection::All,
                order: LinkOrder::Outlinks,
                requires_rendering: false,
            },
        ];
        for name in [
            "All Anchor Text",
            "External Links",
            "Internal Nofollow Outlinks",
            "Internal Outlinks With No Anchor Text",
            "Non-Descriptive Anchor Text In Internal Outlinks",
            "Follow & Nofollow Internal Inlinks To Page",
            "Internal Nofollow Inlinks Only",
            "Outlinks To Localhost",
            "Pages With Uncrawlable Internal Outlinks",
            "Non-Indexable Page Inlinks Only",
        ] {
            let order = if matches!(
                name,
                "Follow & Nofollow Internal Inlinks To Page"
                    | "Internal Nofollow Inlinks Only"
                    | "Non-Indexable Page Inlinks Only"
            ) {
                LinkOrder::Inlinks
            } else {
                LinkOrder::Outlinks
            };
            // OUT-008: Die acht Tab-Filter haben bereits den Spec-Namen als Dateistamm.
            // Der Zusatz (Bulk) vermeidet doppelte Richtungsangaben; der Spec-Name bleibt als Alias gültig.
            let canonical = if matches!(name, "All Anchor Text" | "External Links") {
                format!("Links:{name}")
            } else {
                format!("Links:{name} (Bulk)")
            };
            let aliases = if canonical == format!("Links:{name}") {
                Vec::new()
            } else {
                vec![format!("Links:{name}")]
            };
            v.push(BulkLinkEntry {
                name: canonical,
                aliases,
                selection: Selection::LinkSpec(name),
                order,
                requires_rendering: false,
            });
        }
        v.extend(response_code_entries());
        v
    })
}

/// Kanonische Namen aller Einträge (für Hilfetexte und Vorschläge).
pub fn names() -> Vec<String> {
    entries().iter().map(|e| e.name.clone()).collect()
}

/// Vergleichsform: Kleinschreibung, Leerzeichen um `:` entfernt, Leerzeichenfolgen zu einem Leerzeichen.
fn normalize(name: &str) -> String {
    name.split(':')
        .map(|part| part.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join(":")
        .to_lowercase()
}

/// Sucht einen Eintrag nach kanonischem Namen oder Alias (ohne Beachtung der Groß-/Kleinschreibung).
pub fn find(name: &str) -> Option<&'static BulkLinkEntry> {
    let wanted = normalize(name);
    entries()
        .iter()
        .find(|e| normalize(&e.name) == wanted || e.aliases.iter().any(|a| normalize(a) == wanted))
}

/// Löst einen Namen auf; bei einem unbekannten Namen liefert der Fehler Vorschläge (Levenshtein ≤ 3).
pub fn resolve(name: &str) -> Result<&'static BulkLinkEntry, NameError> {
    find(name).ok_or_else(|| NameError::Unknown {
        input: name.to_string(),
        suggestions: suggestions(name),
    })
}

/// Höchster Abstand für Vorschläge.
const MAX_DISTANCE: usize = 3;
/// Höchstzahl der Vorschläge.
const MAX_SUGGESTIONS: usize = 5;

/// Ähnliche gültige Namen (Levenshtein ≤ 3, nächste zuerst).
pub fn suggestions(input: &str) -> Vec<String> {
    let wanted = normalize(input);
    let mut scored: Vec<(usize, String)> = entries()
        .iter()
        .filter_map(|e| {
            let d = levenshtein(&wanted, &normalize(&e.name));
            (d <= MAX_DISTANCE).then(|| (d, e.name.clone()))
        })
        .collect();
    scored.sort();
    scored.truncate(MAX_SUGGESTIONS);
    scored.into_iter().map(|(_, n)| n).collect()
}

/// Levenshtein-Abstand über Zeichen.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_basics() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", "abc"), 0);
    }

    #[test]
    fn normalizes_case_and_spacing() {
        assert_eq!(normalize(" Links : All   Inlinks "), "links:all inlinks");
    }
}

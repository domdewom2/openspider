//! Namensauflösung `Tab:Filter` (case-insensitiv, Leerzeichen um `:` tolerant) und dynamische Filterlabels.
//!
//! Kanonische Namen stammen aus `docs/plan/M1-filters.toml` (Anhang A). Schwellen sind dort Platzhalter
//! (`Over {title.max_chars} Characters`); bei der Eingabe passt an ihrer Stelle eine Zahl oder `X`
//! (`Over 60 Characters`, `Over X Characters`). Bei einem unbekannten Namen liefert der Fehler Vorschläge
//! mit Levenshtein-Abstand ≤ 3 (K-08).

use os_model::CrawlConfig;
use serde_json::Value as Json;

use crate::defs::{FilterDef, TabDef};
use crate::error::NameError;
use crate::tabs;

/// Höchster Levenshtein-Abstand für Vorschläge.
const MAX_DISTANCE: usize = 3;
/// Höchstzahl der Vorschläge.
const MAX_SUGGESTIONS: usize = 5;

/// Löst `"Tab:Filter"` in die kanonischen Namen `(tab, filter)` auf. Der Filter wird mit seiner
/// kanonischen Kennung zurückgegeben (Platzhalter unaufgelöst, z. B. `Over {title.max_chars} Characters`).
pub fn resolve(input: &str) -> Result<(String, String), NameError> {
    resolve_in(&tabs::all(), input)
}

/// Wie [`resolve`], gegen eine beliebige Tab-Liste (Tests, abweichende Registry).
pub fn resolve_in(tabs: &[TabDef], input: &str) -> Result<(String, String), NameError> {
    if let Some((tab_part, filter_part)) = input.split_once(':') {
        if let Some(tab) = find_tab(tabs, tab_part) {
            if let Some(filter) = find_filter(tab, filter_part) {
                return Ok((tab.id.to_string(), filter.id.to_string()));
            }
        }
    }
    Err(NameError::Unknown {
        input: input.to_string(),
        suggestions: suggestions(tabs, input),
    })
}

/// Sucht einen Tab nach Namen (case-insensitiv, Randleerzeichen ignoriert).
pub fn find_tab<'a>(tabs: &'a [TabDef], name: &str) -> Option<&'a TabDef> {
    let wanted = normalize(name);
    tabs.iter().find(|t| normalize(t.id) == wanted)
}

/// Sucht einen Filter eines Tabs nach seiner kanonischen Kennung oder einem Namen, in dem Platzhalter
/// durch eine Zahl oder `X` ersetzt sind (case-insensitiv).
pub fn find_filter<'a>(tab: &'a TabDef, name: &str) -> Option<&'a FilterDef> {
    let wanted = normalize(name);
    if let Some(f) = tab.filters.iter().find(|f| normalize(f.id) == wanted) {
        return Some(f);
    }
    tab.filters.iter().find(|f| matches_pattern(f.id, &wanted))
}

/// Anzeigename eines Filters: Platzhalter `{pfad}` werden durch den aktuellen Konfigurationswert ersetzt
/// (ONPAGE-002). Unbekannte Pfade bleiben unverändert stehen.
pub fn label(filter_id: &str, cfg: &CrawlConfig) -> String {
    let json = serde_json::to_value(cfg).unwrap_or(Json::Null);
    label_with(filter_id, &json)
}

/// Wie [`label`], mit bereits in JSON gewandelter Konfiguration (vermeidet wiederholtes Serialisieren).
pub fn label_with(filter_id: &str, cfg_json: &Json) -> String {
    let mut out = String::with_capacity(filter_id.len());
    let mut rest = filter_id;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        out.push_str(&rest[..open]);
        let path = &rest[open + 1..open + close];
        match config_value(cfg_json, path) {
            Some(Json::Number(n))
                if matches!(
                    path,
                    "validation.html_doc_max_bytes" | "validation.resource_max_bytes"
                ) =>
            {
                out.push_str(&(n.as_f64().unwrap_or_default() / 1_048_576.0).to_string());
            }
            Some(Json::Number(n)) => out.push_str(&n.to_string()),
            Some(Json::String(s)) => out.push_str(s),
            Some(Json::Bool(b)) => out.push_str(&b.to_string()),
            _ => out.push_str(&rest[open..open + close + 1]),
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

/// Wert unter einem Konfigurationspfad (`title.max_chars`) in der JSON-Fassung der Konfiguration.
pub fn config_value<'a>(cfg_json: &'a Json, path: &str) -> Option<&'a Json> {
    let mut cur = cfg_json;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

/// Kleinschreibung, Randleerzeichen entfernt, innere Leerzeichenfolgen zu einem Leerzeichen.
fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Bestandteile einer Filterkennung: Text oder Platzhalter.
enum Piece {
    Literal(String),
    Placeholder,
}

fn pieces(id: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut rest = id;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        if open > 0 {
            out.push(Piece::Literal(normalize_keep_edges(&rest[..open])));
        }
        out.push(Piece::Placeholder);
        rest = &rest[open + close + 1..];
    }
    if !rest.is_empty() {
        out.push(Piece::Literal(normalize_keep_edges(rest)));
    }
    out
}

/// Wie [`normalize`], aber Rand-Leerzeichen bleiben als je ein Leerzeichen erhalten (an Platzhaltern wichtig).
fn normalize_keep_edges(s: &str) -> String {
    let lead = s.starts_with(char::is_whitespace);
    let trail = s.ends_with(char::is_whitespace);
    let core = normalize(s);
    let mut out = String::new();
    if lead {
        out.push(' ');
    }
    out.push_str(&core);
    if trail && !core.is_empty() {
        out.push(' ');
    }
    out
}

/// Passt `wanted` (bereits normalisiert) auf die Kennung `id` mit Platzhaltern als Zahl oder `x`?
fn matches_pattern(id: &str, wanted: &str) -> bool {
    if !id.contains('{') {
        return false;
    }
    let pieces = pieces(id);
    match_pieces(&pieces, wanted)
}

fn match_pieces(pieces: &[Piece], input: &str) -> bool {
    match pieces.split_first() {
        None => input.is_empty(),
        Some((Piece::Literal(lit), rest)) => input
            .strip_prefix(lit.as_str())
            .is_some_and(|tail| match_pieces(rest, tail)),
        Some((Piece::Placeholder, rest)) => {
            if let Some(tail) = input.strip_prefix('x') {
                if match_pieces(rest, tail) {
                    return true;
                }
            }
            let digits = input.chars().take_while(|c| c.is_ascii_digit()).count();
            (1..=digits).any(|n| match_pieces(rest, &input[n..]))
        }
    }
}

/// Alle gültigen Namen `Tab:Filter` in der Form für Vorschläge (Platzhalter als `X`).
fn candidates(tabs: &[TabDef]) -> Vec<String> {
    let mut out = Vec::new();
    for tab in tabs {
        for filter in &tab.filters {
            out.push(format!("{}:{}", tab.id, placeholders_to_x(filter.id)));
        }
    }
    out
}

fn placeholders_to_x(id: &str) -> String {
    let mut out = String::new();
    let mut rest = id;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push('X');
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

/// Ziffernfolgen durch `x` ersetzen (für den Abstand zu Namen mit Platzhalter).
fn digits_to_x(s: &str) -> String {
    let mut out = String::new();
    let mut in_digits = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                out.push('x');
            }
            in_digits = true;
        } else {
            in_digits = false;
            out.push(c);
        }
    }
    out
}

/// Vorschläge für eine nicht auflösbare Eingabe: Namen mit Levenshtein-Abstand ≤ 3, nächste zuerst.
pub fn suggestions(tabs: &[TabDef], input: &str) -> Vec<String> {
    let (tab_part, filter_part) = match input.split_once(':') {
        Some((t, f)) => (normalize(t), Some(normalize(f))),
        None => (normalize(input), None),
    };
    let mut scored: Vec<(usize, String)> = Vec::new();

    match filter_part {
        Some(filter_part) => {
            let joined = format!("{tab_part}:{filter_part}");
            let joined_x = digits_to_x(&joined);
            for cand in candidates(tabs) {
                let lower = cand.to_lowercase();
                let d = strsim::levenshtein(&joined, &lower)
                    .min(strsim::levenshtein(&joined_x, &lower));
                if d <= MAX_DISTANCE {
                    scored.push((d, cand));
                }
            }
        }
        None => {
            // Nur ein Tab-Name: den nächsten Tabs ihren ersten Filter anhängen.
            for tab in tabs {
                let d = strsim::levenshtein(&tab_part, &tab.id.to_lowercase());
                if d <= MAX_DISTANCE {
                    if let Some(first) = tab.filters.first() {
                        scored.push((d, format!("{}:{}", tab.id, placeholders_to_x(first.id))));
                    }
                }
            }
        }
    }
    scored.sort();
    scored.dedup_by(|a, b| a.1 == b.1);
    scored
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, name)| name)
        .collect()
}

/// Vorschläge für einen unbekannten Spaltennamen (Levenshtein ≤ 3, case-insensitiv).
pub fn column_suggestions<'a>(names: impl Iterator<Item = &'a str>, input: &str) -> Vec<String> {
    let wanted = normalize(input);
    let mut scored: Vec<(usize, String)> = names
        .filter_map(|n| {
            let d = strsim::levenshtein(&wanted, &n.to_lowercase());
            (d <= MAX_DISTANCE).then(|| (d, n.to_string()))
        })
        .collect();
    scored.sort();
    scored.dedup_by(|a, b| a.1 == b.1);
    scored
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, n)| n)
        .collect()
}

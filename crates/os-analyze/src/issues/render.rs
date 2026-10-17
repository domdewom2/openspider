//! Textauflösung: Rohtexte aus `core/issues/texts`, Platzhalterersetzung, Doku-Seiten.

use std::collections::HashMap;
use std::sync::OnceLock;

use os_model::{CrawlConfig, Value};

use super::{CatalogEntry, IssueTexts, Lang};

type TextMap = HashMap<String, IssueTexts>;

fn parse_texts(src: &str) -> TextMap {
    let table: toml::Table = match src.parse() {
        Ok(t) => t,
        Err(e) => panic!("core/issues/texts ist kein gültiges TOML: {e}"),
    };
    let mut map = TextMap::new();
    let Some(issues) = table.get("issues").and_then(|v| v.as_table()) else {
        return map;
    };
    for (id, v) in issues {
        let Some(t) = v.as_table() else { continue };
        let get = |k: &str| t.get(k).and_then(|x| x.as_str()).map(str::to_string);
        if let (Some(what), Some(why), Some(fix)) = (get("what"), get("why"), get("fix")) {
            map.insert(id.clone(), IssueTexts { what, why, fix });
        }
    }
    map
}

/// Ungefüllte Texte (mit Platzhaltern) zu einer Katalog-`id`.
pub(crate) fn raw_texts(id: &str, lang: Lang) -> Option<IssueTexts> {
    static DE: OnceLock<TextMap> = OnceLock::new();
    static EN: OnceLock<TextMap> = OnceLock::new();
    let cell = match lang {
        Lang::De => &DE,
        Lang::En => &EN,
    };
    cell.get_or_init(|| parse_texts(super::texts_source(lang)))
        .get(id)
        .cloned()
}

fn default_table() -> &'static toml::Table {
    static T: OnceLock<toml::Table> = OnceLock::new();
    T.get_or_init(|| {
        CrawlConfig::default()
            .to_toml_string()
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default()
    })
}

fn scalar_text(v: &toml::Value) -> Option<String> {
    match v {
        toml::Value::Integer(i) => Some(i.to_string()),
        toml::Value::Float(f) => Some(float_text(*f)),
        toml::Value::String(s) => Some(s.clone()),
        toml::Value::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Standardwert eines Konfigurationspfads (`title.max_chars`) als Text.
pub(crate) fn default_value(path: &str) -> Option<String> {
    let mut parts = path.split('.');
    let mut cur = default_table().get(parts.next()?)?;
    for p in parts {
        cur = cur.as_table()?.get(p)?;
    }
    if matches!(
        path,
        "validation.html_doc_max_bytes" | "validation.resource_max_bytes"
    ) {
        return cur.as_integer().map(|v| float_text(v as f64 / 1_048_576.0));
    }
    scalar_text(cur)
}

fn float_text(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

fn norm(name: &str) -> String {
    name.trim_start_matches(':').replace('.', "_")
}

/// Wert eines Konfigurationspfads: erst aus `params`, sonst der Standardwert.
fn resolve(path: &str, params: &[(&str, Value)]) -> Option<String> {
    let want = norm(path);
    for (name, value) in params {
        if norm(name) == want {
            match value {
                Value::Int(i)
                    if matches!(
                        path,
                        "validation.html_doc_max_bytes" | "validation.resource_max_bytes"
                    ) =>
                {
                    return Some(float_text(*i as f64 / 1_048_576.0))
                }
                Value::Int(i) => return Some(i.to_string()),
                Value::Float(f) => return Some(float_text(*f)),
                Value::Text(s) => return Some(s.clone()),
                Value::Bool(b) => return Some(b.to_string()),
                Value::Null => {}
            }
        }
    }
    default_value(path)
}

/// Ersetzt die Platzhalter eines Eintrags in den Texten.
pub(crate) fn fill(entry: &CatalogEntry, raw: &IssueTexts, params: &[(&str, Value)]) -> IssueTexts {
    let values: Vec<(String, String)> = entry
        .text_params
        .iter()
        .filter_map(|(ph, path)| Some((format!("{{{ph}}}"), resolve(path, params)?)))
        .collect();
    let apply = |s: &str| {
        values
            .iter()
            .fold(s.to_string(), |acc, (ph, v)| acc.replace(ph.as_str(), v))
    };
    IssueTexts {
        what: apply(&raw.what),
        why: apply(&raw.why),
        fix: apply(&raw.fix),
    }
}

/// Konfigurationspfade, die ein Filtername als `{pfad}` nennt.
pub(crate) fn filter_paths(filter: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = filter;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else {
            break;
        };
        out.push(rest[start + 1..start + len].to_string());
        rest = &rest[start + len + 1..];
    }
    out
}

/// Filtername des Katalogs mit aufgelösten Schwellen (`Over {title.max_chars} Characters` →
/// `Over 60 Characters`); Werte aus `params`, sonst die Standardwerte.
pub fn filter_label(entry: &CatalogEntry, params: &[(&str, Value)]) -> String {
    let mut out = String::new();
    let mut rest = entry.filter.as_str();
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else {
            break;
        };
        out.push_str(&rest[..start]);
        let path = &rest[start + 1..start + len];
        match resolve(path, params) {
            Some(v) => out.push_str(&v),
            None => out.push_str(&rest[start..start + len + 1]),
        }
        rest = &rest[start + len + 1..];
    }
    out.push_str(rest);
    out
}

/// `true`, wenn `actual` zum Katalog-Filternamen `pattern` passt: gleich (ohne Beachtung der
/// Groß-/Kleinschreibung) oder gleich nach Einsetzen beliebiger nichtleerer Werte für die `{…}`-Platzhalter.
pub fn matches_filter(pattern: &str, actual: &str) -> bool {
    let pat = pattern.to_lowercase();
    let act = actual.to_lowercase();
    if pat == act {
        return true;
    }
    // Literale zwischen den Platzhaltern: n Platzhalter ergeben n + 1 Literale.
    let mut literals: Vec<&str> = Vec::new();
    let mut rest = pat.as_str();
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else {
            return false;
        };
        literals.push(&rest[..start]);
        rest = &rest[start + len + 1..];
    }
    if literals.is_empty() {
        return false;
    }
    literals.push(rest);
    let (first, last) = (literals[0], literals[literals.len() - 1]);
    if act.len() < first.len() + last.len() || !act.starts_with(first) || !act.ends_with(last) {
        return false;
    }
    let body = &act[first.len()..act.len() - last.len()];
    let mut pos = 0;
    // Jeder Platzhalter steht für mindestens ein Zeichen; die mittleren Literale folgen der Reihe nach.
    for lit in &literals[1..literals.len() - 1] {
        let Some(skip) = body[pos..].chars().next().map(char::len_utf8) else {
            return false;
        };
        match body[pos + skip..].find(lit) {
            Some(idx) => pos += skip + idx + lit.len(),
            None => return false,
        }
    }
    pos < body.len()
}

/// Doku-Seite zu einem Eintrag (`docs/issues/<doc_key>.md`): Titel, Klassifizierung und die drei Texte in
/// Deutsch und Englisch, Platzhalter mit den Standardwerten.
pub fn doc_page(entry: &CatalogEntry) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# {}: {}\n\n",
        entry.tab,
        filter_label(entry, &[])
    ));
    out.push_str(&format!(
        "Typ: {} ({}) · Priorität: {} ({})\n",
        entry.issue_type, entry.type_rule, entry.priority, entry.priority_rule
    ));
    if let Some(reason) = &entry.override_reason {
        out.push_str(&format!("\nAbweichung von der Herleitung: {reason}\n"));
    }
    if entry.needs_crawl_analysis {
        out.push_str("\nBefüllt erst nach der Crawl-Analyse.\n");
    }
    for (lang, title, what, why, fix) in [
        (
            Lang::De,
            "Deutsch",
            "Was bedeutet das?",
            "Warum ist es relevant?",
            "Wie beheben?",
        ),
        (
            Lang::En,
            "English",
            "What does it mean?",
            "Why does it matter?",
            "How to fix",
        ),
    ] {
        out.push_str(&format!("\n## {title}\n\n"));
        match super::texts(&entry.doc_key, lang, &[]) {
            Some(t) => {
                out.push_str(&format!("**{what}** {}\n\n", t.what));
                out.push_str(&format!("**{why}** {}\n\n", t.why));
                out.push_str(&format!("**{fix}** {}\n", t.fix));
            }
            None => out.push_str("(Text fehlt)\n"),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: UI-063
    #[test]
    fn matches_filter_handles_placeholders() {
        let p = "Over {title.max_chars} Characters";
        assert!(matches_filter(p, "Over {title.max_chars} Characters"));
        assert!(matches_filter(p, "Over 60 Characters"));
        assert!(matches_filter(p, "over 1000 characters"));
        assert!(!matches_filter(p, "Over  Characters"));
        assert!(!matches_filter(p, "Below 60 Characters"));
        assert!(!matches_filter(p, "Over 60 Chars"));
        assert!(matches_filter(
            "Alt Text Over {image.alt_max_chars} Characters",
            "Alt Text Over 125 Characters"
        ));
        assert!(matches_filter("Over {image.max_kb} KB", "Over 100 KB"));
        assert!(!matches_filter("Missing", "Duplicate"));
        // Mehrere Platzhalter.
        assert!(matches_filter("A {x} B {y} C", "A 1 B 2 C"));
        assert!(!matches_filter("A {x} B {y} C", "A 1 B  C"));
    }

    // spec: UI-063
    #[test]
    fn default_values_come_from_the_default_configuration() {
        assert_eq!(default_value("title.max_chars").as_deref(), Some("60"));
        assert_eq!(
            default_value("links.high_crawl_depth").as_deref(),
            Some("4")
        );
        assert_eq!(default_value("no.such.key"), None);
    }
}

//! Paging (`limit` 1–500, Standard 50) und Antwortgrenze von 50 000 Zeichen (AGENT-005).
//!
//! * [`validate_args`] prüft `limit` und `cursor` jedes Werkzeugaufrufs zentral im Server; die Werkzeuge
//!   lesen den Wert über [`parse_limit`] bzw. [`PageArgs::from_args`] (Standard 50).
//! * [`encode_cursor`]/[`decode_cursor`] bauen undurchsichtige Cursor für Listen ohne View-Cursor
//!   (`list_crawls`, Inlinks/Outlinks, `list_issues`). Der Cursor trägt eine Bindung (Crawl, Abfrage,
//!   Sortierung); ein Cursor mit anderer Bindung wird abgelehnt. Ansichten (`query_urls`) nutzen den
//!   Cursor aus `os_analyze::cursor`.
//! * [`shape`] kürzt Antworten über der Zeichenobergrenze und hängt den Hinweis auf den Export
//!   (Werkzeugklasse `write`) an.

use serde_json::Value;

use crate::tool::ToolError;

/// Standard für `limit`.
pub const DEFAULT_LIMIT: u32 = 50;
/// Kleinster erlaubter Wert für `limit`.
pub const MIN_LIMIT: u32 = 1;
/// Größter erlaubter Wert für `limit`.
pub const MAX_LIMIT: u32 = 500;
/// Obergrenze der Zeichen einer Antwort (Summe aller Textblöcke).
pub const MAX_RESPONSE_CHARS: usize = 50_000;

/// Paging-Parameter eines Aufrufs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageArgs {
    /// Zeilen je Seite, 1–500 (Standard 50).
    pub limit: u32,
    /// Cursor der Fortsetzung (`None` = erste Seite).
    pub cursor: Option<String>,
}

impl PageArgs {
    /// Liest `limit` und `cursor` aus den Argumenten.
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`], wenn `limit` keine ganze Zahl von 1 bis 500 oder `cursor` keine
    /// nichtleere Zeichenkette ist.
    pub fn from_args(args: &Value) -> Result<PageArgs, ToolError> {
        Ok(PageArgs {
            limit: parse_limit(args)?,
            cursor: parse_cursor(args)?.map(str::to_owned),
        })
    }
}

/// Liest `limit` (fehlt oder `null` = 50).
///
/// # Errors
/// [`ToolError::InvalidArguments`] mit dem gültigen Bereich, wenn der Wert keine ganze Zahl von 1 bis 500 ist.
pub fn parse_limit(args: &Value) -> Result<u32, ToolError> {
    let invalid = |shown: String| {
        ToolError::InvalidArguments(format!(
            "limit muss eine ganze Zahl von {MIN_LIMIT} bis {MAX_LIMIT} sein (Standard {DEFAULT_LIMIT}), \
             erhalten: {shown}"
        ))
    };
    match args.get("limit") {
        None | Some(Value::Null) => Ok(DEFAULT_LIMIT),
        Some(v) => {
            let n = v.as_u64().ok_or_else(|| invalid(v.to_string()))?;
            u32::try_from(n)
                .ok()
                .filter(|n| (MIN_LIMIT..=MAX_LIMIT).contains(n))
                .ok_or_else(|| invalid(n.to_string()))
        }
    }
}

/// Liest `cursor` (fehlt oder `null` = erste Seite).
///
/// # Errors
/// [`ToolError::InvalidArguments`], wenn `cursor` keine nichtleere Zeichenkette ist.
pub fn parse_cursor(args: &Value) -> Result<Option<&str>, ToolError> {
    match args.get("cursor") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if !s.is_empty() => Ok(Some(s)),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "cursor muss eine nichtleere Zeichenkette sein (den Wert next_cursor der vorigen Antwort), \
             erhalten: {other}"
        ))),
    }
}

/// Zentrale Prüfung vor jedem Werkzeugaufruf: gültiges `limit` und `cursor`, falls angegeben.
///
/// # Errors
/// [`ToolError::InvalidArguments`] wie bei [`parse_limit`]/[`parse_cursor`].
pub fn validate_args(args: &Value) -> Result<(), ToolError> {
    parse_limit(args)?;
    parse_cursor(args)?;
    Ok(())
}

const CURSOR_VERSION: u64 = 1;

/// Baut einen undurchsichtigen Cursor für `offset`, gebunden an `binding` (z. B. Werkzeug, Crawl, Filter
/// und Sortierung als Zeichenkette).
pub fn encode_cursor(binding: &str, offset: u64) -> String {
    let json = serde_json::json!({ "v": CURSOR_VERSION, "b": binding, "o": offset });
    hex_encode(json.to_string().as_bytes())
}

/// Liest den Versatz eines Cursors, der mit [`encode_cursor`] und derselben `binding` entstand.
///
/// # Errors
/// [`ToolError::InvalidArguments`], wenn der Cursor unlesbar ist oder zu einer anderen Abfrage gehört.
pub fn decode_cursor(cursor: &str, binding: &str) -> Result<u64, ToolError> {
    let foreign = || {
        ToolError::InvalidArguments(
            "Der Cursor gehört nicht zu dieser Abfrage (anderer Crawl, anderes Werkzeug, Filter oder \
             andere Sortierung); beginnen Sie ohne cursor von vorn."
                .to_string(),
        )
    };
    let bytes = hex_decode(cursor).ok_or_else(foreign)?;
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| foreign())?;
    let version_ok = v.get("v").and_then(Value::as_u64) == Some(CURSOR_VERSION);
    let binding_ok = v.get("b").and_then(Value::as_str) == Some(binding);
    match (version_ok && binding_ok, v.get("o").and_then(Value::as_u64)) {
        (true, Some(offset)) => Ok(offset),
        _ => Err(foreign()),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    out
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    s.as_bytes()
        .chunks(2)
        .map(|pair| {
            let hi = char::from(pair[0]).to_digit(16)?;
            let lo = char::from(pair[1]).to_digit(16)?;
            u8::try_from(hi * 16 + lo).ok()
        })
        .collect()
}

/// Anzahl Zeichen (nicht Bytes).
pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Schneidet nach `max` Zeichen ab (an einer Zeichengrenze).
pub fn truncate_chars(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// Ergebnis von [`shape`]: Textteile einer Antwort, ggf. gekürzt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shaped {
    /// Zusammenfassung (JSON-Text), bei Kürzung ggf. abgeschnitten.
    pub summary: String,
    /// Nutzdaten des Untrusted-Blocks (JSON-Text), bei Kürzung abgeschnitten oder entfallen.
    pub untrusted: Option<String>,
    /// Hinweis auf die Kürzung samt Exportangebot; `None`, wenn nichts gekürzt wurde.
    pub notice: Option<String>,
}

/// Hinweistext einer gekürzten Antwort mit Angebot, als Datei zu exportieren (Klasse `write`).
pub fn truncation_notice(total_chars: usize, max_chars: usize, write_enabled: bool) -> String {
    let export = if write_enabled {
        "als Datei exportieren (Werkzeug der Klasse write)"
    } else {
        "als Datei exportieren – dafür ist ein Werkzeug der Klasse write nötig, das nicht freigegeben ist \
         (--allow write beim Start oder mcp.toml)"
    };
    format!(
        "[Antwort gekürzt: {total_chars} Zeichen sind mehr als die Obergrenze von {max_chars}. Für die \
         vollständigen Daten kleinere Seiten (limit), weniger Spalten oder engere Filter wählen oder {export}.]"
    )
}

/// Hält eine Antwort unter `max_chars` Zeichen (Summe von Zusammenfassung, Untrusted-Block samt
/// `untrusted_overhead` für Präambel und Marken und Hinweis).
///
/// Passt alles hinein, bleibt die Antwort unverändert. Sonst entfällt zuerst Untrusted-Text, danach wird die
/// Zusammenfassung gekürzt; der Hinweis kommt immer dazu und zählt mit.
pub fn shape(
    summary: String,
    untrusted: Option<String>,
    untrusted_overhead: usize,
    max_chars: usize,
    write_enabled: bool,
) -> Shaped {
    let overhead = if untrusted.is_some() {
        untrusted_overhead
    } else {
        0
    };
    let total = char_len(&summary) + untrusted.as_deref().map_or(0, char_len) + overhead;
    if total <= max_chars {
        return Shaped {
            summary,
            untrusted,
            notice: None,
        };
    }
    let notice = truncation_notice(total, max_chars, write_enabled);
    let budget = max_chars.saturating_sub(char_len(&notice));
    let summary_len = char_len(&summary);
    if summary_len >= budget {
        return Shaped {
            summary: truncate_chars(&summary, budget).to_string(),
            untrusted: None,
            notice: Some(notice),
        };
    }
    let data_budget = (budget - summary_len).saturating_sub(untrusted_overhead);
    let untrusted = untrusted
        .filter(|_| data_budget > 0)
        .map(|u| truncate_chars(&u, data_budget).to_string());
    Shaped {
        summary,
        untrusted,
        notice: Some(notice),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // spec: AGENT-005
    #[test]
    fn limit_defaults_to_50_and_accepts_1_to_500() {
        assert_eq!(parse_limit(&json!({})).unwrap(), 50);
        assert_eq!(parse_limit(&json!({"limit": null})).unwrap(), 50);
        assert_eq!(parse_limit(&json!({"limit": 1})).unwrap(), 1);
        assert_eq!(parse_limit(&json!({"limit": 5})).unwrap(), 5);
        assert_eq!(parse_limit(&json!({"limit": 500})).unwrap(), 500);
    }

    // spec: AGENT-005
    #[test]
    fn limit_outside_range_or_wrong_type_is_an_error_naming_the_range() {
        for bad in [
            json!(0),
            json!(501),
            json!(-1),
            json!(5.5),
            json!("5"),
            json!(true),
            json!(4_294_967_297_u64),
        ] {
            let err = parse_limit(&json!({ "limit": bad.clone() })).unwrap_err();
            let ToolError::InvalidArguments(msg) = err else {
                panic!("falscher Fehlertyp für {bad}");
            };
            assert!(msg.contains("1") && msg.contains("500"), "{bad}: {msg}");
        }
    }

    // spec: AGENT-005
    #[test]
    fn cursor_argument_must_be_a_nonempty_string() {
        assert_eq!(parse_cursor(&json!({})).unwrap(), None);
        assert_eq!(parse_cursor(&json!({"cursor": null})).unwrap(), None);
        assert_eq!(
            parse_cursor(&json!({"cursor": "abc"})).unwrap(),
            Some("abc")
        );
        assert!(parse_cursor(&json!({"cursor": ""})).is_err());
        assert!(parse_cursor(&json!({"cursor": 7})).is_err());
        let page = PageArgs::from_args(&json!({"limit": 7, "cursor": "x"})).unwrap();
        assert_eq!((page.limit, page.cursor.as_deref()), (7, Some("x")));
        assert!(validate_args(&json!({"limit": 0})).is_err());
        assert!(validate_args(&json!({"limit": 20, "cursor": "x"})).is_ok());
    }

    // spec: AGENT-005
    #[test]
    fn cursor_roundtrips_and_rejects_foreign_bindings() {
        let c = encode_cursor("crawl-a|Internal:All|", 40);
        assert!(
            c.bytes().all(|b| b.is_ascii_hexdigit()),
            "undurchsichtig: {c}"
        );
        assert_eq!(decode_cursor(&c, "crawl-a|Internal:All|").unwrap(), 40);
        for bad_binding in ["crawl-b|Internal:All|", "crawl-a|Internal:HTML|", ""] {
            let err = decode_cursor(&c, bad_binding).unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidArguments(_)),
                "{bad_binding}"
            );
        }
        for junk in [
            "",
            "zz",
            "abc",
            "00",
            "é",
            &hex_encode(b"{\"v\":2,\"b\":\"x\",\"o\":1}"),
        ] {
            assert!(decode_cursor(junk, "x").is_err(), "{junk}");
        }
    }

    // spec: AGENT-005
    #[test]
    fn small_responses_pass_unchanged() {
        let shaped = shape("{\"a\":1}".into(), Some("[1]".into()), 120, 50_000, false);
        assert_eq!(shaped.notice, None);
        assert_eq!(shaped.summary, "{\"a\":1}");
        assert_eq!(shaped.untrusted.as_deref(), Some("[1]"));
    }

    // spec: AGENT-005
    #[test]
    fn oversized_untrusted_data_is_cut_and_total_stays_under_the_limit() {
        let data = "ä".repeat(60_000);
        let shaped = shape("{\"total\":1}".into(), Some(data), 100, 50_000, false);
        let notice = shaped.notice.clone().expect("Hinweis");
        assert!(notice.contains("write"), "{notice}");
        assert!(notice.contains("Datei"), "{notice}");
        let total = char_len(&shaped.summary)
            + shaped.untrusted.as_deref().map_or(0, char_len)
            + 100
            + char_len(&notice);
        assert!(total <= 50_000, "{total}");
        assert!(total > 49_000, "es wird nicht zu viel verworfen: {total}");
        assert_eq!(shaped.summary, "{\"total\":1}");
    }

    // spec: AGENT-005
    #[test]
    fn oversized_summary_is_cut_and_untrusted_dropped() {
        let shaped = shape("x".repeat(70_000), Some("data".into()), 100, 50_000, true);
        assert!(shaped.untrusted.is_none());
        let notice = shaped.notice.clone().unwrap();
        assert!(
            !notice.contains("nicht freigegeben"),
            "write ist freigegeben: {notice}"
        );
        assert!(char_len(&shaped.summary) + char_len(&notice) <= 50_000);
    }

    // spec: AGENT-005
    #[test]
    fn truncation_respects_char_boundaries() {
        assert_eq!(truncate_chars("äöü", 2), "äö");
        assert_eq!(truncate_chars("abc", 10), "abc");
        assert_eq!(truncate_chars("abc", 0), "");
    }
}

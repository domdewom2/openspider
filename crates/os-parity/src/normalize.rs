//! Normalisierer und Zellvergleich (Abschnitt 5.2: `[normalize]` und Spaltenoptionen).

use std::collections::BTreeMap;

/// Basis-URLs einer Seite: `{BASE}` (Hauptsite) und `{EXT}` (derselbe Port unter dem anderen
/// Loopback-Namen, F-HOST2: `127.0.0.1` ↔ `localhost`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BaseUrls {
    base: Option<String>,
    ext: Option<String>,
}

impl BaseUrls {
    /// Leitet `{BASE}` und `{EXT}` aus der Basis-URL der Site ab (z. B. `http://127.0.0.1:8902/`).
    pub fn from_base(base: &str) -> Self {
        let base = base.trim().trim_end_matches('/');
        let Some((scheme, rest)) = base.split_once("://") else {
            return Self::default();
        };
        let authority = rest.split('/').next().unwrap_or("");
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => (h, Some(p)),
            _ => (authority, None),
        };
        let ext_host = match host.to_ascii_lowercase().as_str() {
            "127.0.0.1" => Some("localhost"),
            "localhost" => Some("127.0.0.1"),
            _ => None,
        };
        let build = |h: &str| match port {
            Some(p) => format!("{scheme}://{h}:{p}"),
            None => format!("{scheme}://{h}"),
        };
        Self {
            base: Some(build(host)),
            ext: ext_host.map(build),
        }
    }

    /// Ersetzt die Ursprünge durch `{BASE}` bzw. `{EXT}`.
    pub fn normalize(&self, value: &str) -> String {
        let mut out = value.to_string();
        if let Some(b) = &self.base {
            out = replace_origin(&out, b, "{BASE}");
        }
        if let Some(e) = &self.ext {
            out = replace_origin(&out, e, "{EXT}");
        }
        out
    }
}

/// Ersetzt `origin` (ohne Beachtung der Groß-/Kleinschreibung) durch `token`, sofern danach kein
/// Zeichen folgt, das den Ursprung verlängern würde (weitere Portziffern, Host- oder Portfortsetzung).
fn replace_origin(value: &str, origin: &str, token: &str) -> String {
    let hay = value.to_ascii_lowercase();
    let needle = origin.to_ascii_lowercase();
    let mut out = String::with_capacity(value.len());
    let mut pos = 0;
    while let Some(found) = hay[pos..].find(&needle) {
        let start = pos + found;
        let end = start + needle.len();
        let next = hay[end..].chars().next();
        let extends = matches!(next, Some(c) if c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '-' | '_'));
        out.push_str(&value[pos..start]);
        if extends {
            out.push_str(&value[start..end]);
        } else {
            out.push_str(token);
        }
        pos = end;
    }
    out.push_str(&value[pos..]);
    out
}

/// Wie Wahrheitswerte verglichen werden.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BoolMode {
    /// Keine Sonderbehandlung.
    #[default]
    Off,
    /// `true`/`false` ohne Beachtung der Schreibweise.
    CaseInsensitive,
    /// Zusätzlich `yes/no`, `1/0`, `ja/nein`, `wahr/falsch`.
    Loose,
}

/// Aufgelöste Vergleichsregeln einer Spalte.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CellRules {
    /// Zellen vor dem Vergleich trimmen.
    pub trim: bool,
    /// Basis-URLs zu `{BASE}`/`{EXT}` normalisieren.
    pub base_url: bool,
    /// Wahrheitswerte.
    pub bool_mode: BoolMode,
    /// Zahlen auf diese Nachkommastellen runden.
    pub number_decimals: Option<u32>,
    /// Relative Toleranz in Prozent (bezogen auf den Referenzwert).
    pub tolerance_pct: Option<f64>,
    /// Leer und `null` sind gleich.
    pub empty_equals_null: bool,
    /// Wertabbildung Referenz → OpenSpider.
    pub map: BTreeMap<String, String>,
}

impl CellRules {
    /// Bereitet einen Rohwert für den Vergleich auf (Trimmen, Basis-URLs).
    pub fn prepare(&self, raw: &str, bases: &BaseUrls) -> String {
        let v = if self.trim { raw.trim() } else { raw };
        if self.base_url {
            bases.normalize(v)
        } else {
            v.to_string()
        }
    }

    /// Wendet die Wertabbildung auf den (aufbereiteten) Referenzwert an.
    pub fn map_reference(&self, prepared: &str) -> String {
        self.map
            .get(prepared)
            .cloned()
            .unwrap_or_else(|| prepared.to_string())
    }

    /// Vergleicht zwei aufbereitete Werte (Referenz schon abgebildet).
    pub fn equal(&self, reference: &str, actual: &str) -> bool {
        if self.empty_equals_null && is_nullish(reference) && is_nullish(actual) {
            return true;
        }
        if reference == actual {
            return true;
        }
        if self.bool_mode != BoolMode::Off {
            if let (Some(a), Some(b)) = (
                canonical_bool(reference, self.bool_mode),
                canonical_bool(actual, self.bool_mode),
            ) {
                if a == b {
                    return true;
                }
            }
        }
        if let (Some(a), Some(b)) = (parse_number(reference), parse_number(actual)) {
            if let Some(pct) = self.tolerance_pct {
                return within_tolerance(a, b, pct);
            }
            if let Some(d) = self.number_decimals {
                return round_to(a, d) == round_to(b, d);
            }
            return a == b;
        }
        false
    }
}

fn is_nullish(v: &str) -> bool {
    v.is_empty() || v.eq_ignore_ascii_case("null")
}

fn canonical_bool(v: &str, mode: BoolMode) -> Option<bool> {
    let l = v.to_lowercase();
    match l.as_str() {
        "true" => Some(true),
        "false" => Some(false),
        "yes" | "1" | "ja" | "wahr" if mode == BoolMode::Loose => Some(true),
        "no" | "0" | "nein" | "falsch" if mode == BoolMode::Loose => Some(false),
        _ => None,
    }
}

/// Einfache Dezimalzahl: optionales Vorzeichen, Ziffern, optional `.Ziffern`; kein Exponent, kein `inf`/`NaN`.
pub fn parse_number(v: &str) -> Option<f64> {
    let t = v.trim();
    let digits = t.strip_prefix(['-', '+']).unwrap_or(t);
    let (int, frac) = match digits.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (digits, None),
    };
    let ok = !int.is_empty()
        && int.chars().all(|c| c.is_ascii_digit())
        && frac.is_none_or(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()));
    if ok {
        t.parse::<f64>().ok().filter(|n| n.is_finite())
    } else {
        None
    }
}

fn round_to(x: f64, decimals: u32) -> i128 {
    let factor = 10f64.powi(decimals.min(12) as i32);
    (x * factor).round() as i128
}

/// `|actual - reference| <= reference * pct / 100`; bei Referenz 0 muss der Wert exakt 0 sein.
pub fn within_tolerance(reference: f64, actual: f64, pct: f64) -> bool {
    if reference == 0.0 {
        return actual == 0.0;
    }
    (actual - reference).abs() <= reference.abs() * pct / 100.0 + f64::EPSILON * reference.abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> CellRules {
        CellRules {
            trim: true,
            base_url: true,
            ..Default::default()
        }
    }

    #[test]
    fn base_urls_become_placeholders() {
        let b = BaseUrls::from_base("http://127.0.0.1:8902/");
        assert_eq!(
            b.normalize("http://127.0.0.1:8902/seite?a=1"),
            "{BASE}/seite?a=1"
        );
        assert_eq!(b.normalize("http://localhost:8902/x"), "{EXT}/x");
        assert_eq!(b.normalize("http://127.0.0.1:8902"), "{BASE}");
        assert_eq!(
            b.normalize("a http://127.0.0.1:8902/a b http://localhost:8902/b"),
            "a {BASE}/a b {EXT}/b"
        );
    }

    #[test]
    fn base_url_match_respects_port_boundary_and_case() {
        let b = BaseUrls::from_base("http://127.0.0.1:8902");
        assert_eq!(
            b.normalize("http://127.0.0.1:89021/x"),
            "http://127.0.0.1:89021/x"
        );
        assert_eq!(b.normalize("HTTP://127.0.0.1:8902/X"), "{BASE}/X");
        assert_eq!(
            b.normalize("http://example.org/?u=http://127.0.0.1:8902/"),
            "http://example.org/?u={BASE}/"
        );
        let l = BaseUrls::from_base("http://localhost:8905/");
        assert_eq!(l.normalize("http://127.0.0.1:8905/x"), "{EXT}/x");
        assert_eq!(l.normalize("http://localhost:8905/x"), "{BASE}/x");
    }

    #[test]
    fn foreign_hosts_stay_untouched() {
        let b = BaseUrls::from_base("https://example.org");
        assert_eq!(b.normalize("https://example.org/a"), "{BASE}/a");
        assert_eq!(b.normalize("https://other.org/a"), "https://other.org/a");
        assert_eq!(BaseUrls::from_base("kaputt"), BaseUrls::default());
    }

    #[test]
    fn prepare_trims_and_normalizes() {
        let b = BaseUrls::from_base("http://127.0.0.1:1234");
        assert_eq!(
            rules().prepare("  http://127.0.0.1:1234/a ", &b),
            "{BASE}/a"
        );
        let no_trim = CellRules {
            trim: false,
            ..rules()
        };
        assert_eq!(no_trim.prepare(" x ", &b), " x ");
    }

    #[test]
    fn numbers_compare_after_rounding() {
        let r = CellRules {
            number_decimals: Some(2),
            ..rules()
        };
        assert!(r.equal("1.004", "1.0"));
        assert!(r.equal("100", "100.00"));
        assert!(!r.equal("1.01", "1.0"));
        assert!(!r.equal("abc", "1"));
        let exact = rules();
        assert!(exact.equal("5", "5.0"));
        assert!(!exact.equal("5", "5.1"));
    }

    #[test]
    fn tolerance_is_relative_to_reference() {
        let r = CellRules {
            tolerance_pct: Some(3.0),
            ..rules()
        };
        assert!(r.equal("100", "103"));
        assert!(r.equal("100", "97"));
        assert!(!r.equal("100", "103.5"));
        assert!(r.equal("0", "0"));
        assert!(!r.equal("0", "1"));
        assert!(!r.equal("100", "n/a"), "nicht numerisch -> Textvergleich");
    }

    #[test]
    fn booleans_case_insensitive_and_loose() {
        let ci = CellRules {
            bool_mode: BoolMode::CaseInsensitive,
            ..rules()
        };
        assert!(ci.equal("TRUE", "true"));
        assert!(ci.equal("False", "false"));
        assert!(!ci.equal("Yes", "true"));
        let loose = CellRules {
            bool_mode: BoolMode::Loose,
            ..rules()
        };
        assert!(loose.equal("Yes", "true"));
        assert!(loose.equal("0", "FALSE"));
        assert!(!loose.equal("true", "false"));
        assert!(!rules().equal("TRUE", "true"), "ohne Bool-Modus streng");
    }

    #[test]
    fn empty_equals_null_only_when_enabled() {
        let on = CellRules {
            empty_equals_null: true,
            ..rules()
        };
        assert!(on.equal("", "null"));
        assert!(on.equal("NULL", ""));
        assert!(!on.equal("", "0"));
        assert!(!rules().equal("", "null"));
        assert!(rules().equal("", ""));
    }

    #[test]
    fn value_map_translates_reference() {
        let mut map = BTreeMap::new();
        map.insert("Canonicalised".to_string(), "Canonical".to_string());
        let r = CellRules { map, ..rules() };
        assert_eq!(r.map_reference("Canonicalised"), "Canonical");
        assert_eq!(r.map_reference("Other"), "Other");
    }

    #[test]
    fn parse_number_is_strict() {
        assert_eq!(parse_number("12"), Some(12.0));
        assert_eq!(parse_number(" -1.50 "), Some(-1.5));
        assert_eq!(parse_number("1e5"), None);
        assert_eq!(parse_number("NaN"), None);
        assert_eq!(parse_number("1."), None);
        assert_eq!(parse_number(".5"), None);
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("1,5"), None);
    }
}

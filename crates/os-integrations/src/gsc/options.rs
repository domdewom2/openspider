//! Einstellungen der Search-Console-Abfrage (INTEG-111 … 114, 120 … 122, 128) und ihre Prüfung.
//!
//! `GscConfig` (os-model) trägt nur einen Teil der Optionen; [`GscOptions::from_config`] übernimmt diese und setzt für
//! die übrigen die Standardwerte der Spec. Der Dimensionsfilter steht in `GscConfig.filter` als Ausdruck mit einer
//! Regel je Zeile: `<dimension> <operator> <wert>`, z. B. `query excludingRegex (?i)openspider|open spider`.

use std::time::Duration;

use os_model::config::{ApiLimitsConfig, GscConfig, UrlMatchConfig};
use os_model::{DateRangePreset, SearchType};

/// Standard-Lese-Timeout aller Search-Console-Endpunkte in Sekunden (INTEG-128).
pub const READ_TIMEOUT_DEFAULT_S: u32 = 120;
/// Erlaubter Bereich des Lese-Timeouts in Sekunden (INTEG-128).
pub const READ_TIMEOUT_RANGE: std::ops::RangeInclusive<u32> = 10..=600;
/// Fester Timeout für den Verbindungsaufbau (INTEG-128).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Zeilen je Seite von `searchanalytics.query` (INTEG-114).
pub const PAGE_ROWS: u64 = 25_000;
/// Tageskontingent der URL Inspection je Property (INTEG-120).
pub const INSPECTION_DAILY_LIMIT: u64 = 2_000;
/// Höchstzahl URL-Inspection-Anfragen je Minute und Property (INTEG-120).
pub const INSPECTION_PER_MINUTE: usize = 600;

/// Zeitraum der Search-Analytics-Abfrage (INTEG-111).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateSpan {
    /// Die letzten `n` vollständig verfügbaren Tage (Vorgaben 7, 28, 30, 90).
    LastDays(u16),
    /// Die letzten `n` Monate (Vorgaben 3, 6, 12, 16).
    LastMonths(u8),
    /// Frei gewählte Daten (einschließlich).
    Custom { start: time::Date, end: time::Date },
}

impl DateSpan {
    /// Die Vorgaben in Anzeigereihenfolge (ohne „frei wählbar“).
    pub const PRESETS: [DateSpan; 8] = [
        DateSpan::LastDays(7),
        DateSpan::LastDays(28),
        DateSpan::LastDays(30),
        DateSpan::LastDays(90),
        DateSpan::LastMonths(3),
        DateSpan::LastMonths(6),
        DateSpan::LastMonths(12),
        DateSpan::LastMonths(16),
    ];
}

impl Default for DateSpan {
    fn default() -> Self {
        DateSpan::LastDays(30)
    }
}

/// Dimension eines Filters (INTEG-113).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterDimension {
    /// Suchanfrage.
    Query,
    /// Land (ISO-3166-1-alpha-3, klein geschrieben).
    Country,
    /// Gerät (`DESKTOP`, `MOBILE`, `TABLET`).
    Device,
}

impl FilterDimension {
    /// Name in der API.
    pub fn api_name(self) -> &'static str {
        match self {
            FilterDimension::Query => "query",
            FilterDimension::Country => "country",
            FilterDimension::Device => "device",
        }
    }
}

/// Operator eines Filters (INTEG-113), Namen wie in der API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOperator {
    Contains,
    NotContains,
    Equals,
    NotEquals,
    IncludingRegex,
    ExcludingRegex,
}

impl FilterOperator {
    const ALL: [(FilterOperator, &'static str); 6] = [
        (FilterOperator::Contains, "contains"),
        (FilterOperator::NotContains, "notContains"),
        (FilterOperator::Equals, "equals"),
        (FilterOperator::NotEquals, "notEquals"),
        (FilterOperator::IncludingRegex, "includingRegex"),
        (FilterOperator::ExcludingRegex, "excludingRegex"),
    ];

    /// Name in der API.
    pub fn api_name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(op, _)| *op == self)
            .map(|(_, name)| *name)
            .expect("alle Operatoren benannt")
    }

    /// Ist der Wert ein regulärer Ausdruck (RE2)?
    pub fn is_regex(self) -> bool {
        matches!(
            self,
            FilterOperator::IncludingRegex | FilterOperator::ExcludingRegex
        )
    }
}

/// Eine Filterregel; mehrere Regeln sind UND-verknüpft (INTEG-113).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimensionFilter {
    pub dimension: FilterDimension,
    pub operator: FilterOperator,
    pub expression: String,
}

/// Fehler in den Einstellungen; die Meldung ist für Dialog und Statuszeile gedacht.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OptionsError {
    /// Lese-Timeout außerhalb 10 – 600 s.
    #[error("Read-Timeout {0} s liegt außerhalb von 10 – 600 s")]
    ReadTimeout(u32),
    /// Zeile des Filterausdrucks nicht lesbar.
    #[error("Filter {line}: {message}")]
    Filter { line: usize, message: String },
    /// Ungültiger RE2-Ausdruck.
    #[error("Filter {line}: ungültiger RE2-Ausdruck ({message})")]
    Regex { line: usize, message: String },
    /// Freier Zeitraum unvollständig oder verkehrt.
    #[error("Zeitraum ungültig: {0}")]
    DateRange(String),
}

/// Alle Einstellungen einer Search-Console-Abfrage.
#[derive(Debug, Clone, PartialEq)]
pub struct GscOptions {
    /// Gewählte Property (`sc-domain:…` oder URL-Präfix).
    pub site: Option<String>,
    /// Zeitraum.
    pub date_span: DateSpan,
    /// Vorläufige Daten einbeziehen (`dataState = all`, Standard aus).
    pub fresh_data: bool,
    /// Suchtyp (Standard Web).
    pub search_type: SearchType,
    /// Dimensionsfilter, UND-verknüpft.
    pub filters: Vec<DimensionFilter>,
    /// Gesamtlimit der Zeilen (Standard unbegrenzt).
    pub max_rows: Option<u64>,
    /// Lese-Timeout je Anfrage in Sekunden (Standard 120).
    pub read_timeout_s: u32,
    /// URL Inspection abfragen.
    pub url_inspection: bool,
    /// Nicht indexierbare URLs bei der URL Inspection auslassen (INTEG-121).
    pub skip_non_indexable: bool,
    /// Je URL die spezifischste Property verwenden (INTEG-122).
    pub multiple_properties: bool,
    /// Sprache der Inspection-Meldungen (`languageCode`, Standard: UI-Sprache).
    pub language_code: Option<String>,
    /// URL-Abgleich (INTEG-014).
    pub url_match: UrlMatchConfig,
    /// Wiederholungen und Parallelität (INTEG-005).
    pub limits: ApiLimitsConfig,
    /// Basis-URL-Überschreibung (INTEG-019).
    pub base_url: Option<String>,
    /// Konto-Kennung (INTEG-003).
    pub account: Option<String>,
}

impl Default for GscOptions {
    fn default() -> Self {
        GscOptions {
            site: None,
            date_span: DateSpan::default(),
            fresh_data: false,
            search_type: SearchType::Web,
            filters: Vec::new(),
            max_rows: None,
            read_timeout_s: READ_TIMEOUT_DEFAULT_S,
            url_inspection: false,
            skip_non_indexable: false,
            multiple_properties: false,
            language_code: None,
            url_match: UrlMatchConfig::default(),
            limits: ApiLimitsConfig::default(),
            base_url: None,
            account: None,
        }
    }
}

impl GscOptions {
    /// Übernimmt die Felder aus `GscConfig`; Optionen, die die Konfiguration nicht trägt, bleiben auf dem
    /// Standardwert der Spec.
    pub fn from_config(cfg: &GscConfig) -> Result<GscOptions, OptionsError> {
        let date_span = match cfg.date_range.preset {
            DateRangePreset::Last7Days => DateSpan::LastDays(7),
            DateRangePreset::Last28Days => DateSpan::LastDays(28),
            DateRangePreset::Last30Days => DateSpan::LastDays(30),
            DateRangePreset::Last90Days => DateSpan::LastDays(90),
            DateRangePreset::Custom => {
                let parse = |s: &Option<String>, what: &str| {
                    s.as_deref()
                        .and_then(parse_date)
                        .ok_or_else(|| OptionsError::DateRange(format!("{what} fehlt")))
                };
                DateSpan::Custom {
                    start: parse(&cfg.date_range.from, "Beginn")?,
                    end: parse(&cfg.date_range.to, "Ende")?,
                }
            }
        };
        let options = GscOptions {
            site: cfg.site.clone(),
            date_span,
            search_type: cfg.search_type,
            filters: parse_filters(cfg.filter.as_deref().unwrap_or_default())?,
            url_inspection: cfg.url_inspection,
            url_match: cfg.url_match.clone(),
            limits: cfg.limits.clone(),
            base_url: cfg.base_url.clone(),
            account: cfg.account.clone(),
            ..GscOptions::default()
        };
        options.validate()?;
        Ok(options)
    }

    /// Prüft die Einstellungen wie der Dialog (Lese-Timeout, Filter, Zeitraum).
    pub fn validate(&self) -> Result<(), OptionsError> {
        validate_read_timeout(self.read_timeout_s)?;
        for (i, f) in self.filters.iter().enumerate() {
            validate_filter(f, i + 1)?;
        }
        if let DateSpan::Custom { start, end } = self.date_span {
            if start > end {
                return Err(OptionsError::DateRange("Beginn liegt nach dem Ende".into()));
            }
        }
        Ok(())
    }

    /// Lese-Timeout je Anfrage.
    pub fn read_timeout(&self) -> Duration {
        Duration::from_secs(u64::from(self.read_timeout_s))
    }

    /// Suchtyp-Name der API (`web`, `image`, `video`, `news`, `discover`, `googleNews`).
    pub fn api_search_type(&self) -> &'static str {
        match self.search_type {
            SearchType::Web => "web",
            SearchType::Image => "image",
            SearchType::Video => "video",
            SearchType::News => "news",
            SearchType::Discover => "discover",
            SearchType::GoogleNews => "googleNews",
        }
    }

    /// Liefert die API für diesen Suchtyp eine Position? (Bei Discover und Google News nicht, INTEG-112.)
    pub fn has_position(&self) -> bool {
        !matches!(
            self.search_type,
            SearchType::Discover | SearchType::GoogleNews
        )
    }

    /// Ist ein Filter auf diese Dimension für den Suchtyp zulässig? (Discover/Google News: weder `query` noch
    /// `device`, INTEG-112.)
    pub fn dimension_enabled(&self, dimension: FilterDimension) -> bool {
        self.has_position() || dimension == FilterDimension::Country
    }

    /// Die Filter, die gesendet werden (ohne die für den Suchtyp deaktivierten).
    pub fn effective_filters(&self) -> Vec<&DimensionFilter> {
        self.filters
            .iter()
            .filter(|f| self.dimension_enabled(f.dimension))
            .collect()
    }
}

/// Prüft den Lese-Timeout (INTEG-128: 10 – 600 s).
pub fn validate_read_timeout(seconds: u32) -> Result<(), OptionsError> {
    if READ_TIMEOUT_RANGE.contains(&seconds) {
        Ok(())
    } else {
        Err(OptionsError::ReadTimeout(seconds))
    }
}

/// Liest ein Datum `YYYY-MM-DD`.
pub fn parse_date(s: &str) -> Option<time::Date> {
    let mut parts = s.trim().splitn(3, '-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    time::Date::from_calendar_date(year, time::Month::try_from(month).ok()?, day).ok()
}

/// Liest den Filterausdruck: eine Regel je Zeile `<dimension> <operator> <wert>`; leere Zeilen und Zeilen mit `#`
/// am Anfang zählen nicht.
pub fn parse_filters(text: &str) -> Result<Vec<DimensionFilter>, OptionsError> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let err = |message: &str| OptionsError::Filter {
            line: i + 1,
            message: message.to_string(),
        };
        let (dimension, rest) = line
            .split_once(char::is_whitespace)
            .ok_or_else(|| err("Operator fehlt"))?;
        let rest = rest.trim_start();
        let (operator, value) = rest
            .split_once(char::is_whitespace)
            .ok_or_else(|| err("Wert fehlt"))?;
        let dimension = match dimension.to_ascii_lowercase().as_str() {
            "query" => FilterDimension::Query,
            "country" => FilterDimension::Country,
            "device" => FilterDimension::Device,
            _ => return Err(err("unbekannte Dimension (query, country, device)")),
        };
        let operator = FilterOperator::ALL
            .iter()
            .find(|(_, name)| name.eq_ignore_ascii_case(operator))
            .map(|(op, _)| *op)
            .ok_or_else(|| err("unbekannter Operator"))?;
        let filter = DimensionFilter {
            dimension,
            operator,
            expression: normalize_value(dimension, value.trim()),
        };
        validate_filter(&filter, i + 1)?;
        out.push(filter);
    }
    Ok(out)
}

fn normalize_value(dimension: FilterDimension, value: &str) -> String {
    match dimension {
        FilterDimension::Query => value.to_string(),
        FilterDimension::Country => value.to_ascii_lowercase(),
        FilterDimension::Device => value.to_ascii_uppercase(),
    }
}

fn validate_filter(f: &DimensionFilter, line: usize) -> Result<(), OptionsError> {
    let err = |message: &str| OptionsError::Filter {
        line,
        message: message.to_string(),
    };
    if f.expression.is_empty() {
        return Err(err("Wert fehlt"));
    }
    match f.dimension {
        FilterDimension::Country
            if f.expression.len() != 3 || !f.expression.bytes().all(|b| b.is_ascii_lowercase()) =>
        {
            return Err(err("Land als ISO-3166-1-alpha-3-Code angeben (z. B. deu)"));
        }
        FilterDimension::Device
            if !["DESKTOP", "MOBILE", "TABLET"].contains(&f.expression.as_str()) =>
        {
            return Err(err("Gerät: DESKTOP, MOBILE oder TABLET"));
        }
        _ => {}
    }
    if f.operator.is_regex() {
        if f.dimension != FilterDimension::Query {
            return Err(err("Regex-Operatoren nur für query"));
        }
        check_re2(&f.expression).map_err(|message| OptionsError::Regex { line, message })?;
    }
    Ok(())
}

/// Prüft, ob `pattern` in RE2-Syntax gültig ist (Search Console verwendet RE2, INTEG-113): kein Lookaround, keine
/// Rückverweise, keine atomaren Gruppen oder besitzergreifenden Quantoren, Klammern und Zeichenklassen geschlossen,
/// jeder Quantor hat etwas zu wiederholen.
pub fn check_re2(pattern: &str) -> Result<(), String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut depth = 0usize;
    // Ob vor der aktuellen Stelle ein wiederholbares Element steht.
    let mut atom = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' => {
                let Some(&next) = chars.get(i + 1) else {
                    return Err("Ausdruck endet mit \\".into());
                };
                if next.is_ascii_digit() && next != '0' {
                    return Err(format!("Rückverweis \\{next} wird nicht unterstützt"));
                }
                if next == 'k' || next == 'g' {
                    return Err(format!("Rückverweis \\{next} wird nicht unterstützt"));
                }
                i += 2;
                atom = true;
                continue;
            }
            '[' => {
                i = class_end(&chars, i).ok_or("Zeichenklasse nicht geschlossen")?;
                atom = true;
            }
            '(' => {
                if chars.get(i + 1) == Some(&'?') {
                    let rest: String = chars[i + 2..].iter().take(3).collect();
                    if rest.starts_with('=') || rest.starts_with('!') {
                        return Err("Lookahead wird nicht unterstützt".into());
                    }
                    if rest.starts_with("<=") || rest.starts_with("<!") {
                        return Err("Lookbehind wird nicht unterstützt".into());
                    }
                    if rest.starts_with('>') {
                        return Err("atomare Gruppen werden nicht unterstützt".into());
                    }
                    if rest.starts_with('(') {
                        return Err("bedingte Ausdrücke werden nicht unterstützt".into());
                    }
                    if rest.starts_with("P<") || rest.starts_with('<') {
                        // Benannte Gruppe: hinter den Namen springen.
                        let close = (i + 2..chars.len())
                            .find(|&j| chars[j] == '>')
                            .ok_or("Gruppenname nicht geschlossen")?;
                        depth += 1;
                        atom = false;
                        i = close + 1;
                        continue;
                    } else {
                        // Flags: (?imsU-imsU) oder (?flags:…)
                        let mut j = i + 2;
                        while j < chars.len() && "imsU-".contains(chars[j]) {
                            j += 1;
                        }
                        match chars.get(j) {
                            Some(')') => {
                                i = j + 1;
                                atom = false;
                                continue;
                            }
                            Some(':') => {
                                i = j;
                            }
                            _ => return Err("unbekannte Gruppensyntax (?".into()),
                        }
                    }
                }
                depth += 1;
                atom = false;
            }
            ')' => {
                if depth == 0 {
                    return Err("schließende Klammer ohne öffnende".into());
                }
                depth -= 1;
                atom = true;
            }
            '|' => atom = false,
            '*' | '+' | '?' | '{' => {
                let mut end = i;
                if c == '{' {
                    match repeat_end(&chars, i) {
                        Some(e) => end = e,
                        None => {
                            // `{` ohne gültige Wiederholung ist in RE2 ein Literal.
                            atom = true;
                            i += 1;
                            continue;
                        }
                    }
                }
                if !atom {
                    return Err(format!("Quantor {c} ohne Ausdruck davor"));
                }
                let mut j = end + 1;
                if chars.get(j) == Some(&'?') {
                    j += 1;
                } else if chars.get(j) == Some(&'+') {
                    return Err("besitzergreifende Quantoren werden nicht unterstützt".into());
                }
                i = j;
                atom = false;
                continue;
            }
            _ => atom = true,
        }
        i += 1;
    }
    if depth != 0 {
        return Err("Klammer nicht geschlossen".into());
    }
    Ok(())
}

/// Index des schließenden `]` der Zeichenklasse ab `start` (`[`).
fn class_end(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start + 1;
    if chars.get(i) == Some(&'^') {
        i += 1;
    }
    // `]` direkt am Anfang ist ein Literal.
    if chars.get(i) == Some(&']') {
        i += 1;
    }
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '[' if chars.get(i + 1) == Some(&':') => {
                let close = (i + 2..chars.len().saturating_sub(1))
                    .find(|&j| chars[j] == ':' && chars[j + 1] == ']')?;
                i = close + 2;
            }
            ']' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// Index des `}` einer Wiederholung `{n}`, `{n,}`, `{n,m}` ab `start`, sonst `None`.
fn repeat_end(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start + 1;
    let digits = |i: &mut usize| {
        let from = *i;
        while chars.get(*i).is_some_and(|c| c.is_ascii_digit()) {
            *i += 1;
        }
        *i > from
    };
    if !digits(&mut i) {
        return None;
    }
    if chars.get(i) == Some(&',') {
        i += 1;
        digits(&mut i);
    }
    (chars.get(i) == Some(&'}')).then_some(i)
}

//! Report „Crawl Changes“ – Änderungen seit dem letzten Crawl (OUT-154, Nutzerentscheidung 04.11.2026).
//!
//! Beantwortet die Deployment-Frage in einer Tabelle: verschwundene Seiten, neue Fehlerseiten, neue Weiterleitungen und
//! Weiterleitungsketten, geänderte Canonicals, Wechsel der Indexierbarkeit. Grundlage sind die Paare des Vergleichs
//! ([`Comparison::pairing`], URL-Mapping OUT-146); die Change-Detection-Auswahl (TECH-300) spielt keine Rolle.
//!
//! Ablauf: ein Lauf über die existierenden Current-URLs (mit Partner, falls vorhanden) und einer über die
//! Previous-URLs ohne Partner, je Zeile eine Abfrage in jeder Datei; Ketten werden nur für Weiterleitungen abgelaufen.
//! Im Speicher stehen nur die Treffer. Das Token wird alle 512 URLs geprüft (Pause/Abbruch, CRAWL-129).
//!
//! Begriffe (OUT-154): *Seite* = interne URL mit Inhaltsklasse HTML; *Fehler* = Status 4xx/5xx; *Weiterleitung* =
//! `redirect_type` gesetzt (Ziel `redirect_url`); *Kettenlänge* = Sprünge bis zum ersten Ziel ohne Weiterleitung (bei
//! einer Schleife die Glieder im Umlauf); *Canonical* = `canonical_url` (maßgeblich nach TECH-017), nur verglichen, wenn
//! die URL in beiden Crawls HTML ist und beide Crawls Canonicals speichern.

use os_model::{CancelToken, UrlId, Value};
use os_store::CrawlFile;
use rusqlite::{OptionalExtension, Statement};

use super::change::page::set as reason_set;
use super::{checkpoint, CompareError, Comparison, Side};

/// Obere Schranke beim Ablaufen einer Kette (wie die Redirect-Reports).
const WALK_GUARD: u32 = 1_000;

/// Abschnitte des Reports in Reportreihenfolge (Spalte `Change`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChangeKind {
    /// Indexierbare Seite fehlt jetzt oder antwortet mit Fehler/ohne Antwort.
    PageGone,
    /// Neue Fehler-URL (4xx/5xx).
    NewError,
    /// Neue Weiterleitung.
    NewRedirect,
    /// Anderes Weiterleitungsziel.
    RedirectTargetChanged,
    /// Neue Kette (Länge ≥ 2, vorher < 2).
    NewRedirectChain,
    /// Kette verlängert.
    LongerRedirectChain,
    /// Anderes Canonical.
    CanonicalChanged,
    /// Anderer Indexierbarkeitszustand oder Grund.
    IndexabilityChanged,
}

impl ChangeKind {
    /// Alle Abschnitte in Reportreihenfolge.
    pub const ALL: [ChangeKind; 8] = [
        ChangeKind::PageGone,
        ChangeKind::NewError,
        ChangeKind::NewRedirect,
        ChangeKind::RedirectTargetChanged,
        ChangeKind::NewRedirectChain,
        ChangeKind::LongerRedirectChain,
        ChangeKind::CanonicalChanged,
        ChangeKind::IndexabilityChanged,
    ];

    /// Kanonischer Wert der Spalte `Change` (Export, CLI, MCP).
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeKind::PageGone => "Page Gone",
            ChangeKind::NewError => "New Error",
            ChangeKind::NewRedirect => "New Redirect",
            ChangeKind::RedirectTargetChanged => "Redirect Target Changed",
            ChangeKind::NewRedirectChain => "New Redirect Chain",
            ChangeKind::LongerRedirectChain => "Longer Redirect Chain",
            ChangeKind::CanonicalChanged => "Canonical Changed",
            ChangeKind::IndexabilityChanged => "Indexability Changed",
        }
    }

    /// Kanonischer Wert → Abschnitt (ohne Beachtung der Groß-/Kleinschreibung und umgebender Leerräume).
    pub fn parse(name: &str) -> Option<ChangeKind> {
        ChangeKind::ALL
            .into_iter()
            .find(|k| k.as_str().eq_ignore_ascii_case(name.trim()))
    }
}

/// Spalten des Reports in Ausgabereihenfolge (OUT-154).
pub const HEADER: [&str; 15] = [
    "Change",
    "Address",
    "Previous Address",
    "Status Code (Previous)",
    "Status Code (Current)",
    "Indexability (Previous)",
    "Indexability (Current)",
    "Indexability Status (Previous)",
    "Indexability Status (Current)",
    "Canonical (Previous)",
    "Canonical (Current)",
    "Redirect Target (Previous)",
    "Redirect Target (Current)",
    "Redirect Chain Length (Previous)",
    "Redirect Chain Length (Current)",
];

/// Zustand einer URL in einem Crawl.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlState {
    /// Kennung in der Crawl-Datei dieses Crawls.
    pub id: i64,
    /// Adresse.
    pub address: String,
    /// Interne URL.
    pub internal: bool,
    /// Inhaltsklasse HTML.
    pub html: bool,
    /// Statuscode (`None`/0 = keine Antwort).
    pub status_code: Option<i64>,
    /// `Indexable` / `Non-Indexable`.
    pub indexability: Option<String>,
    /// Gründe als normalisierte Menge (TECH-304).
    pub indexability_status: String,
    /// Maßgebliches Canonical; `None` = nicht erhoben (keine HTML-Seite oder Canonicals nicht gespeichert).
    pub canonical: Option<String>,
    /// Weiterleitungsziel; `None` ohne Weiterleitung.
    pub redirect_target: Option<String>,
    /// Kettenlänge (0 ohne Weiterleitung).
    pub chain_length: u32,
}

impl UrlState {
    fn is_error(&self) -> bool {
        self.status_code.is_some_and(|c| (400..600).contains(&c))
    }
    fn no_response(&self) -> bool {
        self.status_code.is_none_or(|c| c <= 0)
    }
    fn indexable_page(&self) -> bool {
        self.internal && self.html && self.indexability.as_deref() == Some("Indexable")
    }
}

/// Eine Zeile des Reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRow {
    /// Abschnitt.
    pub kind: ChangeKind,
    /// Zustand im Current-Crawl (`None`: URL existiert dort nicht).
    pub current: Option<UrlState>,
    /// Zustand im Previous-Crawl (`None`: URL existiert dort nicht).
    pub previous: Option<UrlState>,
}

impl ChangeRow {
    /// `Address`: Current-Adresse, ohne Partner die Previous-Adresse.
    pub fn address(&self) -> &str {
        self.current
            .as_ref()
            .or(self.previous.as_ref())
            .map_or("", |s| s.address.as_str())
    }

    /// `Previous Address`.
    pub fn previous_address(&self) -> Option<&str> {
        self.previous.as_ref().map(|s| s.address.as_str())
    }

    /// Werte in der Reihenfolge von [`HEADER`].
    pub fn values(&self) -> Vec<Value> {
        let text = |v: Option<&str>| v.map_or(Value::Null, |s| Value::Text(s.to_string()));
        let nonempty = |v: Option<&str>| match v {
            Some(s) if !s.is_empty() => Value::Text(s.to_string()),
            _ => Value::Null,
        };
        let pair = |f: &dyn Fn(&UrlState) -> Value| {
            [
                self.previous.as_ref().map_or(Value::Null, f),
                self.current.as_ref().map_or(Value::Null, f),
            ]
        };
        let mut out = vec![
            Value::Text(self.kind.as_str().into()),
            Value::Text(self.address().into()),
            text(self.previous_address()),
        ];
        out.extend(pair(&|s| s.status_code.map_or(Value::Null, Value::Int)));
        out.extend(pair(&|s| text(s.indexability.as_deref())));
        out.extend(pair(&|s| nonempty(Some(s.indexability_status.as_str()))));
        out.extend(pair(&|s| nonempty(s.canonical.as_deref())));
        out.extend(pair(&|s| text(s.redirect_target.as_deref())));
        out.extend(pair(&|s| Value::Int(i64::from(s.chain_length))));
        out
    }
}

/// Liest Zustände einer Crawl-Datei.
struct StateReader<'a> {
    file: &'a CrawlFile,
    canonicals_stored: bool,
    row: Statement<'a>,
}

impl<'a> StateReader<'a> {
    fn new(file: &'a CrawlFile, canonicals_stored: bool) -> Result<Self, CompareError> {
        Ok(StateReader {
            file,
            canonicals_stored,
            row: file.conn().prepare(
                "SELECT address, is_internal, content_class = 'HTML', status_code, indexability, \
                 indexability_status, canonical_url, redirect_type IS NOT NULL, redirect_url \
                 FROM urls WHERE id = ?1",
            )?,
        })
    }

    fn read(&mut self, id: i64) -> Result<Option<UrlState>, CompareError> {
        let row = self
            .row
            .query_row([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, bool>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, bool>(7)?,
                    r.get::<_, Option<String>>(8)?,
                ))
            })
            .optional()?;
        let Some((
            address,
            internal,
            html,
            status_code,
            indexability,
            reasons,
            canonical,
            redirect,
            target,
        )) = row
        else {
            return Ok(None);
        };
        let chain_length = if redirect {
            let chain = os_store::queries::redirect_chain(self.file.conn(), UrlId(id), WALK_GUARD)?;
            chain
                .hops
                .iter()
                .filter(|h| h.redirect_type.is_some())
                .count()
                .max(1) as u32
        } else {
            0
        };
        Ok(Some(UrlState {
            id,
            address,
            internal,
            html,
            status_code,
            indexability,
            indexability_status: reason_set(reasons.as_deref().unwrap_or_default()),
            canonical: (html && self.canonicals_stored).then(|| canonical.unwrap_or_default()),
            redirect_target: redirect.then(|| target.unwrap_or_default()),
            chain_length,
        }))
    }
}

/// Abschnitte einer URL, die in Current existiert (`previous`: Partner, falls vorhanden).
fn kinds_for(c: &Comparison, current: &UrlState, previous: Option<&UrlState>) -> Vec<ChangeKind> {
    let mut out = Vec::new();
    if !current.internal {
        return out;
    }
    if let Some(p) = previous {
        if p.indexable_page() && (current.is_error() || current.no_response()) {
            out.push(ChangeKind::PageGone);
        }
    }
    // „ohne Fehler antwortete“ (OUT-154): Fehler oder keine Antwort im Previous-Crawl ist nicht neu (wie `All Errors`,
    // OUT-080).
    if current.is_error() && previous.is_none_or(|p| !p.is_error() && !p.no_response()) {
        out.push(ChangeKind::NewError);
    }
    if current.redirect_target.is_some() && previous.is_none_or(|p| p.redirect_target.is_none()) {
        out.push(ChangeKind::NewRedirect);
    }
    if let (Some(cur), Some(prev)) = (
        current.redirect_target.as_deref(),
        previous.and_then(|p| p.redirect_target.as_deref()),
    ) {
        if c.common_url(Side::Current, cur) != c.common_url(Side::Previous, prev) {
            out.push(ChangeKind::RedirectTargetChanged);
        }
    }
    if current.chain_length >= 2 {
        match previous {
            Some(p) if p.chain_length >= 2 => {
                if current.chain_length > p.chain_length {
                    out.push(ChangeKind::LongerRedirectChain);
                }
            }
            _ => out.push(ChangeKind::NewRedirectChain),
        }
    }
    if let Some(p) = previous {
        if let (Some(cur), Some(prev)) = (current.canonical.as_deref(), p.canonical.as_deref()) {
            let common = |side, u: &str| {
                if u.is_empty() {
                    String::new()
                } else {
                    c.common_url(side, u)
                }
            };
            if common(Side::Current, cur) != common(Side::Previous, prev) {
                out.push(ChangeKind::CanonicalChanged);
            }
        }
        if current.indexability != p.indexability
            || current.indexability_status != p.indexability_status
        {
            out.push(ChangeKind::IndexabilityChanged);
        }
    }
    out
}

/// Berechnet den Report „Crawl Changes“ (OUT-154) in Reportreihenfolge: Abschnitte wie [`ChangeKind::ALL`], darin
/// `Address` aufsteigend (Bytefolge).
pub fn crawl_changes(c: &Comparison, cancel: &CancelToken) -> Result<Vec<ChangeRow>, CompareError> {
    checkpoint(cancel)?;
    let canonicals = |side: Side| c.side(side).config.spider.canonicals.store;
    let mut current = StateReader::new(&c.current().file, canonicals(Side::Current))?;
    let mut previous = StateReader::new(&c.previous().file, canonicals(Side::Previous))?;
    let pairing = c.pairing();
    let mut rows = Vec::new();
    for (n, id) in pairing.current.iter().enumerate() {
        if n % 512 == 0 {
            checkpoint(cancel)?;
        }
        let Some(cur) = current.read(i64::from(id))? else {
            continue;
        };
        if !cur.internal {
            continue;
        }
        let prev = match pairing.previous_of(id) {
            Some(p) => previous.read(i64::from(p))?,
            None => None,
        };
        for kind in kinds_for(c, &cur, prev.as_ref()) {
            rows.push(ChangeRow {
                kind,
                current: Some(cur.clone()),
                previous: prev.clone(),
            });
        }
    }
    // Previous-URLs ohne Partner: indexierbare Seiten, die im Current-Crawl fehlen (Missing, OUT-142).
    let unpaired = &pairing.previous - &pairing.paired_previous;
    for (n, id) in unpaired.iter().enumerate() {
        if n % 512 == 0 {
            checkpoint(cancel)?;
        }
        let Some(prev) = previous.read(i64::from(id))? else {
            continue;
        };
        if prev.indexable_page() {
            rows.push(ChangeRow {
                kind: ChangeKind::PageGone,
                current: None,
                previous: Some(prev),
            });
        }
    }
    checkpoint(cancel)?;
    rows.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.address().as_bytes().cmp(b.address().as_bytes()))
    });
    Ok(rows)
}

/// Anzahl je Abschnitt in Reportreihenfolge (auch Abschnitte mit 0; MCP `summary.crawl_changes`).
pub fn counts(rows: &[ChangeRow]) -> Vec<(ChangeKind, u64)> {
    ChangeKind::ALL
        .into_iter()
        .map(|k| (k, rows.iter().filter(|r| r.kind == k).count() as u64))
        .collect()
}

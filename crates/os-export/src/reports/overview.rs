//! Report „Crawl Overview“ (OUT-080): Langformat `Section`, `Item`, `URLs`, `% of Total`.
//!
//! Abschnitte in dieser Reihenfolge:
//!
//! 1. `Summary` – Eckdaten aus `crawl_meta`, den URL-Zeilen und den Engine-Zählern (`crawl_counters`).
//!    Hinter den Angaben der Spec folgen vier **[OS-Erweiterung]**-Zeilen mit den Zählern der übersprungenen
//!    URLs (`Skipped By Exclude` … , CRAWL-110 „Durch Exclude übersprungen“).
//! 2. `Crawl Data – Internal` und 3. `Crawl Data – External` – URLs je Inhaltsklasse.
//! 4. `Response Codes – Internal` und `Response Codes – External` – je Statusgruppe, `Redirect Chain`/`Redirect
//!    Loop` aus den Weiterleitungsketten, `All Errors` = `No Response` + 4xx + 5xx.
//! 5. je Tab (Reihenfolge der UI) alle Filter mit Anzahl; `% of Total` bezogen auf den Filter `All` des Tabs.
//! 6. `Crawl Depth` – `0` … `10`, `10+`.
//! 7. `Issues` – Anzahl der Issues nach Typ und Priorität.
//!
//! `% of Total` bezieht sich auf die Grundgesamtheit des Abschnitts, auf zwei Nachkommastellen gerundet; ist
//! sie leer, bleibt die Zelle leer. Filter ohne Zahl (Crawl-Analyse, Konfiguration oder Rendering fehlen)
//! und nicht befüllbare Zeilen haben ein leeres `URLs`-Feld statt 0.
//!
//! Abschnitte 1 bis 4, 6 und 7 rechnen direkt auf der Crawl-Datei und hängen nicht an den Tab-Definitionen;
//! Abschnitt 5 und die Issue-Zeilen lesen `views::filter_counts_in`.

use os_analyze::issues;
use os_analyze::views::{filter_counts_in, FilterCount};
use os_analyze::TabDef;
use os_model::{
    ContentClass, CrawlConfig, CrawlMode, IssuePriority, IssueType, RenderingMode, Timestamp, Value,
};
use os_store::CrawlFile;

use super::common::{count, db, pct2};
use super::redirects;
use crate::error::ExportError;
use os_analyze::ColumnKind;

/// Kopfzeile des Reports.
pub const HEADER: [&str; 4] = ["Section", "Item", "URLs", "% of Total"];

/// Engine-Zähler `crawl_counters` (Namen aus `os-engine` `frontier.rs`, dort `COUNTER_*`). `os-export` hängt
/// nur als Dev-Abhängigkeit an `os-engine`; der Test `reports_overview.rs` prüft die Gleichheit der Namen.
pub const COUNTER_EXCLUDED: &str = "excluded_skipped";
/// Engine-Zähler: durch „nicht in Include“ übersprungen.
pub const COUNTER_NOT_INCLUDED: &str = "not_included_skipped";
/// Engine-Zähler: durch das Tiefenlimit übersprungen.
pub const COUNTER_DEPTH_LIMIT: &str = "skipped_depth_limit";
/// Engine-Zähler: durch das Weiterleitungslimit übersprungen.
pub const COUNTER_REDIRECT_LIMIT: &str = "skipped_redirect_limit";

/// Wert der Spalte `URLs`.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// Leere Zelle.
    Empty,
    /// Anzahl.
    Int(u64),
    /// Text (nur im Abschnitt `Summary`).
    Text(String),
    /// Dezimalzahl mit zwei Nachkommastellen (Durchschnittswerte).
    Float(f64),
}

/// Eine Zeile des Reports.
#[derive(Debug, Clone, PartialEq)]
pub struct OverviewRow {
    /// Abschnitt.
    pub section: String,
    /// Eintrag.
    pub item: String,
    /// Wert.
    pub urls: Cell,
    /// Anteil an der Grundgesamtheit des Abschnitts in Prozent.
    pub pct: Option<f64>,
}

impl OverviewRow {
    fn new(section: &str, item: impl Into<String>, urls: Cell, pct: Option<f64>) -> Self {
        OverviewRow {
            section: section.to_string(),
            item: item.into(),
            urls,
            pct,
        }
    }

    /// Zeile für Spalte `URLs` und `% of Total` als Tabellenwerte samt Zellarten.
    pub fn to_values(&self) -> ([ColumnKind; 4], [Value; 4]) {
        let (kind, urls) = match &self.urls {
            Cell::Empty => (ColumnKind::Int, Value::Null),
            Cell::Int(n) => (
                ColumnKind::Int,
                Value::Int(i64::try_from(*n).unwrap_or(i64::MAX)),
            ),
            Cell::Text(s) => (ColumnKind::Text, Value::Text(s.clone())),
            Cell::Float(f) => (ColumnKind::Float { decimals: 2 }, Value::Float(*f)),
        };
        (
            [
                ColumnKind::Text,
                ColumnKind::Text,
                kind,
                ColumnKind::Float { decimals: 2 },
            ],
            [
                Value::Text(self.section.clone()),
                Value::Text(self.item.clone()),
                urls,
                self.pct.map_or(Value::Null, Value::Float),
            ],
        )
    }
}

/// Der fertig berechnete Report.
#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
    /// Alle Zeilen in Ausgabereihenfolge.
    pub rows: Vec<OverviewRow>,
    /// Filter, die wegen der Crawl-Analyse leer sind (`Tab:Filter`), für die Hinweise nach OUT-112.
    pub pending_analysis: Vec<String>,
}

/// Berechnet den Report.
pub fn build(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    tabs: &[TabDef],
) -> Result<Overview, ExportError> {
    let counts = db_view(filter_counts_in(tabs, file, cfg))?;
    let mut rows = Vec::new();
    summary(file, &mut rows)?;
    crawl_data(file, true, &mut rows)?;
    crawl_data(file, false, &mut rows)?;
    response_codes(file, cfg, true, &mut rows)?;
    response_codes(file, cfg, false, &mut rows)?;
    tab_filters(&counts, &mut rows);
    crawl_depth(file, &mut rows)?;
    issue_rows(&counts, &mut rows);
    Ok(Overview {
        rows,
        pending_analysis: super::prereq::pending_analysis_filters(&counts),
    })
}

fn db_view<T>(r: Result<T, os_analyze::ViewError>) -> Result<T, ExportError> {
    r.map_err(ExportError::from)
}

// ---------------------------------------------------------------------------------------------
// 1 Summary

fn summary(file: &CrawlFile, rows: &mut Vec<OverviewRow>) -> Result<(), ExportError> {
    const S: &str = "Summary";
    let meta = db(file.meta())?;
    let list_source = db(file.get_meta("list_source"))?.filter(|v| !v.is_empty());
    let (first_item, first_value) = match meta.mode {
        CrawlMode::List => ("List File", list_source),
        CrawlMode::Spider => ("Start URL", meta.start_url.clone()),
    };
    let text = |v: Option<String>| v.map_or(Cell::Empty, Cell::Text);
    rows.push(OverviewRow::new(S, first_item, text(first_value), None));
    let mode = match meta.mode {
        CrawlMode::Spider => "Spider",
        CrawlMode::List => "List",
    };
    rows.push(OverviewRow::new(
        S,
        "Mode",
        Cell::Text(mode.to_string()),
        None,
    ));
    let stamp = |t: Option<Timestamp>| text(t.map(|t| t.to_export()));
    rows.push(OverviewRow::new(
        S,
        "Crawl Start",
        stamp(meta.started_at),
        None,
    ));
    rows.push(OverviewRow::new(S, "Crawl End", stamp(meta.ended_at), None));
    let elapsed_s = match (meta.started_at, meta.ended_at) {
        (Some(a), Some(b)) => {
            let secs = (b.0 - a.0).as_seconds_f64();
            (secs >= 0.0).then_some(secs)
        }
        _ => None,
    };
    rows.push(OverviewRow::new(
        S,
        "Elapsed Time",
        elapsed_s.map_or(Cell::Empty, |s| Cell::Text(clock_text(s))),
        None,
    ));

    let encountered = count(file, "SELECT COUNT(*) FROM urls WHERE in_tables = 1")?;
    let crawled = count(
        file,
        "SELECT COUNT(*) FROM urls WHERE in_tables = 1 AND state = 'done' AND robots_blocked = 0",
    )?;
    let blocked = |internal: u8| {
        count(
            file,
            &format!(
                "SELECT COUNT(*) FROM urls WHERE in_tables = 1 AND is_internal = {internal} AND robots_blocked = 1"
            ),
        )
    };
    let queued = count(
        file,
        "SELECT COUNT(*) FROM urls WHERE in_tables = 1 AND state = 'queued'",
    )?;
    rows.push(OverviewRow::new(
        S,
        "Total URLs Encountered",
        Cell::Int(encountered),
        None,
    ));
    rows.push(OverviewRow::new(
        S,
        "Total URLs Crawled",
        Cell::Int(crawled),
        None,
    ));
    rows.push(OverviewRow::new(
        S,
        "Total Internal Blocked by robots.txt",
        Cell::Int(blocked(1)?),
        None,
    ));
    rows.push(OverviewRow::new(
        S,
        "Total External Blocked by robots.txt",
        Cell::Int(blocked(0)?),
        None,
    ));
    rows.push(OverviewRow::new(
        S,
        "URLs Remaining In Queue",
        Cell::Int(queued),
        None,
    ));
    let avg_ms: Option<f64> = db(file.conn().query_row(
        "SELECT AVG(response_time_ms) FROM urls \
         WHERE in_tables = 1 AND state = 'done' AND response_time_ms IS NOT NULL",
        [],
        |r| r.get(0),
    ))?;
    rows.push(OverviewRow::new(
        S,
        "Average Response Time (ms)",
        avg_ms.map_or(Cell::Empty, |v| Cell::Float(round2(v))),
        None,
    ));
    let rate = elapsed_s
        .filter(|s| *s > 0.0)
        .map(|s| round2(crawled as f64 / s));
    rows.push(OverviewRow::new(
        S,
        "Average URLs/s",
        rate.map_or(Cell::Empty, Cell::Float),
        None,
    ));
    rows.push(OverviewRow::new(
        S,
        "OpenSpider Version",
        Cell::Text(env!("CARGO_PKG_VERSION").to_string()),
        None,
    ));
    // [OS-Erweiterung]: Zähler der übersprungenen URLs (CRAWL-110 u. a.).
    for (item, key) in [
        ("Skipped By Exclude", COUNTER_EXCLUDED),
        ("Skipped Not In Include", COUNTER_NOT_INCLUDED),
        ("Skipped By Depth Limit", COUNTER_DEPTH_LIMIT),
        ("Skipped By Redirect Limit", COUNTER_REDIRECT_LIMIT),
        ("Skipped By Folder Depth", "skipped_folder_depth"),
        ("Skipped By Query Parameters", "skipped_query_params"),
        ("Skipped By URL Length", "skipped_url_length"),
    ] {
        rows.push(OverviewRow::new(
            S,
            item,
            Cell::Int(counter(file, key)?),
            None,
        ));
    }
    if file.config()?.auth.form.iter().any(|f| f.profile.is_some()) {
        rows.push(OverviewRow::new(
            S,
            "Durch Sitzungsschutz übersprungen",
            Cell::Int(counter(file, "session_guard_skipped")?),
            None,
        ));
    }
    Ok(())
}

fn counter(file: &CrawlFile, key: &str) -> Result<u64, ExportError> {
    count(
        file,
        &format!("SELECT COALESCE((SELECT value FROM crawl_counters WHERE key = '{key}'), 0)"),
    )
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// `HH:MM:SS`; die Stunden dürfen über 24 gehen.
fn clock_text(seconds: f64) -> String {
    let total = seconds.floor() as u64;
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        (total / 60) % 60,
        total % 60
    )
}

// ---------------------------------------------------------------------------------------------
// 2/3 Crawl Data

const CLASSES: [ContentClass; 11] = [
    ContentClass::Html,
    ContentClass::JavaScript,
    ContentClass::Css,
    ContentClass::Images,
    ContentClass::Pdf,
    ContentClass::Plugins,
    ContentClass::Media,
    ContentClass::Fonts,
    ContentClass::Xml,
    ContentClass::Other,
    ContentClass::Unknown,
];

fn crawl_data(
    file: &CrawlFile,
    internal: bool,
    rows: &mut Vec<OverviewRow>,
) -> Result<(), ExportError> {
    let section = if internal {
        "Crawl Data \u{2013} Internal"
    } else {
        "Crawl Data \u{2013} External"
    };
    let scope = format!("in_tables = 1 AND is_internal = {}", u8::from(internal));
    let all = count(file, &format!("SELECT COUNT(*) FROM urls WHERE {scope}"))?;
    rows.push(OverviewRow::new(
        section,
        "All",
        Cell::Int(all),
        pct2(all, all),
    ));
    for class in CLASSES {
        let n = count(
            file,
            &format!(
                "SELECT COUNT(*) FROM urls WHERE {scope} AND content_class = '{}'",
                class.as_str()
            ),
        )?;
        rows.push(OverviewRow::new(
            section,
            class.as_str(),
            Cell::Int(n),
            pct2(n, all),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// 4 Response Codes

fn response_codes(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    internal: bool,
    rows: &mut Vec<OverviewRow>,
) -> Result<(), ExportError> {
    let section = if internal {
        "Response Codes \u{2013} Internal"
    } else {
        "Response Codes \u{2013} External"
    };
    let scope = format!("in_tables = 1 AND is_internal = {}", u8::from(internal));
    // Grundgesamtheit: URLs mit Antwort- oder Blockierungsstatus (ONPAGE-051).
    let base_sql = format!("{scope} AND state = 'done'");
    let base = count(file, &format!("SELECT COUNT(*) FROM urls WHERE {base_sql}"))?;
    let n = |cond: &str| {
        count(
            file,
            &format!("SELECT COUNT(*) FROM urls WHERE {base_sql} AND {cond}"),
        )
    };
    let mut push = |item: &str, value: Option<u64>| {
        rows.push(OverviewRow::new(
            section,
            item,
            value.map_or(Cell::Empty, Cell::Int),
            value.and_then(|v| pct2(v, base)),
        ));
    };

    push("Blocked by Robots.txt", Some(n("robots_blocked = 1")?));
    // Braucht JavaScript-Rendering (ONPAGE-053), in M1 nicht befüllbar: leeres Feld.
    push("Blocked Resource", None);
    let no_response = n("status_code = 0 AND robots_blocked = 0")?;
    push("No Response", Some(no_response));
    push("Success (2xx)", Some(n("status_code BETWEEN 200 AND 299")?));
    let redirect_3xx = n("status_code BETWEEN 300 AND 399")?;
    push("Redirection (3xx)", Some(redirect_3xx));
    // JavaScript-Weiterleitungen gibt es nur mit Rendering (ONPAGE-057).
    let js = (cfg.rendering.mode == RenderingMode::JavaScript)
        .then(|| n("redirect_type = 'javascript'"))
        .transpose()?;
    push("Redirection (JavaScript)", js);
    push(
        "Redirection (Meta Refresh)",
        Some(n("redirect_type = 'meta_refresh'")?),
    );
    push(
        "Redirection (HTTP Refresh)",
        Some({
            let tabs = os_analyze::tabs::configured(cfg);
            let tab = tabs
                .iter()
                .find(|t| t.id == "Response Codes")
                .expect("Response Codes registry");
            let filter = tab
                .filters
                .iter()
                .find(|f| f.id == "Redirection (HTTP Refresh)")
                .expect("HTTP Refresh filter");
            count(
                file,
                &format!(
                    "SELECT COUNT(*) FROM urls u WHERE {base_sql} AND ({})",
                    filter.predicate
                ),
            )?
        }),
    );
    let (chains, loops) = redirects::chain_and_loop_counts(file, internal)?;
    push("Redirect Chain", Some(chains));
    push("Redirect Loop", Some(loops));
    let client = n("status_code BETWEEN 400 AND 499")?;
    push("Client Error (4xx)", Some(client));
    let server = n("status_code BETWEEN 500 AND 599")?;
    push("Server Error (5xx)", Some(server));
    push("All Errors", Some(no_response + client + server));
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// 5 Tabs

fn tab_filters(counts: &[FilterCount], rows: &mut Vec<OverviewRow>) {
    for c in counts {
        // Grundgesamtheit: der Filter „All“ desselben Tabs (nur, wenn er eine Zahl hat).
        let base = counts
            .iter()
            .find(|a| a.tab == c.tab && a.filter == "All")
            .and_then(|a| a.count);
        let pct = match (c.count, base) {
            (Some(n), Some(b)) => pct2(n, b),
            _ => None,
        };
        rows.push(OverviewRow::new(
            &c.tab,
            c.label.clone(),
            c.count.map_or(Cell::Empty, Cell::Int),
            pct,
        ));
    }
}

// ---------------------------------------------------------------------------------------------
// 6 Crawl Depth

fn crawl_depth(file: &CrawlFile, rows: &mut Vec<OverviewRow>) -> Result<(), ExportError> {
    const S: &str = "Crawl Depth";
    // OUT-080, K-58: nur interne HTML-Seiten mit Status 2xx; Nenner der Prozente ist dieselbe Menge.
    let scope = "in_tables = 1 AND is_internal = 1 AND state = 'done' AND crawl_depth IS NOT NULL \
                 AND content_class = 'HTML' AND status_code BETWEEN 200 AND 299";
    let total = count(file, &format!("SELECT COUNT(*) FROM urls WHERE {scope}"))?;
    for depth in 0..=10u32 {
        let n = count(
            file,
            &format!("SELECT COUNT(*) FROM urls WHERE {scope} AND crawl_depth = {depth}"),
        )?;
        rows.push(OverviewRow::new(
            S,
            depth.to_string(),
            Cell::Int(n),
            pct2(n, total),
        ));
    }
    let deeper = count(
        file,
        &format!("SELECT COUNT(*) FROM urls WHERE {scope} AND crawl_depth > 10"),
    )?;
    rows.push(OverviewRow::new(
        S,
        "10+",
        Cell::Int(deeper),
        pct2(deeper, total),
    ));
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// 7 Issues

fn issue_rows(counts: &[FilterCount], rows: &mut Vec<OverviewRow>) {
    const S: &str = "Issues";
    let issue_rows = issues::issues_from_counts(counts);
    for (ty, plural) in [
        (IssueType::Issue, "Issues"),
        (IssueType::Warning, "Warnings"),
        (IssueType::Opportunity, "Opportunities"),
    ] {
        for prio in [
            IssuePriority::High,
            IssuePriority::Medium,
            IssuePriority::Low,
        ] {
            let n = issue_rows
                .iter()
                .filter(|r| r.issue_type == ty && r.priority == prio)
                .count() as u64;
            rows.push(OverviewRow::new(
                S,
                format!("{plural} ({})", prio.as_str()),
                Cell::Int(n),
                None,
            ));
        }
    }
}

//! Search Analytics (`searchanalytics.query`): Anfrage, Paginierung, Aggregation und Zuordnung zu gecrawlten URLs
//! (INTEG-111 … 114, 117, 128; Abgleich nach INTEG-014).

use std::collections::HashMap;

use os_model::config::UrlMatchConfig;
use os_model::{ApiValueWrite, Timestamp};
use serde_json::{json, Value};

use super::client::{encode_site, Endpoint, Failure, GscHttp};
use super::dates::{self, DateRange};
use super::options::{FilterDimension, GscOptions, PAGE_ROWS};
use super::{metric, SOURCE};
use crate::UrlRef;

/// Eine Zeile der API (Dimension `page`).
#[derive(Debug, Clone, PartialEq)]
pub struct SaRow {
    /// Seiten-URL laut API (kann ein Fragment tragen).
    pub page: String,
    pub clicks: f64,
    pub impressions: f64,
    /// Anteil 0 – 1.
    pub ctr: f64,
    /// Durchschnittliche Position; `None` bei Discover und Google News (INTEG-112).
    pub position: Option<f64>,
}

/// Hinweise zur Abfrage für Tab, Dialog und Statuszeile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// Der Beginn wurde auf das früheste mögliche Datum gesetzt (INTEG-111).
    StartClamped { earliest: time::Date },
    /// Es sind Dimensionsfilter aktiv (INTEG-113): `Gefiltert: …`.
    Filtered(String),
    /// Mit Suchanfrage-Filter fehlen anonymisierte Suchanfragen (INTEG-113).
    AnonymizedQueries,
    /// Ein Filter auf diese Dimension ist beim gewählten Suchtyp deaktiviert (INTEG-112).
    FilterDisabled(&'static str),
    /// Der Suchtyp liefert keine Position (INTEG-112).
    NoPosition,
}

impl Notice {
    /// Hinweistext.
    pub fn text(&self) -> String {
        match self {
            Notice::StartClamped { earliest } => format!(
                "Search Console stellt höchstens 16 Monate bereit; Beginn auf {earliest} gesetzt"
            ),
            Notice::Filtered(what) => format!("Gefiltert: {what}"),
            Notice::AnonymizedQueries => {
                "Mit Suchanfrage-Filter enthalten die Daten keine anonymisierten \
                 Suchanfragen; Summen können daher niedriger sein als ohne Filter"
                    .into()
            }
            Notice::FilterDisabled(dimension) => {
                format!("Filter auf {dimension} ist für diesen Suchtyp deaktiviert")
            }
            Notice::NoPosition => "Dieser Suchtyp liefert keine Position".into(),
        }
    }
}

/// Ergebnis einer Search-Analytics-Abfrage.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Geladene Zeilen (bei einem Fehler die bis dahin geladenen Seiten).
    pub rows: Vec<SaRow>,
    /// Abgefragter Zeitraum.
    pub range: DateRange,
    /// Gesendete Seitenanfragen (ohne Wiederholungen).
    pub pages: u32,
    /// Endgültiger Fehler einer Seite; die übrigen Seiten wurden nicht mehr angefragt.
    pub error: Option<Failure>,
    /// Hinweise.
    pub notices: Vec<Notice>,
}

impl Report {
    /// Statusmeldung der Integration, z. B. `Fehler: Read timeout nach 30 s` (INTEG-128).
    pub fn status_message(&self) -> Option<String> {
        self.error.as_ref().map(|e| match e {
            Failure::Timeout { after_s } => format!("Fehler: Read timeout nach {after_s} s"),
            other => format!("Fehler: {}", other.message()),
        })
    }

    /// Hinweis für den Tab: Zeitraum, Suchtyp und weitere Hinweise.
    pub fn hint(&self, options: &GscOptions) -> String {
        let mut parts = vec![
            format!("Zeitraum {}", self.range.label()),
            format!("Suchtyp {}", options.api_search_type()),
        ];
        parts.extend(self.notices.iter().map(Notice::text));
        parts.join(" · ")
    }
}

/// Die Hinweise, die schon vor dem Senden feststehen.
pub fn notices(options: &GscOptions, range: &DateRange) -> Vec<Notice> {
    let mut out = Vec::new();
    if range.clamped {
        out.push(Notice::StartClamped {
            earliest: range.start,
        });
    }
    if !options.has_position() {
        out.push(Notice::NoPosition);
    }
    for f in &options.filters {
        if !options.dimension_enabled(f.dimension) {
            out.push(Notice::FilterDisabled(f.dimension.api_name()));
        }
    }
    let active = options.effective_filters();
    if !active.is_empty() {
        out.push(Notice::Filtered(
            active
                .iter()
                .map(|f| {
                    format!(
                        "{} {} {}",
                        f.dimension.api_name(),
                        f.operator.api_name(),
                        f.expression
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    if active.iter().any(|f| f.dimension == FilterDimension::Query) {
        out.push(Notice::AnonymizedQueries);
    }
    out
}

/// Körper einer Seitenanfrage.
pub fn request_body(
    options: &GscOptions,
    range: &DateRange,
    start_row: u64,
    row_limit: u64,
) -> Value {
    let mut body = json!({
        "startDate": range.start.to_string(),
        "endDate": range.end.to_string(),
        "dimensions": ["page"],
        "type": options.api_search_type(),
        "aggregationType": "byPage",
        "dataState": if options.fresh_data { "all" } else { "final" },
        "rowLimit": row_limit,
        "startRow": start_row,
    });
    let filters: Vec<Value> = options
        .effective_filters()
        .iter()
        .map(|f| {
            json!({
                "dimension": f.dimension.api_name(),
                "operator": f.operator.api_name(),
                "expression": f.expression,
            })
        })
        .collect();
    if !filters.is_empty() {
        body["dimensionFilterGroups"] = json!([{ "groupType": "and", "filters": filters }]);
    }
    body
}

/// Fragt alle Seiten ab (je 25 000 Zeilen, bis eine Seite weniger liefert oder das Gesamtlimit erreicht ist). Ein
/// endgültiger Fehler beendet die Paginierung; bereits geladene Seiten bleiben erhalten (INTEG-128).
pub async fn fetch_report(http: &GscHttp, options: &GscOptions, now: Timestamp) -> Report {
    let range = dates::resolve(options.date_span, now);
    let mut report = Report {
        rows: Vec::new(),
        range,
        pages: 0,
        error: None,
        notices: notices(options, &range),
    };
    let Some(site) = options.site.as_deref() else {
        report.error = Some(Failure::NoProperty);
        return report;
    };
    let path = format!(
        "/webmasters/v3/sites/{}/searchAnalytics/query",
        encode_site(site)
    );
    let mut start_row = 0u64;
    loop {
        let wanted = options.max_rows.map_or(PAGE_ROWS, |max| {
            max.saturating_sub(report.rows.len() as u64).min(PAGE_ROWS)
        });
        if wanted == 0 {
            break;
        }
        report.pages += 1;
        let page = match http
            .post(
                Endpoint::Analytics,
                &path,
                &request_body(options, &range, start_row, wanted),
                None,
            )
            .await
        {
            Ok(page) => page,
            Err(e) => {
                report.error = Some(e);
                break;
            }
        };
        let rows = parse_rows(&page, options.has_position());
        let n = rows.len() as u64;
        report.rows.extend(rows);
        if n < wanted {
            break;
        }
        start_row += n;
    }
    report
}

/// Liest `rows[]`; eine Antwort ohne `rows` ist leer, kein Fehler (INTEG-114).
pub fn parse_rows(page: &Value, with_position: bool) -> Vec<SaRow> {
    page["rows"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    Some(SaRow {
                        page: r["keys"].get(0)?.as_str()?.to_string(),
                        clicks: r["clicks"].as_f64().unwrap_or(0.0),
                        impressions: r["impressions"].as_f64().unwrap_or(0.0),
                        ctr: r["ctr"].as_f64().unwrap_or(0.0),
                        position: if with_position {
                            r["position"].as_f64()
                        } else {
                            None
                        },
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Abgleichsschlüssel einer API-Zeile: URL ohne Fragment (INTEG-117), dann INTEG-014.
fn row_key(page: &str, options: &UrlMatchConfig) -> Option<(String, bool)> {
    let mut url = url::Url::parse(page).ok()?;
    let fragment = url.fragment().is_some();
    url.set_fragment(None);
    Some((crate::matching::match_key(url.as_str(), options)?, fragment))
}

/// Ordnet jeder gecrawlten URL höchstens eine API-Zeile zu, ohne Werte zu summieren: Zeilen ohne Fragment haben
/// Vorrang (INTEG-117), unter mehreren Varianten gewinnt die mit den meisten Clicks, bei Gleichstand Impressions
/// (INTEG-014). Ergebnis: je gecrawlter URL (Index in `crawled`) der Index der Zeile.
pub fn assign(rows: &[SaRow], crawled: &[UrlRef], options: &UrlMatchConfig) -> Vec<Option<usize>> {
    let best = best_rows(rows, options);
    crawled
        .iter()
        .map(|u| {
            let key = crate::matching::match_key(&u.url, options)?;
            best.get(&key).map(|(i, _)| *i)
        })
        .collect()
}

/// Beste Zeile je Abgleichsschlüssel und ob sie ein Fragment trägt.
fn best_rows(rows: &[SaRow], options: &UrlMatchConfig) -> HashMap<String, (usize, bool)> {
    // Fragmentlos vor Fragment, dann Clicks, dann Impressions; bei vollem Gleichstand bleibt die erste Zeile.
    let better = |a: (&SaRow, bool), b: (&SaRow, bool)| {
        (!a.1)
            .cmp(&!b.1)
            .then(a.0.clicks.total_cmp(&b.0.clicks))
            .then(a.0.impressions.total_cmp(&b.0.impressions))
            == std::cmp::Ordering::Greater
    };
    let mut best: HashMap<String, (usize, bool)> = HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        let Some((key, fragment)) = row_key(&row.page, options) else {
            continue;
        };
        match best.get(&key) {
            Some(&(j, f)) => {
                if better((row, fragment), (&rows[j], f)) {
                    best.insert(key, (i, fragment));
                }
            }
            None => {
                best.insert(key, (i, fragment));
            }
        }
    }
    best
}

/// Werte der zugeordneten URLs (`clicks`, `impressions`, `ctr`, `position`) und mit `unmatched` für API-Zeilen ohne
/// gecrawlte Entsprechung je ein Eintrag ohne `url_id` (landet in `api_urls`, Grundlage für Orphan-URLs nach
/// INTEG-015).
pub fn values(
    rows: &[SaRow],
    crawled: &[UrlRef],
    options: &UrlMatchConfig,
    fetched_at: Timestamp,
    unmatched: bool,
) -> Vec<ApiValueWrite> {
    let mut out = Vec::new();
    let assigned = assign(rows, crawled, options);
    for (u, row) in crawled.iter().zip(&assigned) {
        let Some(row) = row.map(|i| &rows[i]) else {
            continue;
        };
        let mut push = |metric: &str, v: f64| {
            out.push(ApiValueWrite {
                url_id: u.url_id,
                url: u.url.clone(),
                source: SOURCE.into(),
                metric: metric.into(),
                level: "url".into(),
                v_real: Some(v),
                v_text: None,
                fetched_at,
            });
        };
        push(metric::CLICKS, row.clicks);
        push(metric::IMPRESSIONS, row.impressions);
        push(metric::CTR, row.ctr);
        if let Some(position) = row.position {
            push(metric::POSITION, position);
        }
    }
    if !unmatched {
        return out;
    }
    let known: std::collections::HashSet<String> = crawled
        .iter()
        .filter_map(|u| crate::matching::match_key(&u.url, options))
        .collect();
    let mut best: Vec<(String, (usize, bool))> = best_rows(rows, options).into_iter().collect();
    best.sort_by_key(|(_, (i, _))| *i);
    for (key, (i, _)) in best {
        if known.contains(&key) {
            continue;
        }
        let mut url = match url::Url::parse(&rows[i].page) {
            Ok(url) => url,
            Err(_) => continue,
        };
        url.set_fragment(None);
        out.push(ApiValueWrite {
            url_id: None,
            url: url.to_string(),
            source: SOURCE.into(),
            metric: metric::CLICKS.into(),
            level: "url".into(),
            v_real: Some(rows[i].clicks),
            v_text: None,
            fetched_at,
        });
    }
    out
}

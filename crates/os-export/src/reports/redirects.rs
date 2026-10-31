//! Redirect-Reports „All Redirects“ (OUT-081), „Redirect Chains“ (OUT-082) und
//! „Redirects to Errors“ (OUT-084), auch im Listenmodus (OUT-085).
//!
//! Eine Zeile je Start-URL einer Weiterleitung mit allen Sprüngen bis zum Endziel. Die Kette ermittelt
//! `os_store::queries::redirect_chain`; die Flags `redirect_loop` und `redirect_limit_reached` stehen live in
//! `url_flags` (W25) und gelten auch nach Pause oder Stopp.
//!
//! * **Start-URLs:** interne und – soweit gespeichert – externe URLs mit Weiterleitungsziel. Ein
//!   Zwischenglied erzeugt nur dann eine eigene Zeile, wenn es zusätzlich direkt erreicht wurde (nicht
//!   ausschließlich als Weiterleitungsziel entdeckt oder von einer Seite verlinkt, OUT-081).
//! * **Sprung n:** `Redirect Type n` ist die Art der Weiterleitung, die das (n−1)-te Glied auslöst,
//!   `Redirect URL n` das Ziel, `Status Code n`/`Status n` die Antwort des auslösenden Glieds (bei HSTS-Policy
//!   307 / `HSTS Policy`, beim `Refresh`-Header 200).
//! * **Chain Type:** die gemeinsame Art aller Sprünge (`report_label`, K-03), bei verschiedenen `Mixed`.
//! * **Temp Redirect in Chain:** 302, 303, 307 sowie Meta und HTTP Refresh mit Verzögerung > 0; die
//!   HSTS-Policy zählt nicht.
//! * **Schleife:** `Number of Redirects` = Zahl der Glieder im Umlauf, `Final Address` leer, `Final Status` =
//!   `Redirect Loop`. Das Sprunglimit endet die Kette beim letzten abgerufenen Glied (`Final Status` = `Max
//!   Redirects Reached`); ein Ziel ohne Antwort ergibt `Not Followed`.
//! * **Chains:** mindestens zwei Sprünge oder eine Schleife.
//! * Spaltenzahl: die längste ausgegebene Kette bestimmt die Zahl der Sprungspalten.

use std::collections::BTreeSet;
use std::str::FromStr;

use os_model::{LinkType, RedirectType, UrlId};
use os_store::{queries, CrawlFile};

use super::common::db;
use crate::catalog::{TableEntry, TableJob};
use crate::error::ExportError;
use crate::types::{ExportFileInfo, Format};
use os_analyze::ColumnKind;
use os_model::{CrawlConfig, Value};

/// Obere Schranke für das Ablaufen einer Kette, unabhängig vom Weiterleitungslimit der Konfiguration.
const WALK_GUARD: u32 = 1_000;

/// Welcher Redirect-Report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectReport {
    /// Alle Weiterleitungen.
    All,
    /// Nur Ketten ab zwei Sprüngen oder Schleifen.
    Chains,
    /// Ketten mit fehlerhaftem Endziel.
    Errors,
}

/// Ein Glied der Kette mit allen Angaben, die der Report braucht.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Node {
    pub address: String,
    pub status_code: Option<u16>,
    pub status: Option<String>,
    pub redirect_type: Option<RedirectType>,
    pub delay: Option<u32>,
    pub content_type: Option<String>,
    pub indexability: Option<String>,
    pub indexability_status: Option<String>,
    pub redirect_url: Option<String>,
    pub target_id: Option<i64>,
    pub id: Option<i64>,
    pub limit_flag: bool,
}

/// Ausgang der Kette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainOutcome {
    /// Die Kette endet an einem Glied ohne Weiterleitung.
    End,
    /// Die Kette läuft im Kreis.
    Loop,
    /// Das Weiterleitungslimit hat die Kette beendet.
    LimitReached,
    /// Das Ziel wurde nicht abgerufen.
    NotFollowed,
}

/// Eine Kette von der Start-URL bis zum Ende.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Chain {
    pub nodes: Vec<Node>,
    pub outcome: ChainOutcome,
    /// Bei Schleifen: Index des Glieds, auf das die letzte Weiterleitung zurückführt.
    pub closure: Option<usize>,
}

impl Chain {
    /// Anzahl der Sprünge.
    pub fn edges(&self) -> usize {
        match self.outcome {
            ChainOutcome::Loop => self.nodes.len(),
            _ => self.nodes.len().saturating_sub(1),
        }
    }

    /// Ziel des Sprungs `i` (0-basiert).
    fn edge_target(&self, i: usize) -> &Node {
        if i + 1 < self.nodes.len() {
            &self.nodes[i + 1]
        } else {
            &self.nodes[self.closure.unwrap_or(0)]
        }
    }

    fn is_temporary(&self) -> bool {
        (0..self.edges()).any(|i| {
            let n = &self.nodes[i];
            match n.redirect_type {
                Some(RedirectType::Http) => matches!(n.status_code, Some(302 | 303 | 307)),
                Some(RedirectType::MetaRefresh | RedirectType::HttpRefresh) => {
                    n.delay.unwrap_or(0) > 0
                }
                _ => false,
            }
        })
    }

    fn chain_type(&self) -> &'static str {
        let kinds: BTreeSet<&'static str> = (0..self.edges())
            .filter_map(|i| self.nodes[i].redirect_type.map(|t| t.report_label()))
            .collect();
        match kinds.len() {
            0 => "",
            1 => kinds.into_iter().next().unwrap_or(""),
            _ => "Mixed",
        }
    }
}

/// Linktypen der Weiterleitungs-Linkzeilen (zählen nicht als direkte Verlinkung).
fn redirect_link_types() -> String {
    [
        LinkType::HttpRedirect,
        LinkType::HstsPolicy,
        LinkType::JavaScriptRedirect,
        LinkType::MetaRefresh,
        LinkType::HttpRefresh,
    ]
    .iter()
    .map(|t| format!("'{}'", t.as_str()))
    .collect::<Vec<_>>()
    .join(", ")
}

/// Lädt ein Kettenglied.
fn load_node(file: &CrawlFile, id: i64) -> Result<Node, ExportError> {
    let sql = "SELECT address, status_code, status, redirect_type, meta_refresh_delay, content_type, \
               indexability, indexability_status, redirect_url, redirect_target_id, \
               EXISTS(SELECT 1 FROM url_flags f WHERE f.url_id = urls.id AND f.flag = 'redirect_limit_reached') \
               FROM urls WHERE id = ?1";
    let row = db(file.conn().query_row(sql, [id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<i64>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?,
            r.get::<_, Option<String>>(8)?,
            r.get::<_, Option<i64>>(9)?,
            r.get::<_, bool>(10)?,
        ))
    }))?;
    Ok(Node {
        address: row.0,
        status_code: row.1.and_then(|c| u16::try_from(c).ok()),
        status: row.2,
        redirect_type: row
            .3
            .as_deref()
            .and_then(|s| RedirectType::from_str(s).ok()),
        delay: row.4.and_then(|d| u32::try_from(d).ok()),
        content_type: row.5,
        indexability: row.6,
        indexability_status: row.7,
        redirect_url: row.8,
        target_id: row.9,
        id: Some(id),
        limit_flag: row.10,
    })
}

/// Kette ab `start`.
pub(crate) fn chain_for(file: &CrawlFile, start: i64) -> Result<Chain, ExportError> {
    let walked = db(queries::redirect_chain(
        file.conn(),
        UrlId(start),
        WALK_GUARD,
    ))?;
    let mut nodes = Vec::with_capacity(walked.hops.len() + 1);
    for hop in &walked.hops {
        nodes.push(load_node(file, hop.id.0)?);
    }
    if nodes.is_empty() {
        nodes.push(load_node(file, start)?);
    }
    // Weiterleitungslimit: die Kette endet beim letzten abgerufenen Glied.
    if let Some(k) = nodes.iter().position(|n| n.limit_flag) {
        nodes.truncate(k + 1);
        return Ok(Chain {
            nodes,
            outcome: ChainOutcome::LimitReached,
            closure: None,
        });
    }
    if walked.is_loop {
        let closure = nodes
            .last()
            .and_then(|l| l.target_id)
            .and_then(|t| nodes.iter().position(|n| n.id == Some(t)));
        return Ok(Chain {
            nodes,
            outcome: ChainOutcome::Loop,
            closure,
        });
    }
    // Letztes Glied leitet weiter, das Ziel ist aber nicht als URL gespeichert: Ziel aus `redirect_url`.
    let pending = nodes.last().and_then(|l| {
        (l.target_id.is_none() && l.redirect_type.is_some())
            .then(|| l.redirect_url.clone())
            .flatten()
    });
    if let Some(url) = pending {
        nodes.push(Node {
            address: url,
            status_code: None,
            status: None,
            redirect_type: None,
            delay: None,
            content_type: None,
            indexability: None,
            indexability_status: None,
            redirect_url: None,
            target_id: None,
            id: None,
            limit_flag: false,
        });
    }
    let last = nodes.last();
    let not_followed = last.is_some_and(|l| l.status_code.is_none());
    Ok(Chain {
        nodes,
        outcome: if not_followed {
            ChainOutcome::NotFollowed
        } else {
            ChainOutcome::End
        },
        closure: None,
    })
}

/// Zahl der URLs mit Kette (mindestens zwei Sprünge, ohne Schleife) und mit Schleife im Bereich
/// (intern/extern) für die Crawl-Overview und die Filter „Redirect Chain“/„Redirect Loop“.
pub(crate) fn chain_and_loop_counts(
    file: &CrawlFile,
    internal: bool,
) -> Result<(u64, u64), ExportError> {
    let starts = collect_ids(
        file,
        &format!(
            "SELECT id FROM urls WHERE in_tables = 1 AND is_internal = {} \
             AND redirect_target_id IS NOT NULL ORDER BY id",
            u8::from(internal)
        ),
    )?;
    let mut chains = 0;
    for id in starts {
        let walked = db(queries::redirect_chain(file.conn(), UrlId(id), WALK_GUARD))?;
        if !walked.is_loop && walked.hops.len() >= 3 {
            chains += 1;
        }
    }
    let loops = super::common::count(
        file,
        &format!(
            "SELECT COUNT(*) FROM url_flags f JOIN urls u ON u.id = f.url_id \
             WHERE f.flag = 'redirect_loop' AND u.in_tables = 1 AND u.is_internal = {}",
            u8::from(internal)
        ),
    )?;
    Ok((chains, loops))
}

fn collect_ids(file: &CrawlFile, sql: &str) -> Result<Vec<i64>, ExportError> {
    let mut stmt = db(file.conn().prepare(sql))?;
    let rows = db(stmt.query_map([], |r| r.get::<_, i64>(0)))?;
    db(rows.collect::<Result<Vec<_>, _>>())
}

/// Start-URLs der Zeilen in Crawl-Reihenfolge (ohne Zwischenglieder, die nur über die Kette erreicht wurden).
///
/// „Von einer Seite verlinkt“ heißt: mindestens eine Linkzeile mit einem anderen Typ als den Weiterleitungstypen
/// zeigt auf die URL. Mit dem Index `links_target` ist das eine Suche je Kandidat; ohne ihn (K-74: pausierter oder
/// abgebrochener Crawl) eine Teilabfrage, die `links` einmal für alle Kandidaten liest – nie ein Lauf je Zeile.
fn start_ids(file: &CrawlFile) -> Result<Vec<i64>, ExportError> {
    let list_count: i64 = db(file
        .conn()
        .query_row("SELECT COUNT(*) FROM list_input", [], |r| r.get(0)))?;
    if list_count > 0 {
        return collect_ids(
            file,
            "SELECT u.id FROM list_input l JOIN urls u ON u.id = l.url_id WHERE l.outcome = 'ok' ORDER BY l.position",
        );
    }
    let types = redirect_link_types();
    let linked = if db(os_store::link_index::present(file.conn()))? {
        format!(
            "EXISTS (SELECT 1 FROM links l WHERE l.target_id = u.id AND l.link_type NOT IN ({types}))"
        )
    } else {
        format!(
            "u.id IN (SELECT target_id FROM links WHERE target_id IS NOT NULL AND link_type NOT IN ({types}))"
        )
    };
    collect_ids(
        file,
        &format!(
            "SELECT u.id FROM urls u \
             WHERE u.in_tables = 1 AND (u.redirect_target_id IS NOT NULL OR u.redirect_url IS NOT NULL) \
             AND NOT ( \
               u.discovered_via = 'redirect' \
               AND EXISTS (SELECT 1 FROM urls p WHERE p.redirect_target_id = u.id AND p.id <> u.id) \
               AND NOT {linked} \
             ) \
             ORDER BY u.id"
        ),
    )
}

/// Angaben zur Start-URL, die nicht zur Kette gehören.
struct StartInfo {
    inlinks: i64,
    sample_source: Option<String>,
}

/// `Sample Source` je Start-URL für alle `ids` in **einem** Lauf über `links` (kleinste Quell-Kennung unter den
/// Nicht-Weiterleitungs-Links). `None`, wenn der Index `links_target` vorhanden ist: dann genügt die Suche je Zeile.
fn sample_sources(
    file: &CrawlFile,
    ids: &[i64],
) -> Result<Option<std::collections::HashMap<i64, String>>, ExportError> {
    if ids.is_empty() || db(os_store::link_index::present(file.conn()))? {
        return Ok(None);
    }
    let types = redirect_link_types();
    let json = format!(
        "[{}]",
        ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
    );
    let sql = format!(
        "SELECT m.target_id, s.address FROM \
           (SELECT target_id, MIN(source_id) AS source_id FROM links \
             WHERE target_id IN (SELECT value FROM json_each(?1)) AND link_type NOT IN ({types}) \
             GROUP BY target_id) m \
         JOIN urls s ON s.id = m.source_id"
    );
    let mut stmt = db(file.conn().prepare(&sql))?;
    let rows = db(stmt.query_map([json], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    }))?;
    let mut map = std::collections::HashMap::with_capacity(ids.len());
    for row in rows {
        let (id, address) = db(row)?;
        map.insert(id, address);
    }
    Ok(Some(map))
}

fn start_info(
    file: &CrawlFile,
    id: i64,
    samples: Option<&std::collections::HashMap<i64, String>>,
) -> Result<StartInfo, ExportError> {
    let inlinks: i64 = db(file.conn().query_row(
        "SELECT inlinks FROM url_counters WHERE url_id = ?1",
        [id],
        |r| r.get(0),
    ))?;
    if let Some(samples) = samples {
        return Ok(StartInfo {
            inlinks,
            sample_source: samples.get(&id).cloned(),
        });
    }
    let types = redirect_link_types();
    let sql = format!(
        "SELECT s.address FROM links l JOIN urls s ON s.id = l.source_id \
         WHERE l.target_id = ?1 AND l.link_type NOT IN ({types}) \
         ORDER BY l.source_id, l.id LIMIT 1"
    );
    let mut stmt = db(file.conn().prepare_cached(&sql))?;
    let mut rows = db(stmt.query([id]))?;
    let sample_source = match db(rows.next())? {
        Some(row) => Some(db(row.get::<_, String>(0))?),
        None => None,
    };
    Ok(StartInfo {
        inlinks,
        sample_source,
    })
}

const FIXED_COLUMNS: [&str; 13] = [
    "Chain Type",
    "Number of Redirects",
    "Redirect Loop",
    "Temp Redirect in Chain",
    "Address",
    "Final Address",
    "Final Status Code",
    "Final Status",
    "Final Content Type",
    "Final Indexability",
    "Final Indexability Status",
    "Inlinks",
    "Sample Source",
];

fn bool_text(b: bool) -> Value {
    Value::Text(if b { "True" } else { "False" }.to_string())
}

fn text_opt(s: Option<&str>) -> Value {
    s.filter(|s| !s.is_empty())
        .map_or(Value::Null, |s| Value::Text(s.to_string()))
}

/// Zeilenwerte, Zellarten für die festen Spalten und die Sprünge.
fn row_values(
    chain: &Chain,
    start: &Node,
    info: &StartInfo,
    hops: usize,
) -> (Vec<ColumnKind>, Vec<Value>) {
    let mut kinds = vec![ColumnKind::Text; FIXED_COLUMNS.len() + 4 * hops];
    kinds[1] = ColumnKind::Int;
    kinds[6] = ColumnKind::Int;
    kinds[11] = ColumnKind::Int;
    let is_loop = chain.outcome == ChainOutcome::Loop;
    let last = chain.nodes.last();
    let (final_addr, final_code, final_status, final_ct, final_ind, final_ind_status) =
        match chain.outcome {
            ChainOutcome::Loop => (
                None,
                None,
                Some("Redirect Loop".to_string()),
                None,
                None,
                None,
            ),
            ChainOutcome::LimitReached => (
                last.map(|l| l.address.clone()),
                last.and_then(|l| l.status_code),
                Some("Max Redirects Reached".to_string()),
                last.and_then(|l| l.content_type.clone()),
                last.and_then(|l| l.indexability.clone()),
                last.and_then(|l| l.indexability_status.clone()),
            ),
            ChainOutcome::NotFollowed => (
                last.map(|l| l.address.clone()),
                None,
                Some("Not Followed".to_string()),
                None,
                None,
                None,
            ),
            ChainOutcome::End => (
                last.map(|l| l.address.clone()),
                last.and_then(|l| l.status_code),
                last.and_then(|l| l.status.clone()),
                last.and_then(|l| l.content_type.clone()),
                last.and_then(|l| l.indexability.clone()),
                last.and_then(|l| l.indexability_status.clone()),
            ),
        };
    let mut v = vec![
        text_opt(Some(chain.chain_type())),
        Value::Int(chain.edges() as i64),
        bool_text(is_loop),
        bool_text(chain.is_temporary()),
        Value::Text(start.address.clone()),
        text_opt(final_addr.as_deref()),
        final_code.map_or(Value::Null, |c| Value::Int(i64::from(c))),
        text_opt(final_status.as_deref()),
        text_opt(final_ct.as_deref()),
        text_opt(final_ind.as_deref()),
        text_opt(final_ind_status.as_deref()),
        Value::Int(info.inlinks),
        text_opt(info.sample_source.as_deref()),
    ];
    for i in 0..hops {
        if i < chain.edges() {
            let from = &chain.nodes[i];
            v.push(text_opt(from.redirect_type.map(|t| t.report_label())));
            v.push(Value::Text(chain.edge_target(i).address.clone()));
            v.push(
                from.status_code
                    .map_or(Value::Null, |c| Value::Int(i64::from(c))),
            );
            v.push(text_opt(from.status.as_deref()));
            kinds[FIXED_COLUMNS.len() + 4 * i + 2] = ColumnKind::Int;
        } else {
            v.extend([Value::Null, Value::Null, Value::Null, Value::Null]);
        }
    }
    (kinds, v)
}

/// Schreibt den Report.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_report_with_options(
    file: &CrawlFile,
    _cfg: &CrawlConfig,
    which: RedirectReport,
    path: &std::path::Path,
    sheet: &str,
    format: Format,
    opts: &crate::types::WriterOptions,
) -> Result<ExportFileInfo, ExportError> {
    let mut writer = crate::output::open_writer(path, format, sheet, opts)?;
    let cancel = os_model::CancelToken::default();
    let mut ctx = crate::types::ExportCtx {
        cancel: &cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    };
    write_report_to(file, which, writer.as_mut(), &mut ctx)?;
    writer.finish()
}

pub fn write_report_to(
    file: &CrawlFile,
    which: RedirectReport,
    writer: &mut dyn crate::types::TableWriter,
    ctx: &mut crate::types::ExportCtx<'_>,
) -> Result<u64, ExportError> {
    write_rows(file, which, writer, ctx, true)
}

fn error_target(file: &CrawlFile, chain: &Chain) -> Result<bool, ExportError> {
    if chain.edges() == 0
        || chain.outcome == ChainOutcome::Loop
        || chain.outcome == ChainOutcome::LimitReached
    {
        return Ok(false);
    }
    let Some(id) = chain.nodes.last().and_then(|n| n.id) else {
        return Ok(false);
    };
    // Die Fachfilter bleiben die einzige Quelle der Fehlerzuordnung (OUT-084).
    // Insbesondere ist ein nicht abgerufenes Ziel kein Fehler und eine reine Schleife kein Fehlerziel.
    static SQL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let sql = SQL.get_or_init(|| {
        let tab = os_analyze::tabs::response_codes::tab();
        let predicates = [
            "Blocked by Robots.txt",
            "No Response",
            "Client Error (4XX)",
            "Server Error (5XX)",
        ]
        .iter()
        .map(|id| {
            let filter = tab
                .filters
                .iter()
                .find(|f| f.id == *id)
                .expect("Response Codes error filter");
            format!("({})", filter.predicate)
        })
        .collect::<Vec<_>>()
        .join(" OR ");
        format!(
            "SELECT COALESCE(({}) AND ({predicates}), 0) FROM urls u WHERE u.id = ?1",
            tab.population
        )
    });
    db(file.conn().query_row(sql, [id], |r| r.get(0)))
}

fn write_rows(
    file: &CrawlFile,
    which: RedirectReport,
    writer: &mut dyn crate::types::TableWriter,
    ctx: &mut crate::types::ExportCtx<'_>,
    header: bool,
) -> Result<u64, ExportError> {
    // OUT-085: Nur der Chains-Report enthält Listen-URLs ohne Sprung.
    let is_list: bool =
        db(file
            .conn()
            .query_row("SELECT EXISTS(SELECT 1 FROM list_input)", [], |r| r.get(0)))?;
    // Durchgang 1: Zeilenmenge und längste Kette.
    let mut included: Vec<i64> = Vec::new();
    let mut max_hops = 0usize;
    for id in start_ids(file)? {
        let chain = chain_for(file, id)?;
        let keep = match which {
            RedirectReport::All => chain.edges() >= 1,
            RedirectReport::Errors => error_target(file, &chain)?,
            RedirectReport::Chains => {
                is_list || chain.edges() >= 2 || chain.outcome == ChainOutcome::Loop
            }
        };
        if keep {
            max_hops = max_hops.max(chain.edges());
            included.push(id);
        }
    }

    let mut columns: Vec<String> = FIXED_COLUMNS.iter().map(|c| (*c).to_string()).collect();
    for n in 1..=max_hops {
        columns.push(format!("Redirect Type {n}"));
        columns.push(format!("Redirect URL {n}"));
        columns.push(format!("Status Code {n}"));
        columns.push(format!("Status {n}"));
    }
    if header {
        writer.header(&columns)?;
    }
    // Durchgang 2: Zeilen schreiben (konstanter Speicher). Ohne Index `links_target` liest ein Lauf über `links`
    // die Beispielquellen aller Zeilen (eine Adresse je Zeile im Speicher).
    let samples = sample_sources(file, &included)?;
    let mut count = 0;
    for id in included {
        ctx.checkpoint()?;
        let chain = chain_for(file, id)?;
        let info = start_info(file, id, samples.as_ref())?;
        let start = load_node(file, id)?;
        let (kinds, values) = row_values(&chain, &start, &info, max_hops);
        writer.row(&kinds, &values)?;
        count += 1;
        (ctx.progress)(count);
    }
    Ok(count)
}

/// Redirects: Redirects to Errors (OUT-084, M2-D12): Kettenformat wie OUT-081, nur Ketten mit Fehlerziel.
static REDIRECTS_TO_ERRORS: TableEntry = TableEntry {
    name: "Redirects:Redirects to Errors",
    aliases: &[],
    file_stem: "Redirects to Errors",
    header: crate::catalog::CHAIN_HEADER,
    header_fn: Some(error_header),
    requires: &[],
    owner: "M2-D12",
    run: error_run,
};

pub fn entries() -> &'static [TableEntry] {
    std::slice::from_ref(&REDIRECTS_TO_ERRORS)
}

fn error_header(job: &TableJob<'_>) -> Result<Vec<String>, ExportError> {
    let mut max_hops = 0;
    for id in start_ids(job.file)? {
        let chain = chain_for(job.file, id)?;
        if error_target(job.file, &chain)? {
            max_hops = max_hops.max(chain.edges());
        }
    }
    let mut columns: Vec<String> = FIXED_COLUMNS.iter().map(|s| (*s).into()).collect();
    for n in 1..=max_hops {
        columns.extend([
            format!("Redirect Type {n}"),
            format!("Redirect URL {n}"),
            format!("Status Code {n}"),
            format!("Status {n}"),
        ]);
    }
    Ok(columns)
}

fn error_run(
    job: &TableJob<'_>,
    writer: &mut dyn crate::types::TableWriter,
) -> Result<u64, ExportError> {
    let cancel = os_model::CancelToken::default();
    let mut ctx = crate::types::ExportCtx {
        cancel: &cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    };
    write_rows(job.file, RedirectReport::Errors, writer, &mut ctx, false)
}

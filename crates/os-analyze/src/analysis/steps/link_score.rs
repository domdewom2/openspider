//! TECH-145: ten deterministic iterations over the resolved internal link graph.
//!
//! B3-PERF: Der Graph liegt kompakt im Arbeitsspeicher (je eindeutiger Kante 4 Byte: Zielknoten plus
//! Folgebit, je Quelle ein Abschnitt), statt in zehn Durchläufen aus einer SQLite-Temp-Tabelle gelesen zu
//! werden. Bei 5 Mio. Knoten / 100 Mio. Kanten sind das rund 0,5 GB (Grenze nach TECH-145: 2 GB). Die
//! Summation läuft wie zuvor in fester Reihenfolge (Quellknoten aufsteigend), die Rohwerte sind bitgleich.
//! Das Ergebnis wird direkt in `url_ext_data` geschrieben (Schlüsselkennungen einmal aufgelöst).
use crate::{analysis::CancelToken, error::AnalyzeError};
use os_model::{CrawlConfig, StepState};
use os_store::CrawlFile;
use rusqlite::{params, OptionalExtension};

#[derive(Clone, Copy, Debug, Default)]
pub enum Scaling {
    #[default]
    Logarithmic,
    Linear,
}

/// Kein Knoten / keine Auflösung.
const NONE: u32 = u32::MAX;
/// Folgebit im Kanteneintrag (Zielknoten in den unteren 31 Bit).
const FOLLOW: u32 = 1 << 31;

/// Zuordnung URL-Kennung → Position in den nach Kennung sortierten Routen.
enum Lookup {
    /// Dichte Tabelle `id - min` → Position (Standardfall: Kennungen sind fortlaufend vergeben).
    Dense { min: i64, slots: Vec<u32> },
    /// Binäre Suche über die sortierten Kennungen (sehr lückenhafte Kennungen).
    Sparse,
}

struct Routes {
    ids: Vec<i64>,
    next: Vec<Option<i64>>,
    /// Knotennummer (Reihenfolge der Kennungen) oder [`NONE`].
    node: Vec<u32>,
    lookup: Lookup,
}

impl Routes {
    fn new(ids: Vec<i64>, next: Vec<Option<i64>>, node: Vec<u32>) -> Self {
        let lookup = match (ids.first(), ids.last()) {
            (Some(&min), Some(&max))
                if u64::try_from(max - min).is_ok_and(|span| span < 4 * ids.len() as u64 + 1024) =>
            {
                let mut slots = vec![NONE; (max - min + 1) as usize];
                for (pos, id) in ids.iter().enumerate() {
                    slots[(id - min) as usize] = pos as u32;
                }
                Lookup::Dense { min, slots }
            }
            _ => Lookup::Sparse,
        };
        Routes {
            ids,
            next,
            node,
            lookup,
        }
    }

    fn position(&self, id: i64) -> Option<usize> {
        match &self.lookup {
            Lookup::Dense { min, slots } => {
                let slot = usize::try_from(id.checked_sub(*min)?).ok()?;
                slots
                    .get(slot)
                    .copied()
                    .filter(|p| *p != NONE)
                    .map(|p| p as usize)
            }
            Lookup::Sparse => self.ids.binary_search(&id).ok(),
        }
    }

    /// `res(u)`: höchstens 10 Schritte über Weiterleitungen/Canonicals bis zu einem Knoten.
    fn resolve(&self, mut id: i64) -> Option<u32> {
        let mut seen = [0i64; 11];
        for depth in 0..=10 {
            if seen[..depth].contains(&id) {
                return None;
            }
            seen[depth] = id;
            let pos = self.position(id)?;
            if self.node[pos] != NONE {
                return Some(self.node[pos]);
            }
            id = self.next[pos]?;
        }
        None
    }
}

/// Kantenmenge je Quellknoten: Abschnitte in `targets`, nach Quelle aufsteigend, je Quelle genau einer.
struct Graph {
    /// `(Quelle, Anfang, Länge)`.
    segments: Vec<(u32, usize, usize)>,
    /// Zielknoten | [`FOLLOW`], je Abschnitt doppelfrei.
    targets: Vec<u32>,
}

/// Sortiert und entdoppelt die Ziele eines Abschnitts; ein folgbarer Link gewinnt (MAX über `follow`).
fn dedup_targets(buf: &mut Vec<u32>) {
    buf.sort_unstable_by_key(|t| (t & !FOLLOW, t & FOLLOW));
    let mut out = 0usize;
    for i in 0..buf.len() {
        let t = buf[i];
        if out > 0 && (buf[out - 1] & !FOLLOW) == (t & !FOLLOW) {
            buf[out - 1] |= t & FOLLOW;
        } else {
            buf[out] = t;
            out += 1;
        }
    }
    buf.truncate(out);
}

impl Graph {
    fn new() -> Self {
        Graph {
            segments: Vec::new(),
            targets: Vec::new(),
        }
    }

    fn flush(&mut self, source: u32, buf: &mut Vec<u32>) {
        if buf.is_empty() {
            return;
        }
        dedup_targets(buf);
        self.segments.push((source, self.targets.len(), buf.len()));
        self.targets.extend_from_slice(buf);
        buf.clear();
    }

    /// Ordnet die Abschnitte nach Quelle und vereinigt mehrfach vorkommende Quellen (nicht
    /// zusammenhängend gespeicherte Links einer Seite).
    fn finish(mut self) -> Self {
        self.segments.sort_by_key(|s| s.0);
        if self.segments.windows(2).all(|w| w[0].0 != w[1].0) {
            return self;
        }
        let mut segments = Vec::with_capacity(self.segments.len());
        let mut targets = Vec::with_capacity(self.targets.len());
        let mut buf = Vec::new();
        let mut i = 0;
        while i < self.segments.len() {
            let source = self.segments[i].0;
            while i < self.segments.len() && self.segments[i].0 == source {
                let (_, start, len) = self.segments[i];
                buf.extend_from_slice(&self.targets[start..start + len]);
                i += 1;
            }
            dedup_targets(&mut buf);
            segments.push((source, targets.len(), buf.len()));
            targets.extend_from_slice(&buf);
            buf.clear();
        }
        Graph { segments, targets }
    }
}

pub fn run(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    run_with_scaling(file, cfg, cancel, Scaling::Logarithmic)
}

/// Internal linear option for independent comparisons; public default remains logarithmic.
pub fn run_with_scaling(
    file: &CrawlFile,
    _: &CrawlConfig,
    cancel: &CancelToken,
    scaling: Scaling,
) -> Result<StepState, AnalyzeError> {
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    super::links::with_sql_cancel(file.conn(), cancel, || calculate(file, cancel, scaling))
}

fn calculate(
    file: &CrawlFile,
    cancel: &CancelToken,
    scaling: Scaling,
) -> Result<StepState, AnalyzeError> {
    let conn = file.conn();
    // Knoten V und Auflösungsdaten aller URLs.
    let mut nodes: Vec<i64> = Vec::new();
    let mut node_follow: Vec<bool> = Vec::new();
    let routes = {
        let mut ids = Vec::new();
        let mut next_ids = Vec::new();
        let mut node_of = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT id, is_internal, state, content_class, status_code, redirect_type,
            redirect_target_id, canonical_target_id, COALESCE(effective_follow, 1) <> 0
            FROM urls ORDER BY id",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            let id: i64 = r.get(0)?;
            let internal: bool = r.get(1)?;
            let status: Option<i64> = r.get(4)?;
            let redirect: Option<String> = r.get(5)?;
            let canonical: Option<i64> = r.get(7)?;
            let next = if redirect.is_some() || status.is_some_and(|s| (300..400).contains(&s)) {
                r.get(6)?
            } else {
                canonical.filter(|c| *c != id)
            };
            let participates = internal
                && r.get::<_, String>(2)? == "done"
                && r.get::<_, String>(3)? == "HTML"
                && status.is_some_and(|s| s > 0)
                && redirect.is_none()
                && !status.is_some_and(|s| (300..400).contains(&s))
                && canonical.is_none_or(|c| c == id);
            if participates {
                node_of.push(nodes.len() as u32);
                nodes.push(id);
                node_follow.push(r.get(8)?);
            } else {
                node_of.push(NONE);
            }
            ids.push(id);
            next_ids.push(next);
        }
        Routes::new(ids, next_ids, node_of)
    };
    if nodes.len() >= FOLLOW as usize {
        return Err(AnalyzeError::Store(os_store::StoreError::InvalidInput(
            "Link Score: mehr als 2^31 Knoten".into(),
        )));
    }
    let resolved: Vec<u32> = routes
        .ids
        .iter()
        .map(|id| routes.resolve(*id).unwrap_or(NONE))
        .collect();

    // Kanten: ein Lauf über `links` in Speicherreihenfolge (die Links einer Seite liegen beisammen).
    let graph = {
        let mut graph = Graph::new();
        let mut buf: Vec<u32> = Vec::new();
        let mut current = NONE;
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id,
                follow <> 0
                AND (' ' || COALESCE(rel, '') || ' ') NOT LIKE '% nofollow %'
                AND (' ' || COALESCE(rel, '') || ' ') NOT LIKE '% ugc %'
                AND (' ' || COALESCE(rel, '') || ' ') NOT LIKE '% sponsored %'
            FROM links WHERE link_type = 'Hyperlink' AND crawlable = 1",
        )?;
        let mut rows = stmt.query([])?;
        let mut visited = 0u64;
        while let Some(row) = rows.next()? {
            visited += 1;
            if visited.is_multiple_of(8192) && cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            let Some(source) = routes
                .position(row.get(0)?)
                .map(|p| routes.node[p])
                .filter(|n| *n != NONE)
            else {
                continue;
            };
            let Some(target) = row
                .get::<_, Option<i64>>(1)?
                .and_then(|t| routes.position(t))
                .map(|p| resolved[p])
                .filter(|t| *t != NONE && *t != source)
            else {
                continue;
            };
            let follow = row.get::<_, bool>(2)? && node_follow[source as usize];
            if source != current {
                graph.flush(current, &mut buf);
                current = source;
            }
            buf.push(target | if follow { FOLLOW } else { 0 });
        }
        graph.flush(current, &mut buf);
        graph.finish()
    };
    drop(resolved);
    drop(routes);
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }

    let n = nodes.len();
    let mut degree = vec![0u64; n];
    for &(source, _, len) in &graph.segments {
        degree[source as usize] = len as u64;
    }
    let mut rank = vec![1.0 / n.max(1) as f64; n];
    let base = 0.15 / n.max(1) as f64;
    let mut next = vec![base; n];
    for iteration in 0..10 {
        if cancel.is_cancelled() {
            return Ok(StepState::Pending);
        }
        next.fill(base);
        for (i, &(source, start, len)) in graph.segments.iter().enumerate() {
            if i.is_multiple_of(8192) && cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            let source = source as usize;
            for &t in &graph.targets[start..start + len] {
                if t & FOLLOW != 0 {
                    let target = (t & !FOLLOW) as usize;
                    next[target] += 0.85 * rank[source] / degree[source] as f64;
                }
            }
        }
        std::mem::swap(&mut rank, &mut next);
        cancel.set_progress((iteration + 1) as f32 / 10.0);
    }
    drop(graph);
    publish(file, cancel, scaling, &nodes, &rank)
}

/// Schreibt Rohwert und Score je Knoten (`linkscore.raw`, `linkscore.score`) atomar; ein unveränderter
/// Stand rückt den Änderungsfeed nicht vor.
fn publish(
    file: &CrawlFile,
    cancel: &CancelToken,
    scaling: Scaling,
    nodes: &[i64],
    rank: &[f64],
) -> Result<StepState, AnalyzeError> {
    let conn = file.conn();
    let transform = |r: f64| match scaling {
        Scaling::Logarithmic => r.ln(),
        Scaling::Linear => r,
    };
    let min = rank.iter().copied().fold(f64::INFINITY, f64::min);
    let max = rank.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let score_of = |raw: f64| {
        if min == max {
            100
        } else {
            (1.0 + 99.0 * (transform(raw) - transform(min)) / (transform(max) - transform(min)))
                .round()
                .clamp(1.0, 100.0) as i64
        }
    };
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    let key_id = |key: &str| -> rusqlite::Result<Option<i64>> {
        tx.query_row("SELECT id FROM ext_keys WHERE key = ?1", [key], |r| r.get(0))
            .optional()
    };
    let (raw_key, score_key) = (key_id("linkscore.raw")?, key_id("linkscore.score")?);
    // Vorhandene Zeilen (nur wenn einer der Schlüssel je geschrieben wurde; sonst gibt es keine):
    // `(url_id, ist_raw, v_int, v_real)` nach `url_id`, `raw` vor `score` (Schlüsselname aufsteigend).
    let mut existing: Vec<(i64, bool, Option<i64>, Option<f64>)> = Vec::new();
    if raw_key.is_some() || score_key.is_some() {
        let mut stmt = tx.prepare(
            "SELECT url_id, key_id = ?1, v_int, v_real FROM url_ext_data WHERE key_id IN (?1, ?2)",
        )?;
        let mut rows = stmt.query(params![raw_key.unwrap_or(-1), score_key.unwrap_or(-1)])?;
        let mut visited = 0u64;
        while let Some(r) = rows.next()? {
            visited += 1;
            if visited.is_multiple_of(8192) && cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            existing.push((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?));
        }
        existing.sort_by_key(|e| (e.0, !e.1));
    }
    let unchanged = existing.len() == 2 * nodes.len()
        && nodes.iter().zip(rank).enumerate().all(|(i, (&id, &raw))| {
            let (r, s) = (&existing[2 * i], &existing[2 * i + 1]);
            r.0 == id && r.1 && r.3 == Some(raw) && s.0 == id && !s.1 && s.2 == Some(score_of(raw))
        });
    if unchanged {
        tx.commit()?;
        return Ok(StepState::Done);
    }
    let seq = os_store::queries::change_seq(conn)? + 1;
    // Vorhandene/neu veröffentlichte Scores melden sich über url_ext.change_seq.
    // Nur entfernte Knoten brauchen einen zusätzlichen Stempel auf urls.
    {
        let mut stamp = tx.prepare("UPDATE urls SET change_seq = ?1 WHERE id = ?2")?;
        let mut delete = tx.prepare("DELETE FROM url_ext_data WHERE url_id = ?1 AND key_id = ?2")?;
        let mut last = None;
        for &(url_id, is_raw, _, _) in &existing {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            let key = if is_raw { raw_key } else { score_key };
            delete.execute(params![url_id, key])?;
            if last != Some(url_id) && nodes.binary_search(&url_id).is_err() {
                stamp.execute(params![seq, url_id])?;
            }
            last = Some(url_id);
        }
    }
    if !nodes.is_empty() {
        let mut ensure = tx.prepare("INSERT OR IGNORE INTO ext_keys(key) VALUES (?1)")?;
        ensure.execute(["linkscore.raw"])?;
        ensure.execute(["linkscore.score"])?;
        let raw_key = key_id("linkscore.raw")?;
        let score_key = key_id("linkscore.score")?;
        let mut insert = tx.prepare(
            "INSERT INTO url_ext_data (url_id, key_id, v_int, v_real, v_text, change_seq) VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
        )?;
        for (i, (&id, &raw)) in nodes.iter().zip(rank).enumerate() {
            if i.is_multiple_of(1024) && cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            insert.execute(params![id, raw_key, Option::<i64>::None, raw, seq])?;
            insert.execute(params![id, score_key, score_of(raw), Option::<f64>::None, seq])?;
        }
    }
    tx.execute(
        "INSERT OR REPLACE INTO crawl_meta (key, value) VALUES ('change_seq', ?1)",
        [seq.to_string()],
    )?;
    tx.commit()?;
    Ok(StepState::Done)
}

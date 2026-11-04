//! B3-PERF: Link Score im Speicher. Gleichheitsnachweis gegen das bisherige SQL-Verfahren (Kanten in einer
//! Temp-Tabelle, zehn Durchläufe `ORDER BY source, target`): bitgleiche Rohwerte und gleiche Scores auf
//! Zufallsgraphen mit Weiterleitungen, Canonicals, nofollow/ugc/sponsored (auch in Großschreibung),
//! seitenweitem nofollow, Mehrfachlinks, nicht zusammenhängend gespeicherten Links einer Seite,
//! nicht crawlbaren Links, anderen Linktypen und lückenhaften URL-Kennungen.
#[path = "tab_links_support.rs"]
mod support;
use os_analyze::analysis::{steps::link_score, CancelToken};
use os_model::{CrawlConfig, StepState};
use support::*;

/// Das Verfahren vor B3-PERF (Stand beta.2, `link_score.rs`), nur bis zu den Rohwerten.
fn oracle(file: &os_store::CrawlFile) -> Vec<(i64, f64)> {
    struct Route {
        id: i64,
        next: Option<i64>,
        node: Option<usize>,
    }
    fn resolve(routes: &[Route], mut id: i64) -> Option<usize> {
        let mut seen = [0i64; 11];
        for depth in 0..=10 {
            if seen[..depth].contains(&id) {
                return None;
            }
            seen[depth] = id;
            let route = &routes[routes.binary_search_by_key(&id, |r| r.id).ok()?];
            if let Some(node) = route.node {
                return Some(node);
            }
            id = route.next?;
        }
        None
    }
    let conn = file.conn();
    conn.execute_batch("DROP TABLE IF EXISTS temp.o_routes; DROP TABLE IF EXISTS temp.o_edges; CREATE TEMP TABLE o_routes (id INTEGER PRIMARY KEY, node INTEGER, resolved INTEGER);").unwrap();
    let mut nodes = Vec::new();
    let mut routes = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, is_internal, state, content_class, status_code, redirect_type,
            redirect_target_id, canonical_target_id FROM urls ORDER BY id",
            )
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(r) = rows.next().unwrap() {
            let id: i64 = r.get(0).unwrap();
            let internal: bool = r.get(1).unwrap();
            let status: Option<i64> = r.get(4).unwrap();
            let redirect: Option<String> = r.get(5).unwrap();
            let canonical: Option<i64> = r.get(7).unwrap();
            let next = if redirect.is_some() || status.is_some_and(|s| (300..400).contains(&s)) {
                r.get(6).unwrap()
            } else {
                canonical.filter(|c| *c != id)
            };
            let participates = internal
                && r.get::<_, String>(2).unwrap() == "done"
                && r.get::<_, String>(3).unwrap() == "HTML"
                && status.is_some_and(|s| s > 0)
                && redirect.is_none()
                && !status.is_some_and(|s| (300..400).contains(&s))
                && canonical.is_none_or(|c| c == id);
            let node = participates.then_some(nodes.len());
            if participates {
                nodes.push(id);
            }
            routes.push(Route { id, next, node });
        }
    }
    for route in &routes {
        conn.execute(
            "INSERT INTO o_routes VALUES (?1, ?2, ?3)",
            rusqlite::params![
                route.id,
                route.node.map(|n| n as i64),
                resolve(&routes, route.id).map(|n| n as i64)
            ],
        )
        .unwrap();
    }
    conn.execute_batch("CREATE TEMP TABLE o_edges (source INTEGER, target INTEGER, follow INTEGER, PRIMARY KEY(source, target)) WITHOUT ROWID;
        INSERT INTO o_edges SELECT s.node, t.resolved,
            MAX(l.follow <> 0 AND COALESCE(u.effective_follow, 1) <> 0
                AND (' ' || COALESCE(l.rel, '') || ' ') NOT LIKE '% nofollow %'
                AND (' ' || COALESCE(l.rel, '') || ' ') NOT LIKE '% ugc %'
                AND (' ' || COALESCE(l.rel, '') || ' ') NOT LIKE '% sponsored %')
        FROM links l JOIN o_routes s ON s.id = l.source_id JOIN o_routes t ON t.id = l.target_id
        JOIN urls u ON u.id = l.source_id
        WHERE s.node IS NOT NULL AND t.resolved IS NOT NULL AND s.node <> t.resolved
            AND l.link_type = 'Hyperlink' AND l.crawlable = 1
        GROUP BY s.node, t.resolved;").unwrap();
    let n = nodes.len();
    let mut degree = vec![0u64; n];
    {
        let mut stmt = conn
            .prepare("SELECT source, COUNT(*) FROM o_edges GROUP BY source")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            degree[row.get::<_, usize>(0).unwrap()] = row.get(1).unwrap();
        }
    }
    let mut rank = vec![1.0 / n.max(1) as f64; n];
    let base = 0.15 / n.max(1) as f64;
    let mut next = vec![base; n];
    for _ in 0..10 {
        next.fill(base);
        let mut stmt = conn
            .prepare("SELECT source, target FROM o_edges WHERE follow = 1 ORDER BY source, target")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let source: usize = row.get(0).unwrap();
            let target: usize = row.get(1).unwrap();
            next[target] += 0.85 * rank[source] / degree[source] as f64;
        }
        std::mem::swap(&mut rank, &mut next);
    }
    conn.execute_batch("DROP TABLE temp.o_routes; DROP TABLE temp.o_edges;")
        .unwrap();
    nodes.into_iter().zip(rank).collect()
}

fn log_scores(raw: &[(i64, f64)]) -> Vec<(i64, i64)> {
    let min = raw.iter().map(|r| r.1).fold(f64::INFINITY, f64::min);
    let max = raw.iter().map(|r| r.1).fold(f64::NEG_INFINITY, f64::max);
    raw.iter()
        .map(|&(id, r)| {
            let s = if min == max {
                100
            } else {
                (1.0 + 99.0 * (r.ln() - min.ln()) / (max.ln() - min.ln()))
                    .round()
                    .clamp(1.0, 100.0) as i64
            };
            (id, s)
        })
        .collect()
}

fn stored(file: &os_store::CrawlFile) -> (Vec<(i64, f64)>, Vec<(i64, i64)>) {
    let read = |key: &str| -> Vec<(i64, Option<i64>, Option<f64>)> {
        let mut stmt = file
            .conn()
            .prepare("SELECT url_id, v_int, v_real FROM url_ext WHERE key = ?1 ORDER BY url_id")
            .unwrap();
        stmt.query_map([key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    (
        read("linkscore.raw")
            .into_iter()
            .map(|(id, _, r)| (id, r.unwrap()))
            .collect(),
        read("linkscore.score")
            .into_iter()
            .map(|(id, s, _)| (id, s.unwrap()))
            .collect(),
    )
}

/// Kleiner deterministischer Zufallsgenerator (xorshift64*), damit der Test ohne Zusatzcrate auskommt.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn random_site(file: &os_store::CrawlFile, seed: u64, pages: u64, gap: u64, scattered: bool) {
    let mut rng = Rng(seed);
    let conn = file.conn();
    conn.execute_batch("BEGIN").unwrap();
    // Lückenhafte Kennungen (`gap` > 1) prüfen die Auflösung ohne dichte Tabelle.
    let id = |i: u64| (i * gap + 1) as i64;
    for i in 0..pages {
        let roll = rng.below(100);
        let (class, status, state, internal) = match roll {
            0..=3 => ("Images", 200, "done", 1),
            4..=5 => ("HTML", 200, "queued", 1),
            6..=7 => ("HTML", 200, "done", 0),
            8 => ("HTML", 0, "done", 1),
            _ => ("HTML", 200, "done", 1),
        };
        conn.execute("INSERT INTO urls (id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,content_class,status_code,indexability) VALUES (?1,?2,?2,'https://site.test',?3,?4,'link',0,1,?5,?6,'Indexable')",
            rusqlite::params![id(i), format!("https://site.test/{i}"), internal, state, class, status]).unwrap();
    }
    for i in 0..pages {
        let target = id(rng.below(pages));
        match rng.below(100) {
            0..=6 => {
                conn.execute("UPDATE urls SET status_code=301, redirect_type='http', redirect_target_id=?2 WHERE id=?1", rusqlite::params![id(i), target]).unwrap();
            }
            7..=8 => {
                conn.execute(
                    "UPDATE urls SET status_code=302, redirect_target_id=?2 WHERE id=?1",
                    rusqlite::params![id(i), target],
                )
                .unwrap();
            }
            9..=14 => {
                conn.execute(
                    "UPDATE urls SET canonical_target_id=?2, canonical_self=0 WHERE id=?1",
                    rusqlite::params![id(i), target],
                )
                .unwrap();
            }
            15 => {
                conn.execute("UPDATE urls SET canonical_target_id=id WHERE id=?1", [id(i)])
                    .unwrap();
            }
            16..=18 => {
                conn.execute("UPDATE urls SET effective_follow=0 WHERE id=?1", [id(i)])
                    .unwrap();
            }
            19 => {
                conn.execute("UPDATE urls SET effective_follow=NULL WHERE id=?1", [id(i)])
                    .unwrap();
            }
            _ => {}
        }
    }
    let rels = [
        None,
        None,
        None,
        Some("nofollow"),
        Some("UGC"),
        Some("noopener Sponsored"),
        Some("nofollowx"),
        Some("external"),
    ];
    let mut insert = conn.prepare("INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow,rel,crawlable) VALUES(?1,?2,'/',?3,?4,?5,?6,?7)").unwrap();
    let mut emit = |rng: &mut Rng, source: i64, position: u64| {
        let target = match rng.below(40) {
            0 => None,
            1 => Some(source),
            2 => Some(9_999_999),
            _ => Some(id(rng.below(pages))),
        };
        let link_type = if rng.below(20) == 0 {
            "Image"
        } else {
            "Hyperlink"
        };
        insert
            .execute(rusqlite::params![
                source,
                target,
                link_type,
                position,
                i64::from(rng.below(6) != 0),
                rels[rng.below(rels.len() as u64) as usize],
                i64::from(rng.below(25) != 0)
            ])
            .unwrap();
    };
    for i in 0..pages {
        let links = rng.below(12);
        for p in 0..links {
            emit(&mut rng, id(i), p);
        }
    }
    // Nachträglich gespeicherte Links einiger Seiten (nicht zusammenhängend in `links`).
    for p in 0..if scattered { pages / 3 } else { 0 } {
        let source = id(rng.below(pages));
        emit(&mut rng, source, 100 + p);
    }
    drop(insert);
    conn.execute_batch("COMMIT").unwrap();
}

// spec: TECH-145
// spec: ONPAGE-017
#[test]
fn in_memory_graph_matches_previous_sql_iteration_bit_for_bit() {
    for (seed, pages, gap, scattered) in [
        (1u64, 60u64, 1u64, true),
        (7, 400, 1, true),
        (8, 400, 1, false),
        (42, 1500, 1, true),
        (43, 1500, 1, false),
        (99, 700, 9, true),
        (98, 700, 9, false),
    ] {
        let (_dir, file) = file();
        random_site(&file, seed, pages, gap, scattered);
        let expected = oracle(&file);
        assert!(expected.len() > 10, "seed {seed}: zu wenige Knoten");
        assert_eq!(
            link_score::run(&file, &CrawlConfig::default(), &CancelToken::new()).unwrap(),
            StepState::Done
        );
        let (raw, score) = stored(&file);
        assert_eq!(
            raw.iter().map(|r| (r.0, r.1.to_bits())).collect::<Vec<_>>(),
            expected
                .iter()
                .map(|r| (r.0, r.1.to_bits()))
                .collect::<Vec<_>>(),
            "seed {seed}"
        );
        assert_eq!(score, log_scores(&expected), "seed {seed}");
    }
}

// spec: TECH-145
// spec: TECH-139
#[test]
fn reanalysis_after_graph_change_replaces_rows_and_stamps_removed_nodes() {
    let (_dir, file) = file();
    random_site(&file, 5, 300, 1, true);
    let cfg = CrawlConfig::default();
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    let (before, _) = stored(&file);
    let seq = os_store::queries::change_seq(file.conn()).unwrap();
    // Ein Knoten verlässt V (wird Weiterleitung), ein verwaister Wert ohne URL-Zeile wird entfernt.
    let gone = before[3].0;
    sql(&file, &format!("UPDATE urls SET status_code=301, redirect_type='http', redirect_target_id={} WHERE id={gone};
        INSERT INTO url_ext(url_id,key,v_int) VALUES (777777,'linkscore.score',5)", before[0].0));
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    let expected = oracle(&file);
    let (raw, score) = stored(&file);
    assert_eq!(
        raw.iter().map(|r| (r.0, r.1.to_bits())).collect::<Vec<_>>(),
        expected
            .iter()
            .map(|r| (r.0, r.1.to_bits()))
            .collect::<Vec<_>>()
    );
    assert_eq!(score, log_scores(&expected));
    let new_seq = os_store::queries::change_seq(file.conn()).unwrap();
    assert_eq!(new_seq, seq + 1);
    let stamped: u64 = file
        .conn()
        .query_row("SELECT change_seq FROM urls WHERE id=?1", [gone], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(stamped, new_seq);
    let ext_seq: Vec<u64> = file
        .conn()
        .prepare("SELECT DISTINCT change_seq FROM url_ext WHERE key LIKE 'linkscore.%'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(ext_seq, vec![new_seq]);
    // Unverändert: kein weiterer Stempel.
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(os_store::queries::change_seq(file.conn()).unwrap(), new_seq);
}

//! Canonical-Ketten (TECH-026, OUT-086); gemeinsamer Walker für OUT-083.
//! Seiten werden je Kettenglied aus SQLite gelesen, nie als crawlweiter Graph gehalten.

use std::collections::BTreeSet;
use std::str::FromStr;

use crate::catalog::{TableEntry, TableJob, CHAIN_HEADER};
use crate::error::ExportError;
use crate::links_export::sql;
use crate::types::TableWriter;
use os_analyze::ColumnKind;
use os_model::{RedirectType, Value};

pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}
const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Canonicals:Canonical Chains",
    aliases: &[],
    file_stem: "Canonicals:Canonical Chains",
    header: CHAIN_HEADER,
    header_fn: Some(header),
    requires: &[],
    owner: "M2-D04",
    run,
}];

#[derive(Clone)]
struct Node {
    id: Option<i64>,
    address: String,
    code: Option<i64>,
    status: Option<String>,
    content_type: Option<String>,
    indexability: Option<String>,
    indexability_status: Option<String>,
    state: String,
    target: Option<i64>,
    target_url: Option<String>,
    kind: Option<String>,
    temporary: bool,
    limit: bool,
}

fn load(job: &TableJob<'_>, id: i64) -> Result<Node, ExportError> {
    let mut node = sql(job.file.conn().query_row(
        "SELECT u.address, u.status_code, u.status, u.content_type, u.indexability, u.indexability_status,
            u.state, u.redirect_target_id, u.redirect_url, u.redirect_type, u.meta_refresh_delay,
            u.canonical_target_id, u.canonical_url, u.canonical_self,
            (SELECT c.source FROM canonicals c WHERE c.url_id = u.id AND c.resolved IS NOT NULL
                ORDER BY CASE WHEN c.source = 'http' THEN 0 WHEN c.in_head = 1 THEN 1 ELSE 2 END,
                c.position LIMIT 1),
            EXISTS(SELECT 1 FROM url_flags f WHERE f.url_id = u.id AND f.flag = 'redirect_limit_reached')
         FROM urls u WHERE u.id = ?1", [id], |r| {
            let redirect = r.get::<_, Option<String>>(9)?.and_then(|s| RedirectType::from_str(&s).ok());
            let code = r.get::<_, Option<i64>>(1)?;
            let delay = r.get::<_, Option<i64>>(10)?.unwrap_or(0);
            let canonical = job.cfg.spider.canonicals.store && r.get::<_, Option<bool>>(13)? == Some(false);
            let (target, target_url, kind) = if let Some(kind) = redirect {
                (r.get(7)?, r.get(8)?, Some(kind.report_label().to_string()))
            } else if canonical {
                (r.get(11)?, r.get(12)?, Some(if r.get::<_, Option<String>>(14)?.as_deref() == Some("http") {
                    "HTTP Canonical".to_string()
                } else { "Canonical".to_string() }))
            } else { (None, None, None) };
            Ok(Node {
                id: Some(id), address: r.get(0)?, code, status: r.get(2)?, content_type: r.get(3)?,
                indexability: r.get(4)?, indexability_status: r.get(5)?, state: r.get(6)?, target, target_url, kind,
                temporary: match redirect {
                    Some(RedirectType::Http) => matches!(code, Some(302 | 303 | 307)),
                    Some(RedirectType::MetaRefresh | RedirectType::HttpRefresh) => delay > 0,
                    _ => false,
                }, limit: r.get(15)?,
            })
        }))?;
    // Auch importierte/ältere Dateien können nur die Zieladresse statt target_id enthalten.
    if node.target.is_none() {
        if let Some(address) = &node.target_url {
            let mut stmt = sql(job
                .file
                .conn()
                .prepare_cached("SELECT id FROM urls WHERE url = ?1"))?;
            let mut rows = sql(stmt.query([address]))?;
            if let Some(row) = sql(rows.next())? {
                node.target = Some(sql(row.get(0))?);
            }
        }
    }
    Ok(node)
}

#[derive(PartialEq)]
enum Outcome {
    End,
    Loop,
    Limit,
    NotFollowed,
}
struct Chain {
    nodes: Vec<Node>,
    targets: Vec<String>,
    outcome: Outcome,
}
impl Chain {
    fn canonical(&self) -> bool {
        self.nodes.iter().take(self.targets.len()).any(is_canonical)
    }
    fn redirect(&self) -> bool {
        self.nodes
            .iter()
            .take(self.targets.len())
            .any(|n| !is_canonical(n))
    }
    fn selected(&self, pure: bool, list: bool) -> bool {
        // Ein einzelner nicht verfolgter Sprung bleibt im Standard-Listenlauf sichtbar (OUT-085).
        (self.targets.len() >= 2
            || self.outcome == Outcome::Loop
            || (list && self.outcome == Outcome::NotFollowed))
            && (!pure || (self.canonical() && !self.redirect()))
    }
    fn chain_type(&self) -> String {
        match (self.canonical(), self.redirect()) {
            (true, true) => "Redirect & Canonical".into(),
            (true, false) => "Canonical".into(),
            _ => {
                let kinds: BTreeSet<_> = self
                    .nodes
                    .iter()
                    .take(self.targets.len())
                    .filter_map(|n| n.kind.as_deref())
                    .collect();
                if kinds.len() == 1 {
                    kinds.into_iter().next().unwrap_or("").into()
                } else {
                    "Mixed".into()
                }
            }
        }
    }
}
fn is_canonical(node: &Node) -> bool {
    matches!(node.kind.as_deref(), Some("Canonical" | "HTTP Canonical"))
}

fn walk(job: &TableJob<'_>, start: i64) -> Result<Chain, ExportError> {
    let mut nodes = vec![load(job, start)?];
    let mut seen = BTreeSet::from([start]);
    let mut targets = Vec::new();
    let outcome = loop {
        let from = nodes.last().expect("Startglied");
        if from.state != "done" {
            break Outcome::NotFollowed;
        }
        let Some(address) = from.target_url.clone() else {
            break Outcome::End;
        };
        if from.limit || targets.len() >= job.cfg.limits.max_redirects as usize {
            break Outcome::Limit;
        }
        let id = from.target;
        // Das fragmentfreie Ziel entspricht dem Store-Vergleichsschlüssel (TECH-002).
        let next = if let Some(id) = id {
            load(job, id)?
        } else {
            Node {
                id: None,
                address,
                code: None,
                status: None,
                content_type: None,
                indexability: None,
                indexability_status: None,
                state: "discovered".into(),
                target: None,
                target_url: None,
                kind: None,
                temporary: false,
                limit: false,
            }
        };
        targets.push(next.address.clone());
        if next.id.is_some_and(|id| !seen.insert(id)) {
            break Outcome::Loop;
        }
        nodes.push(next);
    };
    Ok(Chain {
        nodes,
        targets,
        outcome,
    })
}

/// Ein unkorrelierter Linkscan, auch in pausierten Dateien ohne links_target-Index.
const STARTS: &str = "WITH samples AS (
        SELECT l.target_id, MIN(l.source_id) source_id FROM links l JOIN urls s ON s.id = l.source_id
        WHERE s.is_internal = 1 AND l.link_type = 'Hyperlink' AND l.target_id IS NOT NULL GROUP BY l.target_id
    )
    SELECT u.id, COALESCE(uc.inlinks, 0), s.address FROM urls u
    LEFT JOIN url_counters uc ON uc.url_id = u.id
    LEFT JOIN samples ON samples.target_id = u.id LEFT JOIN urls s ON s.id = samples.source_id
    LEFT JOIN list_input li ON li.url_id = u.id
    WHERE u.in_tables = 1 AND u.state = 'done'
      AND (u.redirect_url IS NOT NULL OR (:store = 1 AND u.canonical_self = 0 AND u.canonical_url IS NOT NULL))
      AND ((:list = 1 AND li.url_id IS NOT NULL) OR (:list = 0 AND (
        u.discovered_via IN ('start', 'list_input', 'sitemap') OR samples.source_id IS NOT NULL OR u.id NOT IN (
          SELECT redirect_target_id FROM urls WHERE redirect_target_id IS NOT NULL AND redirect_target_id <> id
          UNION SELECT canonical_target_id FROM urls WHERE :store = 1 AND canonical_target_id IS NOT NULL AND canonical_target_id <> id))))
    GROUP BY u.id ORDER BY CASE WHEN :list = 1 THEN MIN(li.position) ELSE u.id END";

fn visit(
    job: &TableJob<'_>,
    pure: bool,
    mut f: impl FnMut(&Chain, i64, Option<String>) -> Result<(), ExportError>,
) -> Result<(), ExportError> {
    let list: bool = sql(job.file.conn().query_row(
        "SELECT value = 'list' FROM crawl_meta WHERE key = 'mode'",
        [],
        |r| r.get(0),
    ))?;
    let mut stmt = sql(job.file.conn().prepare(STARTS))?;
    sql(stmt.raw_bind_parameter(
        stmt.parameter_index(":list")
            .map_err(os_store::StoreError::from)?
            .expect("list"),
        list,
    ))?;
    sql(stmt.raw_bind_parameter(
        stmt.parameter_index(":store")
            .map_err(os_store::StoreError::from)?
            .expect("store"),
        job.cfg.spider.canonicals.store,
    ))?;
    let mut rows = stmt.raw_query();
    while let Some(row) = sql(rows.next())? {
        let chain = walk(job, sql(row.get(0))?)?;
        if chain.selected(pure, list) {
            f(&chain, sql(row.get(1))?, sql(row.get(2))?)?;
        }
    }
    Ok(())
}

pub(crate) fn make_header(job: &TableJob<'_>, pure: bool) -> Result<Vec<String>, ExportError> {
    let mut hops = 0;
    visit(job, pure, |chain, _, _| {
        hops = hops.max(chain.targets.len());
        Ok(())
    })?;
    let mut columns: Vec<_> = CHAIN_HEADER.iter().map(|s| (*s).to_string()).collect();
    for n in 1..=hops {
        columns.extend([
            format!("Redirect Type {n}"),
            format!("Redirect URL {n}"),
            format!("Status Code {n}"),
            format!("Status {n}"),
        ]);
    }
    Ok(columns)
}
fn header(job: &TableJob<'_>) -> Result<Vec<String>, ExportError> {
    make_header(job, true)
}
fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    write(job, writer, true)
}
fn text(s: Option<&str>) -> Value {
    s.map_or(Value::Null, |s| Value::Text(s.into()))
}
fn boolean(value: bool) -> Value {
    text(Some(if value { "True" } else { "False" }))
}

pub(crate) fn write(
    job: &TableJob<'_>,
    writer: &mut dyn TableWriter,
    pure: bool,
) -> Result<u64, ExportError> {
    let width = make_header(job, pure)?.len();
    let mut count = 0;
    visit(job, pure, |chain, inlinks, sample| {
        let last = chain.nodes.last().expect("Startglied");
        let mut values = vec![
            text(Some(&chain.chain_type())),
            Value::Int(chain.targets.len() as i64),
            boolean(chain.outcome == Outcome::Loop),
            boolean(
                chain
                    .nodes
                    .iter()
                    .take(chain.targets.len())
                    .any(|n| n.temporary),
            ),
            text(Some(&chain.nodes[0].address)),
            text(Some(&last.address)),
            last.code.map_or(Value::Null, Value::Int),
            text(last.status.as_deref()),
            text(last.content_type.as_deref()),
            text(last.indexability.as_deref()),
            text(last.indexability_status.as_deref()),
            Value::Int(inlinks),
            text(sample.as_deref()),
        ];
        match chain.outcome {
            Outcome::Loop => {
                for value in &mut values[5..11] {
                    *value = Value::Null;
                }
                values[7] = text(Some("Redirect Loop"));
            }
            Outcome::Limit => values[7] = text(Some("Max Redirects Reached")),
            Outcome::NotFollowed => {
                for value in &mut values[6..11] {
                    *value = Value::Null;
                }
                values[7] = text(Some("Not Followed"));
            }
            Outcome::End => {}
        }
        let mut kinds = vec![ColumnKind::Text; width];
        for n in [1, 6, 11] {
            kinds[n] = ColumnKind::Int;
        }
        for (i, target) in chain.targets.iter().enumerate() {
            let node = &chain.nodes[i];
            values.extend([
                text(node.kind.as_deref()),
                text(Some(target)),
                node.code.map_or(Value::Null, Value::Int),
                text(node.status.as_deref()),
            ]);
            kinds[CHAIN_HEADER.len() + 4 * i + 2] = ColumnKind::Int;
        }
        values.resize(width, Value::Null);
        writer.row(&kinds, &values)?;
        count += 1;
        Ok(())
    })?;
    Ok(count)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    mod support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/tech-m2-canonicals/support.rs"
        ));
    }
    pub(crate) use support::{analyse, fixture, Fixture};
    pub(crate) struct Table {
        pub header: Vec<String>,
        pub rows: Vec<Vec<String>>,
    }
    impl Table {
        pub fn cell<'a>(&self, row: &'a [String], column: &str) -> &'a str {
            &row[self.header.iter().position(|h| h == column).unwrap()]
        }
        fn find(&self, name: &str) -> Option<&Vec<String>> {
            self.rows
                .iter()
                .find(|r| self.cell(r, "Address").ends_with(&format!("/{name}")))
        }
        fn row(&self, name: &str) -> &Vec<String> {
            self.find(name)
                .unwrap_or_else(|| panic!("{name} fehlt: {:?}", self.rows))
        }
    }
    pub(crate) fn read(path: &std::path::Path) -> Table {
        let bytes = std::fs::read(path).unwrap();
        let mut csv =
            csv::Reader::from_reader(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes));
        let header = csv.headers().unwrap().iter().map(str::to_string).collect();
        let rows = csv
            .records()
            .map(|r| r.unwrap().iter().map(str::to_string).collect())
            .collect();
        Table { header, rows }
    }
    fn report(fx: &Fixture, mixed: bool) -> Table {
        let name = if mixed {
            "Redirects:Redirect & Canonical Chains"
        } else {
            "Canonicals:Canonical Chains"
        };
        let result = crate::save_report(
            &fx.file,
            &fx.cfg,
            name,
            &fx.dir.path().join("reports"),
            crate::Format::Csv,
        )
        .unwrap();
        read(&result.files[0].path)
    }

    // spec: TECH-025
    // spec: TECH-026
    // spec: OUT-086
    #[test]
    fn canonical_chains_include_header_edges_and_loops_exclude_single_edges_and_intermediates() {
        let fx = fixture();
        let table = report(&fx, false);
        assert_eq!(table.rows.len(), 2);
        let row = table.row("a");
        assert_eq!(table.cell(row, "Chain Type"), "Canonical");
        assert_eq!(table.cell(row, "Number of Redirects"), "2");
        assert_eq!(table.cell(row, "Redirect Type 1"), "Canonical");
        assert_eq!(table.cell(row, "Redirect Type 2"), "HTTP Canonical");
        assert_eq!(table.cell(row, "Final Address"), "https://site.test/c");
        assert_eq!(table.cell(row, "Status Code 2"), "200");
        assert_eq!(table.cell(row, "Inlinks"), "1");
        assert_eq!(table.cell(row, "Sample Source"), "https://site.test/home");
        for name in [
            "b",
            "one",
            "only",
            "mixed",
            "not-followed",
            "respect-hidden",
        ] {
            assert!(table.find(name).is_none(), "{name}");
        }
        let loop_row = table.row("loop-a");
        assert_eq!(table.cell(loop_row, "Redirect Loop"), "True");
        assert_eq!(table.cell(loop_row, "Final Status"), "Redirect Loop");
        assert_eq!(table.cell(loop_row, "Final Address"), "");
        assert_eq!(table.header.len(), 21);
        fx.file
            .conn()
            .execute_batch("DROP INDEX links_target")
            .unwrap();
        assert_eq!(report(&fx, false).rows, table.rows);
    }

    // spec: OUT-083
    #[test]
    fn combined_report_includes_pure_canonical_redirect_and_mixed_chains() {
        let fx = fixture();
        let table = report(&fx, true);
        assert_eq!(table.rows.len(), 4);
        assert_eq!(table.cell(table.row("a"), "Chain Type"), "Canonical");
        let row = table.row("mixed");
        assert_eq!(table.cell(row, "Chain Type"), "Redirect & Canonical");
        assert_eq!(table.cell(row, "Redirect Type 1"), "Canonical");
        assert_eq!(table.cell(row, "Redirect Type 2"), "HTTP Redirect");
        assert_eq!(table.cell(row, "Temp Redirect in Chain"), "True");
        assert_eq!(table.cell(row, "Final Address"), "https://site.test/final");
        assert_eq!(
            table.cell(table.row("redirect-a"), "Chain Type"),
            "HTTP Redirect"
        );
        fx.file.conn().execute("UPDATE urls SET redirect_type='meta_refresh',meta_refresh_delay=0,status_code=200 WHERE id=32",[]).unwrap();
        let table = report(&fx, true);
        assert_eq!(table.cell(table.row("redirect-a"), "Chain Type"), "Mixed");
        fx.file
            .conn()
            .execute(
                "UPDATE urls SET redirect_type='hsts_policy',status_code=307 WHERE id=6",
                [],
            )
            .unwrap();
        let table = report(&fx, true);
        assert_eq!(
            table.cell(table.row("mixed"), "Temp Redirect in Chain"),
            "False"
        );
    }

    // spec: TECH-026
    // spec: OUT-086
    #[test]
    fn independently_linked_intermediate_limit_and_unknown_target_determine_rows_and_dynamic_width()
    {
        let mut fx = fixture();
        fx.file.conn().execute_batch("UPDATE urls SET canonical_url='https://site.test/final',canonical_target_id=7,canonical_self=0 WHERE id=4;
            INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow) VALUES (1,3,'/b','Hyperlink',200,1);").unwrap();
        let table = report(&fx, false);
        assert_eq!(table.header.len(), 25);
        assert_eq!(table.cell(table.row("a"), "Number of Redirects"), "3");
        assert_eq!(table.cell(table.row("b"), "Number of Redirects"), "2");
        fx.cfg.limits.max_redirects = 2;
        let table = report(&fx, false);
        assert_eq!(table.header.len(), 21);
        assert_eq!(
            table.cell(table.row("a"), "Final Status"),
            "Max Redirects Reached"
        );
        fx.file.conn().execute("UPDATE urls SET canonical_target_id=28,canonical_url='https://site.test/unknown' WHERE id=3",[]).unwrap();
        let table = report(&fx, false);
        assert_eq!(table.cell(table.row("a"), "Final Status"), "Not Followed");
        assert_eq!(table.cell(table.row("a"), "Final Status Code"), "");
    }

    // spec: OUT-085 [P1]
    #[test]
    fn standard_list_report_keeps_not_followed_and_each_uploaded_start_in_order() {
        let fx = fixture();
        fx.file.conn().execute_batch("UPDATE crawl_meta SET value='list' WHERE key='mode';
            INSERT INTO list_input(position,original,url_id,outcome) VALUES (0,'/not-followed',29,'accepted'),(1,'/a',2,'accepted'),(2,'/loop-a',8,'accepted');").unwrap();
        let table = report(&fx, false);
        assert_eq!(table.rows.len(), 3);
        assert_eq!(
            table.cell(&table.rows[0], "Address"),
            "https://site.test/not-followed"
        );
        assert_eq!(table.cell(&table.rows[0], "Final Status"), "Not Followed");
        assert_eq!(table.cell(&table.rows[1], "Address"), "https://site.test/a");
        assert_eq!(
            table.cell(&table.rows[2], "Address"),
            "https://site.test/loop-a"
        );
    }

    // spec: OUT-086
    #[test]
    fn empty_csv_has_fixed_header_and_xlsx_has_matching_chain_cells() {
        use calamine::{open_workbook_auto, Data, Reader};
        let mut fx = fixture();
        fx.cfg.spider.canonicals.store = false;
        let table = report(&fx, false);
        assert_eq!(table.header, CHAIN_HEADER);
        assert!(table.rows.is_empty());
        fx.cfg.spider.canonicals.store = true;
        let result = crate::save_report(
            &fx.file,
            &fx.cfg,
            "Canonicals:Canonical Chains",
            &fx.dir.path().join("xlsx"),
            crate::Format::Xlsx,
        )
        .unwrap();
        let mut workbook = open_workbook_auto(&result.files[0].path).unwrap();
        let range = workbook.worksheet_range_at(0).unwrap().unwrap();
        assert_eq!(
            range.get((0, 17)),
            Some(&Data::String("Redirect Type 2".into()))
        );
        assert_eq!(range.get((1, 1)), Some(&Data::Float(2.0)));
    }
}

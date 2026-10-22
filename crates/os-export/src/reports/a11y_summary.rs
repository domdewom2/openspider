use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::{ExportError, TableWriter};
use os_analyze::tabs::accessibility as a11y;
use os_model::Value;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Accessibility:Violations Summary",
    aliases: &[],
    file_stem: "Accessibility:Violations Summary",
    header: &[
        "Issue",
        "Rule ID",
        "Guideline",
        "Impact",
        "Priority",
        "URLs",
        "% of Total",
        "Sample URL",
    ],
    requires: &[
        ReportRequirement::Rendering,
        ReportRequirement::Config {
            key: "extraction.accessibility",
            what: "accessibility extraction",
        },
    ],
    owner: "M2-D21",
    run,
    header_fn: None,
}];

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let total: i64 = crate::links_export::sql(job.file.conn().query_row(
        &format!("SELECT COUNT(*) FROM urls u WHERE {}", a11y::POPULATION),
        [],
        |r| r.get(0),
    ))?;
    let sql=format!("SELECT a.rule_id,CASE MIN(CASE a.impact WHEN 'critical' THEN 0 WHEN 'serious' THEN 1 WHEN 'moderate' THEN 2 WHEN 'minor' THEN 3 ELSE 4 END) WHEN 0 THEN 'critical' WHEN 1 THEN 'serious' WHEN 2 THEN 'moderate' WHEN 3 THEN 'minor' END,COUNT(DISTINCT a.url_id),MIN(u.address) FROM a11y_rules a JOIN urls u ON u.id=a.url_id WHERE {} AND a.status='violation' GROUP BY a.rule_id ORDER BY a.rule_id",a11y::POPULATION);
    let mut stmt = crate::links_export::sql(job.file.conn().prepare(&sql))?;
    let mut rows = crate::links_export::sql(stmt.query([]))?;
    let mut collected = Vec::new();
    while let Some(row) = crate::links_export::sql(rows.next())? {
        let id: String = crate::links_export::sql(row.get(0))?;
        let Some(rule) = a11y::rule(&id) else {
            continue;
        };
        let impact: Option<String> = crate::links_export::sql(row.get(1))?;
        let urls: i64 = crate::links_export::sql(row.get(2))?;
        let values = vec![
            rule.issue.clone().into(),
            id.into(),
            rule.group.clone().into(),
            a11y::impact(impact.as_deref()).into(),
            rule.priority.clone().into(),
            Value::Int(urls),
            Value::Float(if total == 0 {
                0.0
            } else {
                100.0 * urls as f64 / total as f64
            }),
            Value::Text(crate::links_export::sql(row.get(3))?),
        ];
        let mut kinds = [os_analyze::ColumnKind::Text; 8];
        kinds[5] = os_analyze::ColumnKind::Int;
        kinds[6] = os_analyze::ColumnKind::Float { decimals: 2 };
        collected.push((rule.priority.clone(), urls, kinds, values));
    }
    collected.sort_by(|a, b| {
        let rank = |p: &str| match p {
            "High" => 0,
            "Medium" => 1,
            _ => 2,
        };
        rank(&a.0).cmp(&rank(&b.0)).then(b.1.cmp(&a.1))
    });
    let count = collected.len() as u64;
    for (_, _, kinds, values) in collected {
        writer.row(&kinds, &values)?;
    }
    Ok(count)
}

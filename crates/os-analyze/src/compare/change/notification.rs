//! Abschlussmail-Inhalt aus dem fertigen Crawl und seinem gespeicherten Vergleich (OUT-152).
use super::super::{CompareError, Comparison};
use os_model::CancelToken;
use os_store::CrawlFile;

/// Eine Zeile der Änderungstabelle; Zahlen bleiben auch für Renderer zugänglich.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeRow {
    pub label: String,
    pub previous: u64,
    pub current: u64,
    pub change: i64,
    pub percent: Option<f64>,
}

/// Dieselben Filter und Elemente wie beim manuell geöffneten Vergleich.
pub fn rows(
    c: &mut Comparison,
    config: &os_model::ChangeDetectionConfig,
    skip_empty: bool,
) -> Result<Vec<ChangeRow>, CompareError> {
    let mut rows: Vec<_> = c
        .overview(&CancelToken::new())?
        .into_iter()
        .filter_map(|f| {
            Some(ChangeRow {
                label: format!("{} / {}", f.tab, f.label),
                previous: f.previous?,
                current: f.current?,
                change: f.change()?,
                percent: f.change_pct(),
            })
        })
        .collect();
    let pairing = c.pairing();
    for (label, previous, current, change) in [
        (
            "New",
            0,
            (&pairing.current - &pairing.paired_current).len(),
            1,
        ),
        (
            "Missing",
            (&pairing.previous - &pairing.paired_previous).len(),
            0,
            -1,
        ),
    ] {
        rows.push(ChangeRow {
            label: label.into(),
            previous,
            current,
            change: change * (previous + current) as i64,
            percent: (previous > 0).then_some(-100.0),
        });
    }
    for count in super::counts(c, config, false)? {
        // Elementzeilen zählen geänderte URL-Paare, keine Bestandsdifferenz.
        rows.push(ChangeRow {
            label: count.element,
            previous: 0,
            current: count.change,
            change: count.change as i64,
            percent: None,
        });
    }
    if skip_empty {
        rows.retain(|r| r.change != 0);
    }
    rows.sort_by(|a, b| {
        b.change
            .unsigned_abs()
            .cmp(&a.change.unsigned_abs())
            .then(a.label.cmp(&b.label))
    });
    Ok(rows)
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Crawl-Metadaten bewahren die normalisierte Start-URL einschließlich Userinfo.
/// In der Abschlussmail darf diese Benutzername/Passwort-Komponente nicht erscheinen.
fn start_address(value: &str) -> String {
    let Some((scheme, rest)) = value.split_once("://") else {
        return String::new();
    };
    if !matches!(scheme, "http" | "https") {
        return String::new();
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = rest[..end].rsplit('@').next().unwrap_or_default();
    format!("{scheme}://{authority}{}", &rest[end..])
}

/// Fertige Alternativen für den SMTP-Adapter; kein Exportinhalt und keine Zugangsdaten.
pub struct Content {
    pub text: String,
    pub html: String,
}

/// Baut Grunddaten, Issue-Tabelle und Auto-Compare-Anhang für den tatsächlichen Versand.
pub fn content(
    file: &CrawlFile,
    task: &str,
    auto_compare: bool,
    skip_empty: bool,
) -> Result<Content, CompareError> {
    let meta = file.meta()?;
    let cfg = file.config()?;
    let mut text = format!(
        "Task: {task}\nProject: {}\nStart URL: {}\nStart: {}\nEnd: {}\n",
        meta.project,
        start_address(meta.start_url.as_deref().unwrap_or("")),
        meta.started_at.map(|v| v.to_db()).unwrap_or_default(),
        meta.ended_at.map(|v| v.to_db()).unwrap_or_default()
    );
    if let (Some(start), Some(end)) = (meta.started_at, meta.ended_at) {
        text.push_str(&format!(
            "Duration ms: {}\n",
            end.unix_millis() - start.unix_millis()
        ));
    }
    let mut html = format!("<h1>{}</h1><pre>{}</pre>", escape(task), escape(&text));
    text.push_str("Status Code\tURLs\n");
    let mut stmt = file.conn().prepare("SELECT status_code,COUNT(*) FROM urls WHERE state='done' GROUP BY status_code ORDER BY status_code")?;
    for item in stmt.query_map([], |r| {
        Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, u64>(1)?))
    })? {
        let (status, count) = item?;
        let line = format!(
            "{}\t{count}\n",
            status.map(|s| s.to_string()).unwrap_or_default()
        );
        html.push_str(&format!("<pre>{}</pre>", escape(&line)));
        text.push_str(&line);
    }
    let tabs = crate::tabs::configured_for_file(&cfg, file);
    text.push_str("Issue / Filter\tType\tPriority\tURLs\n");
    html.push_str(
        "<table><tr><th>Issue / Filter</th><th>Type</th><th>Priority</th><th>URLs</th></tr>",
    );
    for tab in &tabs {
        if tab.id == super::super::CHANGE_DETECTION_TAB || tab.is_stub() {
            continue;
        }
        for filter in &tab.filters {
            if filter.is_stub() {
                continue;
            }
            let Some(issue) = crate::issues::entry_for(tab.id, filter.id) else {
                continue;
            };
            let count = super::super::classify::filter_set(&tabs, file, &cfg, tab.id, filter.id)?
                .ids
                .map(|v| v.len())
                .unwrap_or(0);
            if count == 0 {
                continue;
            }
            let label = format!("{} / {}", tab.id, filter.id);
            text.push_str(&format!(
                "{label}\t{:?}\t{:?}\t{count}\n",
                issue.issue_type, issue.priority
            ));
            html.push_str(&format!(
                "<tr><td>{}</td><td>{:?}</td><td>{:?}</td><td>{count}</td></tr>",
                escape(&label),
                issue.issue_type,
                issue.priority
            ));
        }
    }
    html.push_str("</table>");
    if auto_compare {
        if let Some(path) = file.get_meta("compare.latest")? {
            let (mut comparison, config) =
                super::auto::load(std::path::Path::new(&path), &CancelToken::new())?;
            // Das gespeicherte Artefakt kann von einem expliziten CompareConfig stammen.
            super::calculate(&comparison, &config.change_detection, &CancelToken::new())?;
            let table = rows(&mut comparison, &config.change_detection, skip_empty)?;
            text.push_str("Issue / Filter\tPrevious\tCurrent\tChange\tChange %\n");
            html.push_str("<table><tr><th>Issue / Filter</th><th>Previous</th><th>Current</th><th>Change</th><th>Change %</th></tr>");
            for row in table {
                let pct = row.percent.map(|p| format!("{p:.2}")).unwrap_or_default();
                text.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{pct}\n",
                    row.label, row.previous, row.current, row.change
                ));
                let strong =
                    row.change.unsigned_abs() >= 10 && row.percent.is_none_or(|p| p.abs() >= 10.0);
                let tag = if strong { "strong" } else { "span" };
                html.push_str(&format!("<tr><td><{tag}>{}</{tag}></td><td>{}</td><td>{}</td><td>{}</td><td>{pct}</td></tr>",escape(&row.label),row.previous,row.current,row.change));
            }
            html.push_str("</table>");
        } else if file.get_meta("compare.auto_state")?.as_deref() == Some("no_previous") {
            text.push_str("kein vorheriger Crawl\n");
            html.push_str("<p>kein vorheriger Crawl</p>");
        } else {
            return Err(CompareError::Invalid(
                "Auto comparison result missing for completion mail".into(),
            ));
        }
    }
    // Auch ohne Vergleich werden Gesamtzahl und Indexierbarkeit aus demselben Crawl gelesen.
    for (label, predicate) in [
        ("URLs", "1"),
        ("Indexable", "indexability='Indexable'"),
        ("Non-Indexable", "indexability='Non-Indexable'"),
    ] {
        let n: u64 = file.conn().query_row(
            &format!("SELECT COUNT(*) FROM urls WHERE state='done' AND ({predicate})"),
            [],
            |r| r.get(0),
        )?;
        text.push_str(&format!("{label}: {n}\n"));
        html.push_str(&format!("<p>{label}: {n}</p>"));
    }
    Ok(Content { text, html })
}

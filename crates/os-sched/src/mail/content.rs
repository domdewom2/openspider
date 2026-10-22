use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Issue {
    pub name: String,
    pub kind: String,
    pub priority: String,
    pub urls: u64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Change {
    pub name: String,
    pub previous: u64,
    pub current: u64,
}
impl Change {
    pub fn delta(&self) -> i128 {
        i128::from(self.current) - i128::from(self.previous)
    }
    pub fn percent(&self) -> Option<f64> {
        (self.previous > 0).then(|| self.delta() as f64 * 100.0 / self.previous as f64)
    }
    pub fn strong(&self) -> bool {
        self.delta().abs() >= 10 && self.percent().is_none_or(|p| p.abs() >= 10.0)
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Completion {
    pub task: Option<String>,
    pub project: String,
    pub start_url: String,
    pub started: String,
    pub ended: String,
    pub duration_ms: i64,
    pub overview: Vec<(String, u64)>,
    pub response_codes: Vec<(String, u64)>,
    pub issues: Vec<Issue>,
    pub auto_compare: bool,
    pub changes: Option<Vec<Change>>,
    pub skip_empty: bool,
    pub error: Option<String>,
}
fn clean(s: &str) -> String {
    os_fetch::redact::text(s).replace(['\r', '\n'], " ")
}
fn html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
impl Completion {
    pub fn subject(&self) -> String {
        format!(
            "{}OpenSpider: {}",
            if self.error.is_some() {
                "[FAILED] "
            } else {
                ""
            },
            clean(self.task.as_deref().unwrap_or(&self.project))
        )
    }
    pub fn render(&self) -> (String, String) {
        let mut lines = vec![
            format!("Aufgabe: {}", clean(self.task.as_deref().unwrap_or("–"))),
            format!("Projekt: {}", clean(&self.project)),
            format!("Start-URL: {}", os_fetch::redact::url(&self.start_url)),
            format!("Start: {}", clean(&self.started)),
            format!("Ende: {}", clean(&self.ended)),
            format!("Dauer: {} ms", self.duration_ms),
        ];
        if let Some(e) = &self.error {
            lines.push(format!("Fehler: {}", clean(e)));
        }
        for (name, n) in &self.overview {
            lines.push(format!("{}: {n}", clean(name)));
        }
        lines.push("Antwortcodes".into());
        for (name, n) in &self.response_codes {
            lines.push(format!("{}: {n}", clean(name)));
        }
        lines.push("Issues: Name | Typ | Priorität | URLs".into());
        for i in &self.issues {
            lines.push(format!(
                "{} | {} | {} | {}",
                clean(&i.name),
                clean(&i.kind),
                clean(&i.priority),
                i.urls
            ));
        }
        let mut body = format!("<html><body><pre>{}</pre>", html(&lines.join("\n")));
        if self.auto_compare {
            if let Some(changes) = &self.changes {
                lines.push("Issue / Filter | Previous | Current | Change | Change %".into());
                body.push_str("<table><tr><th>Issue / Filter</th><th>Previous</th><th>Current</th><th>Change</th><th>Change %</th></tr>");
                let mut changes = changes
                    .iter()
                    .filter(|r| !self.skip_empty || r.delta() != 0)
                    .collect::<Vec<_>>();
                changes.sort_by(|a, b| {
                    b.delta()
                        .abs()
                        .cmp(&a.delta().abs())
                        .then_with(|| a.name.cmp(&b.name))
                });
                for r in changes {
                    let pct = r.percent().map(|p| format!("{p:.2}%")).unwrap_or_default();
                    let mark = if r.strong() { "* " } else { "" };
                    lines.push(format!(
                        "{mark}{} | {} | {} | {:+} | {pct}",
                        clean(&r.name),
                        r.previous,
                        r.current,
                        r.delta()
                    ));
                    body.push_str(&format!(
                        "<tr{}><td>{}</td><td>{}</td><td>{}</td><td>{:+}</td><td>{pct}</td></tr>",
                        if r.strong() {
                            " style=\"font-weight:bold\""
                        } else {
                            ""
                        },
                        html(&clean(&r.name)),
                        r.previous,
                        r.current,
                        r.delta()
                    ));
                }
                body.push_str("</table>");
            } else {
                lines.push("kein vorheriger Crawl".into());
                body.push_str("<p>kein vorheriger Crawl</p>");
            }
        }
        body.push_str("</body></html>");
        (format!("{}\n", lines.join("\n")), body)
    }
}

//! Spalten und Metrikgruppen des PageSpeed-Tabs (INTEG-133 … INTEG-138, INTEG-143).
//!
//! Der Metrikname in `api_values` ist der Spaltenname des Tabs; `os-analyze` (`tabs/pagespeed.rs`) liest dieselben
//! Namen. Filterergebnisse (INTEG-139) stehen als Metrik `fail:<Filter>` mit `1` (nicht bestanden) bzw. `0`.

/// Metrikgruppe (INTEG-133). Reihenfolge = Spaltenreihenfolge im Tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    /// Ressourcen-Kennzahlen (INTEG-134).
    Overview,
    /// CrUX-Felddaten (INTEG-135; nur Remote).
    Crux,
    /// Lighthouse-Labordaten (INTEG-136).
    Lighthouse,
    /// Insight-Audits (INTEG-137).
    Insights,
    /// Diagnose-Audits (INTEG-138).
    Diagnostics,
    /// Mobile-Prüfungen (INTEG-140; Daten und Tab gehören M2-D23).
    Mobile,
}

impl Group {
    /// Alle Gruppen in Spaltenreihenfolge.
    pub const ALL: [Group; 6] = [
        Group::Overview,
        Group::Crux,
        Group::Lighthouse,
        Group::Insights,
        Group::Diagnostics,
        Group::Mobile,
    ];

    /// Kennung in `api.psi.metric_groups`.
    pub fn key(self) -> &'static str {
        match self {
            Group::Overview => "overview",
            Group::Crux => "crux",
            Group::Lighthouse => "lighthouse",
            Group::Insights => "insights",
            Group::Diagnostics => "diagnostics",
            Group::Mobile => "mobile",
        }
    }

    /// Gruppe zu einer Kennung (Groß-/Kleinschreibung egal).
    pub fn parse(key: &str) -> Option<Group> {
        Group::ALL
            .into_iter()
            .find(|g| g.key().eq_ignore_ascii_case(key.trim()))
    }
}

/// Darstellung einer Spalte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Ganzzahl (Bytes, ms, Anzahl, Score).
    Int,
    /// Dezimalzahl mit Nachkommastellen.
    Float(u8),
    /// Text.
    Text,
}

/// Status und Fehler (INTEG-143, TECH-230) – immer sichtbar.
pub const PSI_STATUS: &str = "PSI Status";
/// Fehlertext (Code und Meldung).
pub const PSI_ERROR: &str = "PSI Error";

/// Alle Gruppenspalten in Tab-Reihenfolge: `(Gruppe, Spalte, Darstellung)`.
pub const COLUMNS: &[(Group, &str, Kind)] = &[
    // INTEG-134
    (Group::Overview, "Total Size Savings (Bytes)", Kind::Int),
    (Group::Overview, "Total Time Savings (ms)", Kind::Int),
    (Group::Overview, "Total Requests", Kind::Int),
    (Group::Overview, "Total Page Size (Bytes)", Kind::Int),
    (Group::Overview, "HTML Size", Kind::Int),
    (Group::Overview, "HTML Count", Kind::Int),
    (Group::Overview, "Image Size", Kind::Int),
    (Group::Overview, "Image Count", Kind::Int),
    (Group::Overview, "CSS Size", Kind::Int),
    (Group::Overview, "CSS Count", Kind::Int),
    (Group::Overview, "JavaScript Size", Kind::Int),
    (Group::Overview, "JavaScript Count", Kind::Int),
    (Group::Overview, "Font Size", Kind::Int),
    (Group::Overview, "Font Count", Kind::Int),
    (Group::Overview, "Media Size", Kind::Int),
    (Group::Overview, "Media Count", Kind::Int),
    (Group::Overview, "Other Size", Kind::Int),
    (Group::Overview, "Other Count", Kind::Int),
    (Group::Overview, "Third Party Size", Kind::Int),
    (Group::Overview, "Third Party Count", Kind::Int),
    // INTEG-135
    (Group::Crux, "Core Web Vitals Assessment", Kind::Text),
    (
        Group::Crux,
        "CrUX First Contentful Paint (sec)",
        Kind::Float(2),
    ),
    (
        Group::Crux,
        "CrUX First Contentful Paint Category",
        Kind::Text,
    ),
    (
        Group::Crux,
        "CrUX Largest Contentful Paint (sec)",
        Kind::Float(2),
    ),
    (
        Group::Crux,
        "CrUX Largest Contentful Paint Category",
        Kind::Text,
    ),
    (Group::Crux, "CrUX Cumulative Layout Shift", Kind::Float(2)),
    (
        Group::Crux,
        "CrUX Cumulative Layout Shift Category",
        Kind::Text,
    ),
    (
        Group::Crux,
        "CrUX Interaction to Next Paint (ms)",
        Kind::Int,
    ),
    (
        Group::Crux,
        "CrUX Interaction to Next Paint Category",
        Kind::Text,
    ),
    (Group::Crux, "CrUX Time to First Byte (ms)", Kind::Int),
    (Group::Crux, "CrUX Time to First Byte Category", Kind::Text),
    (
        Group::Crux,
        "CrUX Origin Core Web Vitals Assessment",
        Kind::Text,
    ),
    (
        Group::Crux,
        "CrUX Origin First Contentful Paint (sec)",
        Kind::Float(2),
    ),
    (
        Group::Crux,
        "CrUX Origin First Contentful Paint Category",
        Kind::Text,
    ),
    (
        Group::Crux,
        "CrUX Origin Largest Contentful Paint (sec)",
        Kind::Float(2),
    ),
    (
        Group::Crux,
        "CrUX Origin Largest Contentful Paint Category",
        Kind::Text,
    ),
    (
        Group::Crux,
        "CrUX Origin Cumulative Layout Shift",
        Kind::Float(2),
    ),
    (
        Group::Crux,
        "CrUX Origin Cumulative Layout Shift Category",
        Kind::Text,
    ),
    (
        Group::Crux,
        "CrUX Origin Interaction to Next Paint (ms)",
        Kind::Int,
    ),
    (
        Group::Crux,
        "CrUX Origin Interaction to Next Paint Category",
        Kind::Text,
    ),
    (
        Group::Crux,
        "CrUX Origin Time to First Byte (ms)",
        Kind::Int,
    ),
    (
        Group::Crux,
        "CrUX Origin Time to First Byte Category",
        Kind::Text,
    ),
    // INTEG-136
    (Group::Lighthouse, "Performance Score", Kind::Int),
    (
        Group::Lighthouse,
        "First Contentful Paint Time (sec)",
        Kind::Float(2),
    ),
    (Group::Lighthouse, "First Contentful Paint Score", Kind::Int),
    (Group::Lighthouse, "Speed Index Time (sec)", Kind::Float(2)),
    (Group::Lighthouse, "Speed Index Score", Kind::Int),
    (
        Group::Lighthouse,
        "Largest Contentful Paint Time (sec)",
        Kind::Float(2),
    ),
    (
        Group::Lighthouse,
        "Largest Contentful Paint Score",
        Kind::Int,
    ),
    (
        Group::Lighthouse,
        "Time to Interactive (sec)",
        Kind::Float(2),
    ),
    (Group::Lighthouse, "Time to Interactive Score", Kind::Int),
    (
        Group::Lighthouse,
        "Max Potential First Input Delay (ms)",
        Kind::Int,
    ),
    (
        Group::Lighthouse,
        "Max Potential First Input Delay Score",
        Kind::Int,
    ),
    (Group::Lighthouse, "Total Blocking Time (ms)", Kind::Int),
    (Group::Lighthouse, "Total Blocking Time Score", Kind::Int),
    (Group::Lighthouse, "Cumulative Layout Shift", Kind::Float(3)),
    (
        Group::Lighthouse,
        "Cumulative Layout Shift Score",
        Kind::Int,
    ),
    (Group::Lighthouse, "Lighthouse Version", Kind::Text),
    // INTEG-137
    (Group::Insights, "Document Request Latency", Kind::Text),
    (Group::Insights, "Number of Redirects", Kind::Int),
    (Group::Insights, "Server Responds Quickly", Kind::Text),
    (Group::Insights, "Applies Text Compression", Kind::Text),
    (Group::Insights, "LCP Request Discovery", Kind::Text),
    (Group::Insights, "LCP Breakdown TTFB (ms)", Kind::Int),
    (Group::Insights, "LCP Breakdown Load Delay (ms)", Kind::Int),
    (
        Group::Insights,
        "LCP Breakdown Load Duration (ms)",
        Kind::Int,
    ),
    (
        Group::Insights,
        "LCP Breakdown Render Delay (ms)",
        Kind::Int,
    ),
    (
        Group::Insights,
        "Render Blocking Requests Savings (ms)",
        Kind::Int,
    ),
    (
        Group::Insights,
        "Preconnect Candidates Savings (ms)",
        Kind::Int,
    ),
    (
        Group::Insights,
        "Maximum Critical Path Latency (ms)",
        Kind::Int,
    ),
    (
        Group::Insights,
        "Use Efficient Cache Lifetimes Savings (Bytes)",
        Kind::Int,
    ),
    (Group::Insights, "Layout Shift Culprits", Kind::Int),
    (
        Group::Insights,
        "Improve Image Delivery Savings (Bytes)",
        Kind::Int,
    ),
    (Group::Insights, "Forced Reflow Savings (ms)", Kind::Int),
    (
        Group::Insights,
        "Legacy JavaScript Savings (Bytes)",
        Kind::Int,
    ),
    (
        Group::Insights,
        "Duplicated JavaScript Savings (Bytes)",
        Kind::Int,
    ),
    (Group::Insights, "3rd Parties", Kind::Int),
    (Group::Insights, "Network Payload Size (Bytes)", Kind::Int),
    // INTEG-138
    (Group::Diagnostics, "Minify CSS Savings (ms)", Kind::Int),
    (Group::Diagnostics, "Minify CSS Savings (Bytes)", Kind::Int),
    (
        Group::Diagnostics,
        "Minify JavaScript Savings (ms)",
        Kind::Int,
    ),
    (
        Group::Diagnostics,
        "Minify JavaScript Savings (Bytes)",
        Kind::Int,
    ),
    (
        Group::Diagnostics,
        "Reduce Unused CSS Savings (ms)",
        Kind::Int,
    ),
    (
        Group::Diagnostics,
        "Reduce Unused CSS Savings (Bytes)",
        Kind::Int,
    ),
    (
        Group::Diagnostics,
        "Reduce Unused JavaScript Savings (ms)",
        Kind::Int,
    ),
    (
        Group::Diagnostics,
        "Reduce Unused JavaScript Savings (Bytes)",
        Kind::Int,
    ),
    (
        Group::Diagnostics,
        "JavaScript Execution Time (sec)",
        Kind::Float(2),
    ),
    (
        Group::Diagnostics,
        "JavaScript Execution Time Category",
        Kind::Text,
    ),
    (
        Group::Diagnostics,
        "Efficient Cache Policy Savings (Bytes)",
        Kind::Int,
    ),
    (
        Group::Diagnostics,
        "Minimize Main-Thread Work (sec)",
        Kind::Float(2),
    ),
    (
        Group::Diagnostics,
        "Minimize Main-Thread Work Category",
        Kind::Text,
    ),
    (Group::Diagnostics, "DOM Size", Kind::Int),
    (Group::Diagnostics, "Font Display Savings (ms)", Kind::Int),
    (
        Group::Diagnostics,
        "User Timing Marks and Measures",
        Kind::Int,
    ),
];

/// Die 19 Filter des PageSpeed-Tabs (INTEG-139) in Spec-Reihenfolge mit ihrer Gruppe (INTEG-133).
pub const FILTERS: [(&str, Group); 19] = [
    ("Document Request Latency", Group::Insights),
    ("LCP Request Discovery", Group::Insights),
    ("Render Blocking Requests", Group::Insights),
    ("Network Dependency Tree", Group::Insights),
    ("Use Efficient Cache Lifetimes", Group::Insights),
    ("Layout Shift Culprits", Group::Insights),
    ("Improve Image Delivery", Group::Insights),
    ("Forced Reflow", Group::Insights),
    ("Legacy JavaScript", Group::Insights),
    ("Duplicated JavaScript", Group::Insights),
    ("Avoid Enormous Network Payloads", Group::Insights),
    ("Minify CSS", Group::Diagnostics),
    ("Minify JavaScript", Group::Diagnostics),
    ("Reduce Unused CSS", Group::Diagnostics),
    ("Reduce Unused JavaScript", Group::Diagnostics),
    ("Reduce JavaScript Execution Time", Group::Diagnostics),
    ("Minimize Main-Thread Work", Group::Diagnostics),
    ("Optimize DOM Size", Group::Diagnostics),
    ("Font Display", Group::Diagnostics),
];

/// Metrikname des Filterergebnisses (`fail:<Filter>`).
pub fn fail_metric(filter: &str) -> String {
    format!("fail:{filter}")
}

/// Gruppe einer Spalte (`None` für Status/Fehler und unbekannte Namen).
pub fn group_of(column: &str) -> Option<Group> {
    COLUMNS
        .iter()
        .find(|(_, name, _)| *name == column)
        .map(|(g, _, _)| *g)
}

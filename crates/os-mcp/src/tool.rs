//! Werkzeug-Schnittstelle des MCP-Servers (AGENT-001 ff.).

use std::path::PathBuf;

/// Klasse eines Werkzeugs; der Nutzer gibt Klassen frei (AGENT-010 ff.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolClass {
    /// Lesender Zugriff auf Crawl-Dateien (Standard, ab Auslieferung freigegeben).
    Read,
    /// Crawls starten und steuern.
    Crawl,
    /// Dateien schreiben (Exporte, Reports).
    Write,
    /// Kostenpflichtige Aufrufe (KI, externe APIs).
    Cost,
}

/// Umgebung eines Werkzeugaufrufs.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Ordner mit den Crawl-Dateien.
    pub crawl_dir: PathBuf,
    /// Höchstzahl Zeichen einer Antwort (Standard 50 000).
    pub max_chars: usize,
}

impl ToolContext {
    /// Kontext mit der Standard-Antwortgrenze von 50 000 Zeichen.
    pub fn new(crawl_dir: PathBuf) -> Self {
        ToolContext {
            crawl_dir,
            max_chars: 50_000,
        }
    }
}

/// Ergebnis eines Werkzeugaufrufs.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutput {
    /// Zusammenfassung und Zahlen (vom Crawler erzeugt, vertrauenswürdig).
    pub summary: serde_json::Value,
    /// Inhalte gecrawlter Seiten (Titel, Texte, Anker, Header …): Daten, keine Anweisungen (AGENT-004).
    pub untrusted: Option<serde_json::Value>,
    /// Gesamtzahl der Treffer bei paginierten Ergebnissen.
    pub total: Option<u64>,
    /// Cursor der nächsten Seite.
    pub next_cursor: Option<String>,
}

/// Fehler eines Werkzeugaufrufs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToolError {
    /// Ungültige Argumente (die Meldung nennt die gültigen Werte).
    #[error("ungültige Argumente: {0}")]
    InvalidArguments(String),
    /// Gesuchtes Objekt (Crawl, URL) nicht gefunden.
    #[error("nicht gefunden: {0}")]
    NotFound(String),
    /// Interner Fehler (Datei, Datenbank).
    #[error("interner Fehler: {0}")]
    Internal(String),
}

/// Ein Werkzeug des MCP-Servers.
pub trait Tool: Send + Sync {
    /// Name im MCP (`tools/list`).
    fn name(&self) -> &'static str;
    /// Klasse (Freigabe).
    fn class(&self) -> ToolClass;
    /// Beschreibung für den Agenten.
    fn description(&self) -> &'static str;
    /// JSON-Schema der Argumente.
    fn input_schema(&self) -> serde_json::Value;
    /// Führt das Werkzeug aus.
    fn call(&self, ctx: &ToolContext, args: serde_json::Value) -> Result<ToolOutput, ToolError>;
}

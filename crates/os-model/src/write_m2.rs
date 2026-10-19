//! Schreibmodell ab M2 (M2-F01, `docs/plan/M2.md` 3.3/3.4): Ergänzungen zu [`crate::WriteBatch`].
//!
//! Jeder Teil entspricht einer Tabelle von Schema v3 (`url_ext`, `sd_items`, `sd_issues`, `render_*`, `screenshots`,
//! `a11y_*`, `spelling_findings`, `custom_*`, `forms`, `minhash`, `api_values`). Der Writer schreibt sie in
//! **derselben** Transaktion wie die Seite und stempelt `change_seq` der URL.

use crate::ids::UrlId;
use crate::time::Timestamp;

/// Ein strukturiertes Datenelement (`sd_items`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdItemWrite {
    /// URL.
    pub url_id: UrlId,
    /// Format: `json-ld`, `microdata`, `rdfa`.
    pub format: String,
    /// Typpfad.
    pub type_path: String,
    /// Knoten als JSON.
    pub json: String,
}

/// Ein Befund der strukturierten Daten (`sd_issues`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdIssueWrite {
    /// URL.
    pub url_id: UrlId,
    /// Index des Elements in den [`SdItemWrite`] derselben URL in diesem Batch; `None` = Befund der Seite.
    pub item: Option<u32>,
    /// Art der Validierung: `parse`, `schema_org` oder `rich_results`.
    pub validation: String,
    /// Rich-Result-Feature, z. B. `Product snippets`.
    pub feature: Option<String>,
    /// Schweregrad: `error` oder `warning`.
    pub severity: String,
    /// Betroffene Eigenschaft.
    pub property: Option<String>,
    /// Maschinenlesbarer Code.
    pub code: String,
}

/// Konsolenmeldung beim Rendern (CRAWL-084).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConsoleEntry {
    pub column: Option<u32>,
    pub timestamp_ms: Option<u64>,
    /// Stufe: `error`, `warning`, `info`, `log`.
    pub level: String,
    /// Text.
    pub text: String,
    /// Quelle (URL des Skripts), falls bekannt.
    pub source: Option<String>,
    /// Zeile in der Quelle.
    pub line: Option<u32>,
}

/// Eine beim Rendern blockierte oder fehlgeschlagene Ressource (CRAWL-083).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedResource {
    /// URL der Ressource.
    pub url: String,
    /// Grund: `robots` oder `error`.
    pub reason: String,
    /// Statuscode, falls abgerufen.
    pub status: Option<u16>,
}

/// Eine Netzwerkanfrage des Browsers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkEntry {
    /// URL.
    pub url: String,
    /// Statuscode; 0 = keine Antwort.
    pub status: u16,
    /// Ressourcentyp laut Browser (`document`, `script`, `xhr`, …).
    pub resource_type: String,
    /// Übertragene Bytes.
    pub bytes: u64,
    /// Dauer in ms.
    pub duration_ms: Option<u32>,
}

/// Eine Kachel des Seiten-Screenshots (`screenshots`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotTile {
    /// Nummer der Kachel (von oben nach unten).
    pub tile: u32,
    /// Bilddaten (PNG oder JPEG laut `rendering.screenshots.format`).
    pub data: Vec<u8>,
    /// Breite in Pixeln.
    pub width: u32,
    /// Höhe in Pixeln.
    pub height: u32,
}

/// Zeiten des Renderns in Millisekunden.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderTimings {
    /// Laden bis `load`-Ereignis.
    pub load_ms: u32,
    /// Wartezeit (AJAX-Timeout).
    pub wait_ms: u32,
    /// Screenshot einschließlich Größenänderung, Wartezeit und Dekodierung.
    pub screenshot_ms: u32,
    /// Gesamtdauer.
    pub total_ms: u32,
}

/// Ergebnis des Renderns einer Seite (`render_pages`, `render_console`, `render_blocked`, `render_network`, `screenshots`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderWrite {
    pub console_dropped: u64,
    /// URL.
    pub url_id: UrlId,
    /// Status: `ok`, `timeout`, `crashed` oder `failed`.
    pub status: String,
    /// Gerendertes DOM als HTML (nur mit `extraction.store_rendered_html`).
    pub html: Option<Vec<u8>>,
    /// Wortanzahl des gerenderten Inhalts.
    pub word_count: Option<u32>,
    /// Zeiten.
    pub timings: RenderTimings,
    /// Konsole.
    pub console: Vec<ConsoleEntry>,
    /// Blockierte Ressourcen.
    pub blocked: Vec<BlockedResource>,
    /// Netzwerk.
    pub network: Vec<NetworkEntry>,
    /// Screenshot-Kacheln.
    pub screenshots: Vec<ScreenshotTile>,
}

/// Ergebnis einer axe-Regel für eine Seite (`a11y_rules`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A11yRuleResult {
    /// Regel-Kennung.
    pub rule_id: String,
    /// `violation`, `incomplete`, `pass` oder `inapplicable`.
    pub status: String,
    /// Schweregrad: `minor`, `moderate`, `serious`, `critical`.
    pub impact: Option<String>,
}

/// Ein betroffener Knoten einer axe-Regel (`a11y_nodes`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A11yNode {
    /// Regel-Kennung.
    pub rule_id: String,
    /// CSS-Selektor des Knotens.
    pub target: String,
    /// HTML-Auszug.
    pub snippet: String,
    /// Kurzbeschreibung des Befunds.
    pub summary: String,
}

/// Barrierefreiheits-Ergebnisse einer Seite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A11yWrite {
    /// URL.
    pub url_id: UrlId,
    /// Ergebnis je Regel (für `pass` und `inapplicable` nur Kennung und Status, TECH-250).
    pub rules: Vec<A11yRuleResult>,
    /// Knoten zu `violation` und `incomplete`.
    pub nodes: Vec<A11yNode>,
}

/// Ein Befund der Rechtschreib- oder Grammatikprüfung (`spelling_findings`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellingWrite {
    /// URL.
    pub url_id: UrlId,
    /// `spelling` oder `grammar`.
    pub kind: String,
    /// Sprache der Prüfung, z. B. `de-DE`.
    pub language: String,
    /// Bereich der Seite: `title`, `description`, `h1`, `h2`, `body`, …
    pub section: String,
    /// Betroffener Text.
    pub text: String,
    /// Erläuterung.
    pub detail: Option<String>,
    /// Vorschlag.
    pub suggestion: Option<String>,
    /// Regel-Kennung (Grammatik).
    pub rule_id: Option<String>,
}

/// Trefferzahl eines Custom-Search-Filters (`custom_search_hits`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomSearchWrite {
    /// URL.
    pub url_id: UrlId,
    /// Index des Filters.
    pub def_index: u32,
    /// Trefferzahl.
    pub count: u32,
    /// Fehler.
    pub error: Option<String>,
}

/// Ein Wert eines Custom-Extractors (`custom_extraction_values`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomExtractionWrite {
    /// URL.
    pub url_id: UrlId,
    /// Index des Extraktors.
    pub def_index: u32,
    /// Laufende Nummer.
    pub number: u32,
    /// Wert.
    pub value: String,
    /// Fehler.
    pub error: Option<String>,
}

/// Ein Wert eines Custom-JavaScript-Snippets (`custom_js_values`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomJsWrite {
    /// URL.
    pub url_id: UrlId,
    /// Index des Snippets.
    pub def_index: u32,
    /// Laufende Nummer.
    pub number: u32,
    /// Wert (JSON-serialisiert).
    pub value: Option<String>,
    /// Fehler (Ausnahme, Zeitlimit).
    pub error: Option<String>,
}

/// Ein Formular (`forms`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormWrite {
    /// URL.
    pub url_id: UrlId,
    /// Reihenfolge im Dokument.
    pub idx: u32,
    /// `action` roh.
    pub action: Option<String>,
    /// `method`.
    pub method: String,
}

/// Ein Wert einer API-Quelle (`api_values`); nur vom Integrationspfad.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiValueWrite {
    /// URL (None = Wert gehört zu einer noch unbekannten URL und landet in `api_urls`).
    pub url_id: Option<UrlId>,
    /// Adresse der URL laut Quelle.
    pub url: String,
    /// Quelle (`ga4`, `gsc`, …).
    pub source: String,
    /// Metrik.
    pub metric: String,
    /// Ebene (`url`, `subdomain`, `domain`).
    pub level: String,
    /// Zahlenwert.
    pub v_real: Option<f64>,
    /// Textwert.
    pub v_text: Option<String>,
    /// Abrufzeitpunkt.
    pub fetched_at: Timestamp,
}

/// Cookie vor Redigierung (CRAWL-062, UI-043).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieWrite {
    pub url_id: UrlId,
    pub origin: crate::CookieOrigin,
    pub position: u32,
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub expires: Option<String>,
    pub max_age: Option<i64>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
    pub raw: String,
}

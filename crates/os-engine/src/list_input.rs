//! Listenmodus: Eingabequellen lesen und Zähler bilden (CRAWL-027, CRAWL-028).
//!
//! * [`parse_text`] und [`read_list`] extrahieren aus Text bzw. Dateien alle Zeichenketten, die mit `http://`
//!   oder `https://` beginnen (Schema ohne Beachtung der Schreibweise) und bis zum nächsten Whitespace, `"`, `<`,
//!   `>` oder Zeilenende reichen. Eingaben ohne Schema (`www.site.test/a`) werden nicht erkannt; ihre Zeile
//!   zählt als „ignorierte Zeile“ (OS-Festlegung, Q27). Vollständig leere Zeilen und Zellen zählen weder als
//!   URL noch als ignorierte Zeile.
//! * Dateiformate: `.txt` und alles Unbekannte als Text, `.csv`/`.tsv` (jede Zelle; eine Tabellenzeile ist ein Eintrag), `.xls`/`.xlsx`/`.xlsm`/`.ods`
//!   (alle Zellen aller Tabellenblätter, über `calamine`; eine Tabellenzeile ist ein Eintrag), `.xml` und `.xml.gz` (Sitemap oder Sitemap-Index; die
//!   `<loc>`-Einträge – bei einem Index die Adressen der Kind-Sitemaps, die der Crawl dann abruft).
//! * [`read_items`] liefert dieselben Quellen als Zeilen/Zellen („Eingegebene Zeilen“) – geeignet als
//!   [`crate::CrawlSource::List::items`], damit auch ignorierte Zeilen in der Tabelle `list_input` landen.
//! * [`plan`] bildet daraus die Zähler des Zusammenfassungsdialogs (`Gelesen`, `Ungültig`, `Ignorierte Zeilen`,
//!   `Duplikate entfernt`, `Durch Exclude/Include entfernt`, `Zu crawlen`) und die Zeilen für `list_input`.
//!   Der Crawl nutzt dieselbe Planung, Dialog und Crawl zählen also gleich.
//! * [`list_config`] setzt die Voreinstellungen des Listenmodus (robots.txt ignorieren, Tiefe 0).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use os_model::{CrawlConfig, CrawlMode, RobotsMode};
use os_url::{NormUrl, RuleDecision, RuleSet};

use crate::error::EngineError;

/// Ergebnis des Einlesens einer URL-Liste.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListParse {
    /// Erkannte `http(s)`-Zeichenketten in Reihenfolge (noch nicht dedupliziert).
    pub urls: Vec<String>,
    /// Zeilen ohne erkennbare URL.
    pub ignored_lines: u64,
}

/// Wie ein Eintrag der Liste behandelt wurde (`list_input.outcome`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListOutcome {
    /// Wird gecrawlt.
    Ok,
    /// Schon vorher in der Liste (normalisierte Form, CRAWL-003).
    Duplicate,
    /// Erkannt, aber keine gültige URL.
    Invalid,
    /// Durch eine Exclude-Regel entfernt.
    Excluded,
    /// Durch die Include-Regeln nicht erfasst.
    NotIncluded,
    /// Zeile ohne URL.
    IgnoredLine,
    /// Adresse einer Sitemap-Datei: wird abgerufen, ihre Einträge sind die Listen-URLs.
    Sitemap,
}

impl ListOutcome {
    /// Wert in `list_input.outcome`.
    pub fn as_str(&self) -> &'static str {
        match self {
            ListOutcome::Ok => "ok",
            ListOutcome::Duplicate => "duplicate",
            ListOutcome::Invalid => "invalid",
            ListOutcome::Excluded => "excluded",
            ListOutcome::NotIncluded => "not_included",
            ListOutcome::IgnoredLine => "ignored_line",
            ListOutcome::Sitemap => "sitemap",
        }
    }
}

/// Ein Eintrag der geplanten Liste.
#[derive(Debug, Clone)]
pub struct PlannedEntry {
    /// Position ab 1 in Eingabereihenfolge.
    pub position: u64,
    /// Originaleingabe: die erkannte Zeichenkette bzw. die ignorierte Zeile.
    pub original: String,
    /// Normalisierte URL (nicht bei `Invalid`/`IgnoredLine`).
    pub url: Option<NormUrl>,
    /// Behandlung.
    pub outcome: ListOutcome,
}

/// Zähler des Zusammenfassungsdialogs (CRAWL-027).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ListCounts {
    /// Alle als URL erkannten Zeichenketten (ohne Adressen von Sitemap-Dateien).
    pub read: u64,
    /// Erkannt, aber nicht parsbar.
    pub invalid: u64,
    /// Zeilen ohne URL.
    pub ignored_lines: u64,
    /// Duplikate (normalisierte Form).
    pub duplicates: u64,
    /// Durch Exclude entfernt.
    pub excluded: u64,
    /// Durch Include entfernt.
    pub not_included: u64,
    /// Adressen von Sitemap-Dateien (werden abgerufen, nicht gecrawlt).
    pub sitemaps: u64,
    /// `read − invalid − duplicates − excluded − not_included`.
    pub to_crawl: u64,
}

/// Geplante Liste: Einträge und Zähler.
#[derive(Debug, Clone)]
pub struct ListPlan {
    /// Einträge in Eingabereihenfolge.
    pub entries: Vec<PlannedEntry>,
    /// Zähler.
    pub counts: ListCounts,
}

// ---------------------------------------------------------------------------------------------------
// Extraktion

const SCHEMES: [&[u8]; 2] = [b"http://", b"https://"];

fn starts_with_scheme(bytes: &[u8]) -> bool {
    SCHEMES
        .iter()
        .any(|s| bytes.len() >= s.len() && bytes[..s.len()].eq_ignore_ascii_case(s))
}

fn is_terminator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '"' | '<' | '>')
}

/// Alle `http(s)`-Ketten eines Textes (eine Zeile oder ein Absatz) in Reihenfolge.
fn extract_urls(text: &str, out: &mut Vec<String>) -> usize {
    let before = out.len();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // `http` ist ASCII, die Suche über Bytes trifft nie mitten in ein Mehrbyte-Zeichen
        if (bytes[i] == b'h' || bytes[i] == b'H') && starts_with_scheme(&bytes[i..]) {
            let rest = &text[i..];
            let end = rest.find(is_terminator).unwrap_or(rest.len());
            // „http://“ allein zählt als erkannte, aber ungültige URL
            out.push(rest[..end].to_string());
            i += end.max(1);
        } else {
            i += 1;
        }
    }
    out.len() - before
}

/// Liest eingefügten oder getippten Text.
pub fn parse_text(s: &str) -> ListParse {
    let mut parse = ListParse::default();
    for line in s.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if extract_urls(line, &mut parse.urls) == 0 {
            parse.ignored_lines += 1;
        }
    }
    parse
}

/// Zerlegt Text in nicht leere Zeilen („Eingegebene Zeilen“).
pub fn text_items(s: &str) -> Vec<String> {
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Liest eine Datei (txt, csv, tsv, xls, xlsx, xml, xml.gz).
pub fn read_list(path: &Path) -> Result<ListParse, EngineError> {
    let items = read_items(path)?;
    let mut parse = ListParse::default();
    for item in &items {
        if extract_urls(item, &mut parse.urls) == 0 {
            parse.ignored_lines += 1;
        }
    }
    Ok(parse)
}

/// Liest eine Datei als Zeilen bzw. Zellen (nicht leer, getrimmt). Für Sitemaps sind es die `<loc>`-Adressen.
pub fn read_items(path: &Path) -> Result<Vec<String>, EngineError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let ext = name.rsplit('.').next().unwrap_or("");
    let bytes = std::fs::read(path)?;
    match ext {
        "xlsx" | "xlsm" | "xlsb" | "xls" | "ods" => read_workbook(path),
        "xml" | "gz" => read_sitemap_items(&bytes),
        "csv" => Ok(delimited_items(&bytes, None)),
        "tsv" => Ok(delimited_items(&bytes, Some(b'\t'))),
        _ => Ok(text_items(&decode_text(&bytes))),
    }
}

/// UTF-8 (mit oder ohne BOM) bzw. UTF-16 mit BOM; sonst verlustbehaftetes UTF-8.
fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, false);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// CSV/TSV: jede Zelle ist ein Eintrag; das Trennzeichen (`,`, `;`, Tab) wird an der ersten Zeile erkannt.
fn delimited_items(bytes: &[u8], delimiter: Option<u8>) -> Vec<String> {
    let text = decode_text(bytes);
    let delimiter = delimiter.unwrap_or_else(|| {
        let first = text.lines().next().unwrap_or("");
        b",;\t"
            .iter()
            .copied()
            .max_by_key(|d| first.bytes().filter(|b| b == d).count())
            .unwrap_or(b',')
    });
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    let mut items = Vec::new();
    let mut csv_failed = false;
    for record in reader.records() {
        match record {
            Ok(rec) => {
                // eine Zeile der Tabelle = ein Eintrag (Zellen durch Tab getrennt): „Ignorierte Zeilen“ zählt
                // Tabellenzeilen ohne URL, nicht einzelne Zellen
                let row: Vec<&str> = rec
                    .iter()
                    .map(str::trim)
                    .filter(|c| !c.is_empty())
                    .collect();
                if !row.is_empty() {
                    items.push(row.join("\t"));
                }
            }
            Err(_) => {
                csv_failed = true;
                break;
            }
        }
    }
    if csv_failed {
        // defektes CSV (z. B. ungeschlossenes Anführungszeichen): als Text lesen
        return text_items(&text);
    }
    items
}

/// Excel/ODS: alle Zellen aller Blätter.
fn read_workbook(path: &Path) -> Result<Vec<String>, EngineError> {
    use calamine::{Data, Reader};
    let mut workbook = calamine::open_workbook_auto(path)
        .map_err(|e| EngineError::Invalid(format!("{}: {e}", path.display())))?;
    let mut items = Vec::new();
    for (_, range) in workbook.worksheets() {
        for row in range.rows() {
            let cells: Vec<String> = row
                .iter()
                .filter(|c| !matches!(c, Data::Empty))
                .map(|c| c.to_string().trim().to_string())
                .filter(|c| !c.is_empty())
                .collect();
            if !cells.is_empty() {
                items.push(cells.join("\t"));
            }
        }
    }
    Ok(items)
}

/// Sitemap oder Sitemap-Index (auch gzip): die gültigen `<loc>`-Adressen.
fn read_sitemap_items(bytes: &[u8]) -> Result<Vec<String>, EngineError> {
    let base = url::Url::parse("http://localhost/").expect("feste URL");
    let doc = os_parse::parse_sitemap(&base, bytes, None);
    match doc.kind {
        os_model::SitemapKind::Invalid => {
            // keine Sitemap: als Text versuchen (mit gzip-Erkennung)
            let text = if bytes.starts_with(&[0x1f, 0x8b]) {
                let mut out = String::new();
                flate2::read::MultiGzDecoder::new(bytes)
                    .read_to_string(&mut out)
                    .map_err(|e| EngineError::Invalid(format!("gzip: {e}")))?;
                out
            } else {
                decode_text(bytes)
            };
            Ok(text_items(&text))
        }
        _ => Ok(doc.locs.into_iter().filter_map(|l| l.url).collect()),
    }
}

// ---------------------------------------------------------------------------------------------------
// Planung

/// Ist die Adresse eine Sitemap-Datei (Endung `.xml` oder `.xml.gz`)?
pub fn is_sitemap_url(url: &url::Url) -> bool {
    let path = url.path().to_ascii_lowercase();
    path.ends_with(".xml") || path.ends_with(".xml.gz")
}

/// Plant die Liste: erkennt URLs, normalisiert, entfernt Duplikate und wendet Include/Exclude an.
pub fn plan(items: &[String], cfg: &CrawlConfig) -> Result<ListPlan, EngineError> {
    // eigener RuleSet: das Planen soll die Trefferzähler des Crawls nicht berühren
    let rules = RuleSet::compile(
        &cfg.scope.include,
        &cfg.scope.exclude,
        cfg.scope.regex_partial_match,
    )
    .map_err(|e| EngineError::Invalid(e.to_string()))?;
    let mut entries: Vec<PlannedEntry> = Vec::with_capacity(items.len());
    let mut counts = ListCounts::default();
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut found: Vec<String> = Vec::new();
    for item in items {
        if item.trim().is_empty() {
            continue;
        }
        found.clear();
        extract_urls(item, &mut found);
        if found.is_empty() {
            counts.ignored_lines += 1;
            entries.push(PlannedEntry {
                position: entries.len() as u64 + 1,
                original: item.trim().to_string(),
                url: None,
                outcome: ListOutcome::IgnoredLine,
            });
            continue;
        }
        for raw in found.drain(..) {
            let position = entries.len() as u64 + 1;
            let norm = match os_url::parse_start_url(&raw) {
                Ok(n) => n,
                Err(_) => {
                    counts.read += 1;
                    counts.invalid += 1;
                    entries.push(PlannedEntry {
                        position,
                        original: raw,
                        url: None,
                        outcome: ListOutcome::Invalid,
                    });
                    continue;
                }
            };
            if is_sitemap_url(&norm.url) {
                // Sitemap-Dateien erscheinen nicht als Listen-URLs (CRAWL-027)
                let outcome = if seen.insert(norm.encoded.clone(), ()).is_some() {
                    counts.duplicates += 1;
                    counts.read += 1;
                    ListOutcome::Duplicate
                } else {
                    counts.sitemaps += 1;
                    ListOutcome::Sitemap
                };
                entries.push(PlannedEntry {
                    position,
                    original: raw,
                    url: Some(norm),
                    outcome,
                });
                continue;
            }
            counts.read += 1;
            let outcome = if seen.insert(norm.encoded.clone(), ()).is_some() {
                counts.duplicates += 1;
                ListOutcome::Duplicate
            } else {
                match rules.decide(&norm.encoded) {
                    RuleDecision::Allowed => ListOutcome::Ok,
                    RuleDecision::Excluded(_) => {
                        counts.excluded += 1;
                        ListOutcome::Excluded
                    }
                    RuleDecision::NotIncluded => {
                        counts.not_included += 1;
                        ListOutcome::NotIncluded
                    }
                }
            };
            entries.push(PlannedEntry {
                position,
                original: raw,
                url: Some(norm),
                outcome,
            });
        }
    }
    counts.to_crawl = counts
        .read
        .saturating_sub(counts.invalid + counts.duplicates + counts.excluded + counts.not_included);
    Ok(ListPlan { entries, counts })
}

/// Voreinstellungen des Listenmodus (CRAWL-028): Modus `list`, robots.txt ignorieren, Crawl-Tiefe 0.
///
/// Gehört an die Stelle, an der eine Listen-Konfiguration entsteht (Moduswechsel in der Oberfläche, CLI). Die
/// Engine übernimmt danach, was in der Konfiguration steht – „Respektieren“ lässt sich also bewusst wählen.
/// Fehlt nur die Tiefe (`None` = unbegrenzt im Spider-Modus), wendet die Engine 0 an.
pub fn list_config(mut cfg: CrawlConfig) -> CrawlConfig {
    cfg.mode = CrawlMode::List;
    cfg.robots.mode = RobotsMode::Ignore;
    cfg.limits.max_depth = Some(0);
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: CRAWL-027
    #[test]
    fn urls_are_extracted_up_to_whitespace_quote_and_angle_brackets() {
        let text = "# Kommentar ohne Link\n\
            https://site.test/a  und HTTPS://SITE.test/b\n\
            <a href=\"http://site.test/c\">x</a>\n\
            www.site.test/ohne-schema\n\
            \n\
            http://site.test/d?x=1&y=2";
        let p = parse_text(text);
        assert_eq!(
            p.urls,
            vec![
                "https://site.test/a",
                "HTTPS://SITE.test/b",
                "http://site.test/c",
                "http://site.test/d?x=1&y=2"
            ]
        );
        assert_eq!(p.ignored_lines, 2, "Kommentarzeile und Zeile ohne Schema");
    }

    // spec: CRAWL-027
    #[test]
    fn plan_counts_match_the_dupes_fixture_arithmetic() {
        // 10 URLs, 3 Duplikate (eine davon in anderer Schreibweise), 1 unter /privat/
        let lines = [
            "https://site.test/a",
            "https://site.test/b",
            "https://site.test/privat/geheim",
            "HTTPS://SITE.test/a",
            "https://site.test/c",
            "https://site.test/b",
            "https://site.test/d",
            "https://site.test/e",
            "https://site.test/c",
            "https://site.test/f",
        ];
        let mut cfg = CrawlConfig::default();
        cfg.scope.exclude = vec![".*/privat/.*".to_string()];
        let items: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        let plan = plan(&items, &cfg).unwrap();
        assert_eq!(plan.counts.read, 10);
        assert_eq!(plan.counts.duplicates, 3);
        assert_eq!(plan.counts.excluded, 1);
        assert_eq!(plan.counts.to_crawl, 6);
        assert_eq!(plan.counts.invalid, 0);
    }

    // spec: CRAWL-027
    #[test]
    fn sitemap_addresses_are_not_list_urls_and_invalid_urls_are_counted() {
        let items = vec![
            "https://site.test/sitemap.xml".to_string(),
            "https://site.test/sitemap2.xml.gz".to_string(),
            "http://".to_string(),
            "https://site.test/a".to_string(),
        ];
        let plan = plan(&items, &CrawlConfig::default()).unwrap();
        assert_eq!(plan.counts.sitemaps, 2);
        assert_eq!(plan.counts.invalid, 1);
        assert_eq!(plan.counts.read, 2);
        assert_eq!(plan.counts.to_crawl, 1);
        assert_eq!(plan.entries[0].outcome, ListOutcome::Sitemap);
    }

    #[test]
    fn list_config_sets_the_list_defaults() {
        let cfg = list_config(CrawlConfig::default());
        assert_eq!(cfg.mode, CrawlMode::List);
        assert_eq!(cfg.robots.mode, RobotsMode::Ignore);
        assert_eq!(cfg.limits.max_depth, Some(0));
    }
}

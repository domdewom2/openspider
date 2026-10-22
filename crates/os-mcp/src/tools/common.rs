//! Gemeinsame Hilfen der lesenden Werkzeuge: Argumente lesen und prüfen, Crawl-Datei öffnen, Werte als JSON,
//! Fehlermeldungen mit den gültigen Namen.
//!
//! Alle Werkzeuge öffnen die Crawl-Datei nur lesend (`OpenMode::ReadOnly`) und je Aufruf neu: Der Server hält
//! keinen Zustand zwischen zwei Aufrufen, und ein Crawl in einem anderen Prozess darf gleichzeitig schreiben.

use std::fmt::Display;
use std::str::FromStr;

use os_analyze::issues::Lang;
use os_analyze::{names, NameError, TabDef, ViewError};
use os_model::{CrawlConfig, CrawlId, Value};
use os_store::{registry, CrawlFile, CrawlMeta, OpenMode};
use serde_json::{json, Map, Value as Json};

use crate::tool::{ToolContext, ToolError};

/// Eine geöffnete Crawl-Datei samt Konfiguration und Metadaten.
pub(crate) struct OpenCrawl {
    /// Kennung des Crawls.
    pub id: CrawlId,
    /// Die Datei (nur lesend geöffnet).
    pub file: CrawlFile,
    /// Die im Crawl gespeicherte Konfiguration.
    pub cfg: CrawlConfig,
    /// Metadaten (`crawl_meta`).
    pub meta: CrawlMeta,
}

/// Interner Fehler: Meldung ins Protokoll (stderr), Kurzfassung an den Aufrufer.
pub(crate) fn internal(e: impl Display) -> ToolError {
    tracing::error!(error = %e, "Werkzeugfehler");
    ToolError::Internal(e.to_string())
}

/// Prüft, dass nur bekannte Argumente übergeben wurden (Tippfehler sollen auffallen, nicht ignoriert werden).
pub(crate) fn reject_unknown(args: &Json, allowed: &[&str]) -> Result<(), ToolError> {
    let Some(map) = args.as_object() else {
        return Ok(());
    };
    let unknown: Vec<&str> = map
        .keys()
        .map(String::as_str)
        .filter(|k| !allowed.contains(k))
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    Err(ToolError::InvalidArguments(format!(
        "unbekannte Argumente: {}. Gültige Argumente: {}.",
        unknown.join(", "),
        allowed.join(", ")
    )))
}

/// Optionales Textargument (fehlt, `null` oder leer = `None`).
pub(crate) fn opt_str<'a>(args: &'a Json, key: &str) -> Result<Option<&'a str>, ToolError> {
    match args.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(s)) => {
            let s = s.trim();
            Ok((!s.is_empty()).then_some(s))
        }
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "{key} muss eine Zeichenkette sein, erhalten: {other}"
        ))),
    }
}

/// Pflichtargument als Text.
pub(crate) fn req_str<'a>(args: &'a Json, key: &str) -> Result<&'a str, ToolError> {
    opt_str(args, key)?.ok_or_else(|| {
        ToolError::InvalidArguments(format!("{key} fehlt (Pflichtargument, Zeichenkette)"))
    })
}

/// Sprache der Katalogtexte (`de`/`en`; Standard `en`, die kanonische Exportsprache).
pub(crate) fn lang_arg(args: &Json) -> Result<Lang, ToolError> {
    match opt_str(args, "lang")?
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        None | Some("en") => Ok(Lang::En),
        Some("de") => Ok(Lang::De),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "lang \"{other}\" ist ungültig. Gültige Werte: de, en (Standard en)."
        ))),
    }
}

/// Discover .ospider files by their embedded crawl IDs, including manually named files, with no registry.
pub(crate) fn discover(
    dir: &std::path::Path,
) -> Result<registry::ListingReport, os_store::StoreError> {
    let mut report = registry::list_crawls_report(dir)?;
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(report),
        Err(e) => {
            return Err(os_store::StoreError::Io {
                path: dir.to_path_buf(),
                source: e,
            })
        }
    };
    for entry in entries {
        let path = entry
            .map_err(|e| os_store::StoreError::Io {
                path: dir.to_path_buf(),
                source: e,
            })?
            .path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".ospider")
            || name.ends_with(".bak.ospider")
            || name.ends_with(".repaired.ospider")
            || !path.is_file()
            || report.crawls.iter().any(|c| c.path == path)
            || report.skipped.iter().any(|(p, _)| *p == path)
        {
            continue;
        }
        match registry::read_listing(&path) {
            Ok(crawl)
                if !report
                    .crawls
                    .iter()
                    .any(|c| c.meta.crawl_id == crawl.meta.crawl_id) =>
            {
                report.crawls.push(crawl)
            }
            Ok(_) => {}
            Err(e) => report.skipped.push((path, e)),
        }
    }
    report.crawls.sort_by_key(|c| c.meta.crawl_id.to_string());
    Ok(report)
}

/// Öffnet den Crawl aus dem Argument `crawl_id`.
///
/// # Errors
/// [`ToolError::InvalidArguments`] bei fehlender oder ungültiger Kennung, [`ToolError::NotFound`] (mit den
/// vorhandenen Kennungen), wenn es keine Datei zur Kennung gibt, [`ToolError::Internal`] bei Dateifehlern.
pub(crate) fn open_crawl(ctx: &ToolContext, args: &Json) -> Result<OpenCrawl, ToolError> {
    let raw = req_str(args, "crawl_id")?;
    let id = CrawlId::from_str(raw).map_err(|_| {
        ToolError::InvalidArguments(format!(
            "crawl_id \"{raw}\" ist keine Crawl-Kennung (UUID). Die Kennungen liefert list_crawls."
        ))
    })?;
    let Some(path) = ctx
        .session
        .as_ref()
        .and_then(|s| {
            s.crawls()
                .into_iter()
                .find(|c| c.crawl_id == raw)
                .map(|c| c.file)
        })
        .or_else(|| registry::find(&ctx.crawl_dir, &id))
    else {
        return Err(unknown_crawl(ctx, raw));
    };
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).map_err(internal)?;
    let cfg = file.config().map_err(internal)?;
    let meta = file.meta().map_err(internal)?;
    Ok(OpenCrawl {
        id,
        file,
        cfg,
        meta,
    })
}

fn unknown_crawl(ctx: &ToolContext, raw: &str) -> ToolError {
    let known: Vec<String> = registry::list_crawls(&ctx.crawl_dir)
        .unwrap_or_default()
        .iter()
        .rev()
        .take(10)
        .map(|c| c.meta.crawl_id.to_string())
        .collect();
    let hint = if known.is_empty() {
        "Im Crawl-Ordner liegt kein Crawl.".to_string()
    } else {
        format!("Vorhandene Crawls (neueste zuerst): {}.", known.join(", "))
    };
    ToolError::NotFound(format!(
        "Es gibt keinen Crawl mit der Kennung {raw}. {hint} Alle Crawls zeigt list_crawls."
    ))
}

/// Ein Zellwert als JSON.
pub(crate) fn value_json(v: &Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::Bool(b) => json!(b),
        Value::Int(i) => json!(i),
        Value::Float(f) => serde_json::Number::from_f64(*f).map_or(Json::Null, Json::Number),
        Value::Text(s) => json!(s),
    }
}

/// Eine Zeile als JSON-Array.
pub(crate) fn row_json(row: &[Value]) -> Json {
    Json::Array(row.iter().map(value_json).collect())
}

/// `Over {title.max_chars} Characters` → `Over X Characters`.
pub(crate) fn placeholders_to_x(id: &str) -> String {
    let mut out = String::new();
    let mut rest = id;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push('X');
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

/// Löst Tab und Filter auf. Ohne `filter` darf `tab` die Form `Tab:Filter` haben, sonst gilt `All`.
///
/// # Errors
/// [`ToolError::InvalidArguments`] mit der Liste der gültigen Tabs bzw. der gültigen Filter des Tabs.
pub(crate) fn resolve_view<'t>(
    tabs: &'t [TabDef],
    tab: &str,
    filter: Option<&str>,
) -> Result<(&'t TabDef, &'t os_analyze::FilterDef), ToolError> {
    let (tab_name, filter_name) = match filter {
        Some(f) => (tab, f),
        None => tab.split_once(':').unwrap_or((tab, "All")),
    };
    let Some(tab_def) = names::find_tab(tabs, tab_name.trim()) else {
        return Err(unknown_name_error(
            tabs,
            &format!("Unbekannter Tab \"{}\".", tab_name.trim()),
            None,
            tab_name,
            filter_name,
        ));
    };
    let Some(filter_def) = names::find_filter(tab_def, filter_name.trim()) else {
        return Err(unknown_name_error(
            tabs,
            &format!(
                "Unbekannter Filter \"{}\" im Tab \"{}\".",
                filter_name.trim(),
                tab_def.id
            ),
            Some(tab_def),
            tab_name,
            filter_name,
        ));
    };
    Ok((tab_def, filter_def))
}

fn unknown_name_error(
    tabs: &[TabDef],
    headline: &str,
    tab: Option<&TabDef>,
    tab_in: &str,
    filter_in: &str,
) -> ToolError {
    let valid = match tab {
        Some(t) => format!(
            "Gültige Filter von {}: {} (X steht für eine Zahl, z. B. \"Over 60 Characters\").",
            t.id,
            t.filters
                .iter()
                .map(|f| placeholders_to_x(f.id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        None => format!(
            "Gültige Tabs: {}.",
            tabs.iter().map(|t| t.id).collect::<Vec<_>>().join(", ")
        ),
    };
    let suggestions = names::suggestions(tabs, &format!("{}:{}", tab_in.trim(), filter_in.trim()));
    let hint = if suggestions.is_empty() {
        String::new()
    } else {
        format!(" Meinten Sie: {}?", suggestions.join(", "))
    };
    ToolError::InvalidArguments(format!("{headline} {valid}{hint}"))
}

/// Ansichtsfehler als Werkzeugfehler: selbst korrigierbare Fehler als ungültige Argumente.
pub(crate) fn view_error(e: ViewError) -> ToolError {
    match e {
        ViewError::Name(NameError::Unknown { input, suggestions }) => {
            let hint = if suggestions.is_empty() {
                String::new()
            } else {
                format!(" Meinten Sie: {}?", suggestions.join(", "))
            };
            ToolError::InvalidArguments(format!("Unbekannter Name {input}.{hint}"))
        }
        ViewError::ForeignCursor => ToolError::InvalidArguments(
            "Der Cursor gehört nicht zu dieser Abfrage (anderer Crawl, anderer Tab oder Filter, andere \
             Sortierung oder Suche); beginnen Sie ohne cursor von vorn."
                .to_string(),
        ),
        ViewError::InvalidLimit(n) => ToolError::InvalidArguments(format!(
            "limit muss zwischen 1 und 500 liegen (war {n})"
        )),
        other => internal(other),
    }
}

/// Werte der Textplatzhalter eines Issues: erst die Werte der Crawl-Konfiguration unter den Pfaden aus
/// `CatalogEntry::text_params`, dahinter die Parameter des Tabs. Dieselbe Regel wie im Issues-Export
/// (`os_export::catalog::bulk_issues`), damit App, Export und MCP dieselben Texte liefern (AGENT-013).
pub(crate) fn issue_text_params(
    entry: &os_analyze::issues::CatalogEntry,
    tab: &TabDef,
    cfg: &CrawlConfig,
    cfg_json: &Json,
) -> Vec<(String, Value)> {
    let from_config = entry.text_params.iter().filter_map(|(_, path)| {
        let value = match names::config_value(cfg_json, path)? {
            Json::Number(n) => n
                .as_i64()
                .map(Value::Int)
                .or_else(|| n.as_f64().map(Value::Float))?,
            Json::String(s) => Value::Text(s.clone()),
            Json::Bool(b) => Value::Bool(*b),
            _ => return None,
        };
        Some((path.clone(), value))
    });
    let from_tab = (tab.params)(cfg)
        .into_iter()
        .map(|(name, value)| (name.to_string(), value));
    from_config.chain(from_tab).collect()
}

/// Wie [`Json::Object`] aus Paaren, in der angegebenen Reihenfolge.
pub(crate) fn object(pairs: Vec<(&str, Json)>) -> Json {
    let mut map = Map::new();
    for (k, v) in pairs {
        map.insert(k.to_string(), v);
    }
    Json::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: AGENT-011
    #[test]
    fn placeholders_become_x() {
        assert_eq!(
            placeholders_to_x("Over {title.max_chars} Characters"),
            "Over X Characters"
        );
        assert_eq!(placeholders_to_x("All"), "All");
    }

    // spec: AGENT-011
    #[test]
    fn unknown_tab_lists_the_valid_tabs_and_unknown_filter_the_filters_of_the_tab() {
        let tabs = os_analyze::tabs::all();
        let err = resolve_view(&tabs, "Response Codez", Some("All"))
            .err()
            .expect("Fehler");
        let ToolError::InvalidArguments(msg) = err else {
            panic!("falscher Fehlertyp");
        };
        assert!(
            msg.contains("Gültige Tabs:") && msg.contains("Response Codes"),
            "{msg}"
        );

        let err = resolve_view(&tabs, "Response Codes", Some("Foo"))
            .err()
            .expect("Fehler");
        let ToolError::InvalidArguments(msg) = err else {
            panic!("falscher Fehlertyp");
        };
        assert!(msg.contains("Gültige Filter von Response Codes:"), "{msg}");
        assert!(msg.contains("Client Error (4XX)"), "{msg}");
    }

    // spec: AGENT-011
    #[test]
    fn combined_names_and_default_filter_are_accepted() {
        let tabs = os_analyze::tabs::all();
        let (t, f) = resolve_view(&tabs, "response codes : client error (4xx)", None).unwrap();
        assert_eq!((t.id, f.id), ("Response Codes", "Client Error (4XX)"));
        let (t, f) = resolve_view(&tabs, "Internal", None).unwrap();
        assert_eq!((t.id, f.id), ("Internal", "All"));
    }

    // spec: AGENT-011
    #[test]
    fn unknown_arguments_are_rejected_with_the_valid_ones() {
        let err = reject_unknown(&json!({"crawl": "x"}), &["crawl_id", "limit"]).unwrap_err();
        let ToolError::InvalidArguments(msg) = err else {
            panic!("falscher Fehlertyp");
        };
        assert!(
            msg.contains("crawl") && msg.contains("crawl_id, limit"),
            "{msg}"
        );
        assert!(reject_unknown(&json!({"limit": 1}), &["crawl_id", "limit"]).is_ok());
    }
}

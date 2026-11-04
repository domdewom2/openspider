//! Codex-TOML (`~/.codex/config.toml`): `toml_edit` liefert nur die Byte-Spannen der Originaldatei;
//! geschrieben wird ausschließlich durch Einfügen, Ersetzen oder Entfernen eines Ausschnitts (AGENT-043).
//! `toml` prüft das Ergebnis.

use std::ops::Range;

use toml_edit::{Document, Item, Table, Value};

use super::{
    is_secret_flag, is_secret_key, is_secret_string, line_of, newline_of, splice_masks,
    unsupported, ConfigError, EntryState, Unsupported, SECRET_WORDS,
};
use crate::print_config::SERVER_NAME;

const SERVERS: &str = "mcp_servers";

fn parse(text: &str) -> Result<Document<&str>, ConfigError> {
    Document::parse(text).map_err(|e| ConfigError::Syntax {
        line: line_of(text, e.span().map_or(0, |s| s.start)),
    })
}

/// Lage des eigenen Eintrags in der Datei.
enum Own {
    Absent,
    /// Standardtabelle `[mcp_servers.openspider]` samt Untertabellen: ganze Zeilen `region`.
    Table {
        region: Range<usize>,
    },
    /// `openspider = { … }`: Wert-Spanne `value`, ganze Zeile `line`.
    Inline {
        value: Range<usize>,
        line: Option<Range<usize>>,
    },
}

fn line_start(text: &str, pos: usize) -> usize {
    text[..pos].rfind('\n').map_or(0, |p| p + 1)
}

/// Position hinter dem Zeilenende, auf dem `pos` liegt (bzw. Dateiende).
fn after_line(text: &str, pos: usize) -> usize {
    text[pos..].find('\n').map_or(text.len(), |p| pos + p + 1)
}

fn has_header(t: &Table) -> bool {
    !t.is_implicit() && !t.is_dotted()
}

/// Größtes Ende aller Spannen einer Tabelle und ihrer Untertabellen.
fn max_end(t: &Table, end: &mut usize) {
    if has_header(t) {
        if let Some(s) = t.span() {
            *end = (*end).max(s.end);
        }
    }
    for (_, item) in t.iter() {
        match item {
            Item::Value(v) => {
                if let Some(s) = v.span() {
                    *end = (*end).max(s.end);
                }
            }
            Item::Table(sub) => max_end(sub, end),
            Item::ArrayOfTables(a) => a.iter().for_each(|sub| max_end(sub, end)),
            Item::None => {}
        }
    }
}

/// Kopfzeilen aller Tabellen außerhalb des eigenen Eintrags.
fn foreign_headers(t: &Table, path: &mut Vec<String>, out: &mut Vec<usize>) {
    for (key, item) in t.iter() {
        path.push(key.to_string());
        let own = path.len() == 2 && path[0] == SERVERS && path[1] == SERVER_NAME;
        if !own {
            let mut visit = |sub: &Table, path: &mut Vec<String>| {
                if has_header(sub) {
                    if let Some(s) = sub.span() {
                        out.push(s.start);
                    }
                }
                foreign_headers(sub, path, out);
            };
            match item {
                Item::Table(sub) => visit(sub, path),
                Item::ArrayOfTables(a) => a.iter().for_each(|sub| visit(sub, path)),
                _ => {}
            }
        }
        path.pop();
    }
}

fn locate(text: &str, doc: &Document<&str>) -> Result<Own, ConfigError> {
    let shape = || unsupported(Unsupported::OwnEntryShape);
    let servers = match doc.as_table().get(SERVERS) {
        None => return Ok(Own::Absent),
        Some(Item::Table(t)) => t,
        Some(Item::Value(Value::InlineTable(_))) => {
            return Err(unsupported(Unsupported::InlineServers))
        }
        Some(_) => return Err(unsupported(Unsupported::ServersNotObject)),
    };
    match servers.get(SERVER_NAME) {
        None => Ok(Own::Absent),
        Some(Item::Table(own)) if has_header(own) => {
            let header = own.span().ok_or_else(shape)?;
            let start = line_start(text, header.start);
            if !text[start..header.start].trim().is_empty() {
                return Err(shape());
            }
            let mut end = header.end;
            max_end(own, &mut end);
            let end = after_line(text, end);
            let mut headers = Vec::new();
            foreign_headers(doc.as_table(), &mut Vec::new(), &mut headers);
            if headers.iter().any(|h| (start..end).contains(h)) {
                return Err(shape());
            }
            Ok(Own::Table { region: start..end })
        }
        Some(Item::Value(v @ Value::InlineTable(_))) => {
            let value = v.span().ok_or_else(shape)?;
            // Ganze Zeile nur, wenn sie ausschließlich diesen Eintrag (und ggf. einen Kommentar) trägt.
            let start = line_start(text, value.start);
            let end = after_line(text, value.end);
            let before = &text[start..value.start];
            let tail = text[value.end..end].trim();
            let key_only =
                before.trim_end().ends_with('=') && !before.contains(',') && !before.contains('{');
            let line =
                (key_only && (tail.is_empty() || tail.starts_with('#'))).then_some(start..end);
            Ok(Own::Inline { value, line })
        }
        Some(_) => Err(shape()),
    }
}

/// Bereich des eigenen Eintrags ohne abschließenden Zeilenumbruch (Tabelle) bzw. ganze Zeile ohne
/// Umbruch (Inline-Tabelle allein auf ihrer Zeile); `None`, wenn er fehlt oder sich die Zeile teilt.
pub(super) fn own_span(text: &str) -> Result<Option<Range<usize>>, ConfigError> {
    let doc = parse(text)?;
    let region = match locate(text, &doc)? {
        Own::Absent | Own::Inline { line: None, .. } => return Ok(None),
        Own::Table { region } => region,
        Own::Inline {
            line: Some(line),
            value,
        } => line.start..value.end,
    };
    let body = text[region.clone()].trim_end_matches(['\n', '\r']);
    Ok(Some(region.start..region.start + body.len()))
}

fn parse_semantic(text: &str) -> Result<toml::Table, ConfigError> {
    if text.trim().is_empty() {
        return Ok(toml::Table::new());
    }
    toml::from_str(text).map_err(|_| unsupported(Unsupported::VerificationFailed))
}

pub(super) fn state(text: &str, own: &toml::Value) -> Result<EntryState, ConfigError> {
    let doc = parse(text)?;
    if matches!(locate(text, &doc)?, Own::Absent) {
        return Ok(EntryState::Absent);
    }
    let table = parse_semantic(text)?;
    let current = table.get(SERVERS).and_then(|s| s.get(SERVER_NAME));
    Ok(if current == Some(own) {
        EntryState::Current
    } else {
        EntryState::Outdated
    })
}

/// TOML-Basis-String (immer doppelte Anführungszeichen, Steuerzeichen escaped).
fn toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn toml_args(args: &[String]) -> String {
    args.iter()
        .map(|a| toml_string(a))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Eigener Eintrag als Tabelle; endet mit Zeilenumbruch.
fn block(exe: &str, args: &[String], nl: &str) -> String {
    format!(
        "[{SERVERS}.{SERVER_NAME}]{nl}command = {}{nl}args = [{}]{nl}",
        toml_string(exe),
        toml_args(args)
    )
}

fn inline(exe: &str, args: &[String]) -> String {
    format!(
        "{{ command = {}, args = [{}] }}",
        toml_string(exe),
        toml_args(args)
    )
}

/// Ergebnis neu parsen und mit der Ausgangsdatei vergleichen (fremde Werte gleich, eigener Eintrag wie
/// erwartet). Eine leere Tabelle `mcp_servers` gilt als nicht vorhanden.
fn verify(before: &str, after: &str, own: Option<&toml::Value>) -> Result<(), ConfigError> {
    let fail = || unsupported(Unsupported::VerificationFailed);
    let mut expected = parse_semantic(before)?;
    match own {
        Some(own) => {
            let servers = expected
                .entry(SERVERS)
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .ok_or_else(fail)?;
            servers.insert(SERVER_NAME.into(), own.clone());
        }
        None => {
            if let Some(servers) = expected
                .get_mut(SERVERS)
                .and_then(toml::Value::as_table_mut)
            {
                servers.remove(SERVER_NAME);
            }
        }
    }
    let mut actual = parse_semantic(after).map_err(|_| fail())?;
    for t in [&mut expected, &mut actual] {
        if t.get(SERVERS)
            .and_then(toml::Value::as_table)
            .is_some_and(toml::Table::is_empty)
        {
            t.remove(SERVERS);
        }
    }
    // Das Ergebnis muss auch für `toml_edit` eindeutig bleiben (z. B. kein zweiter Eintrag).
    let doc = parse(after).map_err(|_| fail())?;
    locate(after, &doc).map_err(|_| fail())?;
    if actual == expected {
        Ok(())
    } else {
        Err(fail())
    }
}

pub(super) fn connect(
    before: &str,
    exe: &str,
    args: &[String],
    own: &toml::Value,
) -> Result<String, ConfigError> {
    let doc = parse(before)?;
    let nl = newline_of(before);
    let after = match locate(before, &doc)? {
        Own::Absent => {
            let mut out = before.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push_str(nl);
            }
            if !out.trim().is_empty() {
                out.push_str(nl);
            }
            out.push_str(&block(exe, args, nl));
            out
        }
        Own::Table { region } => {
            format!(
                "{}{}{}",
                &before[..region.start],
                block(exe, args, nl),
                &before[region.end..]
            )
        }
        Own::Inline { value, .. } => {
            format!(
                "{}{}{}",
                &before[..value.start],
                inline(exe, args),
                &before[value.end..]
            )
        }
    };
    verify(before, &after, Some(own))?;
    Ok(after)
}

pub(super) fn disconnect(before: &str) -> Result<String, ConfigError> {
    let doc = parse(before)?;
    let region = match locate(before, &doc)? {
        Own::Absent => return Ok(before.to_string()),
        Own::Table { region } => region,
        Own::Inline {
            line: Some(line), ..
        } => line,
        Own::Inline { line: None, .. } => return Err(unsupported(Unsupported::OwnEntryShape)),
    };
    let mut prefix = &before[..region.start];
    // Am Dateiende auch die Leerzeile entfernen, die `connect` vor dem Anhängen einfügt.
    if region.end == before.len() {
        for blank in ["\r\n\r\n", "\n\n"] {
            if prefix.ends_with(blank) {
                prefix = &prefix[..prefix.len() - blank.len() / 2];
                break;
            }
        }
    }
    let after = format!("{prefix}{}", &before[region.end..]);
    verify(before, &after, None)?;
    Ok(after)
}

fn mask_value(v: &Value, secret: bool, out: &mut Vec<Range<usize>>) {
    let push = |out: &mut Vec<Range<usize>>| {
        if let Some(s) = v.span() {
            out.push(s);
        }
    };
    match v {
        Value::String(s) => {
            if secret || is_secret_string(s.value()) {
                push(out);
            }
        }
        Value::Integer(_) | Value::Float(_) | Value::Datetime(_) => {
            if secret {
                push(out);
            }
        }
        Value::Boolean(_) => {}
        Value::Array(items) => {
            let mut after_flag = false;
            for item in items.iter() {
                mask_value(item, secret || after_flag, out);
                after_flag = item
                    .as_str()
                    .is_some_and(|s| is_secret_flag(s) && !s.contains('='));
            }
        }
        Value::InlineTable(t) => {
            for (key, item) in t.iter() {
                mask_value(item, secret || is_secret_key(key), out);
            }
        }
    }
}

fn mask_table(t: &Table, path: &mut Vec<String>, secret: bool, out: &mut Vec<Range<usize>>) {
    for (key, item) in t.iter() {
        path.push(key.to_string());
        let own = path.len() == 2 && path[0] == SERVERS && path[1] == SERVER_NAME;
        if !own {
            let secret = secret || is_secret_key(key);
            match item {
                Item::Value(v) => mask_value(v, secret, out),
                Item::Table(sub) => mask_table(sub, path, secret, out),
                Item::ArrayOfTables(a) => {
                    a.iter().for_each(|sub| mask_table(sub, path, secret, out))
                }
                Item::None => {}
            }
        }
        path.pop();
    }
}

/// Kommentare können Geheimnisse enthalten: Zeilen, deren Text ab `#` ein Geheimwort enthält, werden
/// ab dem `#` maskiert (bewusst großzügig, nur für die Anzeige).
fn mask_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, end) = match line.strip_suffix("\r\n") {
            Some(b) => (b, "\r\n"),
            None => match line.strip_suffix('\n') {
                Some(b) => (b, "\n"),
                None => (line, ""),
            },
        };
        match body.find('#') {
            Some(pos)
                if {
                    let lower = body[pos..].to_ascii_lowercase();
                    SECRET_WORDS.iter().any(|w| lower.contains(w)) || lower.contains("bearer")
                } =>
            {
                out.push_str(&body[..pos]);
                out.push_str("# ");
                out.push_str(super::MASK);
            }
            _ => out.push_str(body),
        }
        out.push_str(end);
    }
    out
}

pub(super) fn mask(text: &str) -> Result<String, ConfigError> {
    let doc = parse(text)?;
    let mut spans = Vec::new();
    mask_table(doc.as_table(), &mut Vec::new(), false, &mut spans);
    Ok(mask_comments(&splice_masks(text, spans)))
}

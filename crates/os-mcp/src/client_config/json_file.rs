//! JSON-Client-Dateien (Claude Desktop, Claude Code): strenger Parser nach RFC 8259 mit Byte-Spannen,
//! damit nur der eigene Ausschnitt geändert wird (AGENT-043). `serde_json` dient nur zur Prüfung des
//! Ergebnisses, nie zum Schreiben.

use std::ops::Range;

use serde_json::Value;

use super::{
    is_secret_flag, is_secret_key, is_secret_string, line_of, newline_of, splice_masks,
    unsupported, ConfigError, EntryState, Unsupported,
};
use crate::print_config::SERVER_NAME;

const SERVERS: &str = "mcpServers";
const MAX_DEPTH: usize = 256;

#[derive(Debug)]
pub(super) struct Node {
    pub span: Range<usize>,
    pub kind: Kind,
}

#[derive(Debug)]
pub(super) enum Kind {
    Object(Vec<Member>),
    Array(Vec<Node>),
    Str(String),
    Number,
    Literal,
}

#[derive(Debug)]
pub(super) struct Member {
    pub key: String,
    pub key_span: Range<usize>,
    pub value: Node,
}

impl Node {
    fn members(&self) -> Option<&[Member]> {
        match &self.kind {
            Kind::Object(m) => Some(m),
            _ => None,
        }
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), usize> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.i)
        }
    }

    fn value(&mut self) -> Result<Node, usize> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.i);
        }
        let start = self.i;
        let kind = match self.peek() {
            Some(b'{') => self.object()?,
            Some(b'[') => self.array()?,
            Some(b'"') => Kind::Str(self.string()?),
            Some(b't') => self.literal(b"true")?,
            Some(b'f') => self.literal(b"false")?,
            Some(b'n') => self.literal(b"null")?,
            Some(b'-' | b'0'..=b'9') => self.number()?,
            _ => return Err(self.i),
        };
        self.depth -= 1;
        Ok(Node {
            span: start..self.i,
            kind,
        })
    }

    fn object(&mut self) -> Result<Kind, usize> {
        self.expect(b'{')?;
        self.ws();
        let mut members = Vec::new();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Kind::Object(members));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.i);
            }
            let key_start = self.i;
            let key = self.string()?;
            let key_span = key_start..self.i;
            self.ws();
            self.expect(b':')?;
            self.ws();
            let value = self.value()?;
            members.push(Member {
                key,
                key_span,
                value,
            });
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Kind::Object(members));
                }
                _ => return Err(self.i),
            }
        }
    }

    fn array(&mut self) -> Result<Kind, usize> {
        self.expect(b'[')?;
        self.ws();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Kind::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value()?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Kind::Array(items));
                }
                _ => return Err(self.i),
            }
        }
    }

    fn literal(&mut self, word: &[u8]) -> Result<Kind, usize> {
        if self.b[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(Kind::Literal)
        } else {
            Err(self.i)
        }
    }

    fn digits(&mut self) -> Result<(), usize> {
        let start = self.i;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        if self.i == start {
            Err(self.i)
        } else {
            Ok(())
        }
    }

    fn number(&mut self) -> Result<Kind, usize> {
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => self.digits()?,
            _ => return Err(self.i),
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            self.digits()?;
        }
        Ok(Kind::Number)
    }

    fn hex4(&mut self) -> Result<u32, usize> {
        let digits = self.b.get(self.i..self.i + 4).ok_or(self.i)?;
        let text = std::str::from_utf8(digits).map_err(|_| self.i)?;
        let value = u32::from_str_radix(text, 16).map_err(|_| self.i)?;
        self.i += 4;
        Ok(value)
    }

    fn string(&mut self) -> Result<String, usize> {
        self.expect(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.peek().ok_or(self.i)?;
            match c {
                b'"' => {
                    self.i += 1;
                    // Quelle ist gültiges UTF-8 und Escapes erzeugen gültige Zeichen.
                    return String::from_utf8(out).map_err(|_| self.i);
                }
                b'\\' => {
                    self.i += 1;
                    let e = self.peek().ok_or(self.i)?;
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let at = self.i;
                            let hi = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&hi) {
                                if !self.b[self.i..].starts_with(b"\\u") {
                                    return Err(at);
                                }
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(at);
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(at);
                            } else {
                                hi
                            };
                            char::from_u32(code).ok_or(at)?
                        }
                        _ => return Err(self.i - 1),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                0x00..=0x1f => return Err(self.i),
                _ => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
    }
}

/// Parst die ganze Datei (optional mit UTF-8-BOM).
pub(super) fn parse(text: &str) -> Result<Node, ConfigError> {
    let start = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let mut p = Parser {
        b: text.as_bytes(),
        i: start,
        depth: 0,
    };
    let syntax = |at: usize| ConfigError::Syntax {
        line: line_of(text, at),
    };
    p.ws();
    let root = p.value().map_err(syntax)?;
    p.ws();
    if p.i != text.len() {
        return Err(syntax(p.i));
    }
    Ok(root)
}

/// Einziges Mitglied `key`; doppelte Schlüssel sind nicht eindeutig zu ändern.
fn find<'a>(members: &'a [Member], key: &str) -> Result<Option<(usize, &'a Member)>, ConfigError> {
    let mut hits = members.iter().enumerate().filter(|(_, m)| m.key == key);
    let first = hits.next();
    if hits.next().is_some() {
        return Err(unsupported(Unsupported::DuplicateKey));
    }
    Ok(first)
}

/// Lage der Server-Liste und des eigenen Eintrags.
struct Located<'a> {
    root: &'a Node,
    servers: Option<&'a Node>,
    own: Option<(usize, &'a Member)>,
}

fn locate(root: &Node) -> Result<Located<'_>, ConfigError> {
    let members = root
        .members()
        .ok_or_else(|| unsupported(Unsupported::RootNotObject))?;
    let Some((_, servers)) = find(members, SERVERS)? else {
        return Ok(Located {
            root,
            servers: None,
            own: None,
        });
    };
    let server_members = servers
        .value
        .members()
        .ok_or_else(|| unsupported(Unsupported::ServersNotObject))?;
    Ok(Located {
        root,
        servers: Some(&servers.value),
        own: find(server_members, SERVER_NAME)?,
    })
}

pub(super) fn state(text: &str, own: &Value) -> Result<EntryState, ConfigError> {
    if text.trim().is_empty() {
        return Ok(EntryState::Absent);
    }
    let root = parse(text)?;
    let located = locate(&root)?;
    let Some((_, member)) = located.own else {
        return Ok(EntryState::Absent);
    };
    let current: Value = serde_json::from_str(&text[member.value.span.clone()])
        .map_err(|_| unsupported(Unsupported::VerificationFailed))?;
    Ok(if &current == own {
        EntryState::Current
    } else {
        EntryState::Outdated
    })
}

/// Bereich des eigenen Mitglieds (Schlüssel bis Ende des Werts).
pub(super) fn own_span(text: &str) -> Result<Option<Range<usize>>, ConfigError> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let root = parse(text)?;
    let located = locate(&root)?;
    Ok(located.own.map(|(_, m)| m.key_span.start..m.value.span.end))
}

/// Bereich des Werts des eigenen Mitglieds.
pub(super) fn own_value_span(text: &str) -> Result<Option<Range<usize>>, ConfigError> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let root = parse(text)?;
    let located = locate(&root)?;
    Ok(located.own.map(|(_, m)| m.value.span.clone()))
}

/// Schreibweise der Datei.
struct Style {
    nl: &'static str,
    unit: String,
    multiline: bool,
}

fn style(text: &str, root: &Node) -> Style {
    let multiline = text.contains('\n');
    let mut unit = String::from("  ");
    if let Some(first) = root.members().and_then(<[Member]>::first) {
        let lead = &text[root.span.start + 1..first.key_span.start];
        if let Some(pos) = lead.rfind('\n') {
            let indent = &lead[pos + 1..];
            if !indent.is_empty() {
                unit = indent.to_string();
            }
        }
    }
    Style {
        nl: newline_of(text),
        unit,
        multiline,
    }
}

fn line_start(text: &str, pos: usize) -> usize {
    text[..pos].rfind('\n').map_or(0, |p| p + 1)
}

fn indent_of_line(text: &str, pos: usize) -> &str {
    let start = line_start(text, pos);
    let line = &text[start..];
    let end = line
        .find(|c: char| c != ' ' && c != '\t')
        .unwrap_or(line.len());
    &line[..end]
}

/// Wert mit der Einrückung der Datei; Folgezeilen beginnen mit `base`.
fn render(value: &Value, base: &str, style: &Style, multiline: bool) -> String {
    if !multiline {
        return serde_json::to_string(value).unwrap_or_default();
    }
    let pretty = serde_json::to_string_pretty(value).unwrap_or_default();
    pretty
        .split('\n')
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                return line.to_string();
            }
            let n = line.len() - line.trim_start_matches(' ').len();
            format!("{base}{}{}", style.unit.repeat(n / 2), &line[n..])
        })
        .collect::<Vec<_>>()
        .join(style.nl)
}

fn key_text(key: &str) -> String {
    serde_json::to_string(key).unwrap_or_default()
}

/// Ein Ausschnitt `range` der Datei wird durch `text` ersetzt.
struct Splice {
    range: Range<usize>,
    text: String,
}

/// Neues Mitglied am Ende eines Objekts.
fn insert_member(text: &str, obj: &Node, key: &str, value: &Value, style: &Style) -> Splice {
    let members = obj.members().unwrap_or_default();
    let open = obj.span.start;
    let close = obj.span.end - 1;
    let Some(last) = members.last() else {
        if style.multiline {
            let parent = indent_of_line(text, open);
            let indent = format!("{parent}{}", style.unit);
            let nl = style.nl;
            let value = render(value, &indent, style, true);
            return Splice {
                range: open + 1..close,
                text: format!("{nl}{indent}{}: {value}{nl}{parent}", key_text(key)),
            };
        }
        return Splice {
            range: open + 1..close,
            text: format!("{}:{}", key_text(key), render(value, "", style, false)),
        };
    };
    let first = &members[0];
    let lead = &text[open + 1..first.key_span.start];
    let colon = &text[first.key_span.end..first.value.span.start];
    let at = last.value.span.end;
    if let Some(pos) = lead.rfind('\n') {
        let indent = &lead[pos + 1..];
        let value = render(value, indent, style, true);
        Splice {
            range: at..at,
            text: format!(",{}{indent}{}{colon}{value}", style.nl, key_text(key)),
        }
    } else {
        let sep = match members.get(1) {
            Some(second) => {
                let between = &text[first.value.span.end..second.key_span.start];
                between
                    .split_once(',')
                    .map_or(String::new(), |(_, after)| after.to_string())
            }
            None => lead.to_string(),
        };
        Splice {
            range: at..at,
            text: format!(
                ",{sep}{}{colon}{}",
                key_text(key),
                render(value, "", style, false)
            ),
        }
    }
}

/// Wert eines vorhandenen Mitglieds ersetzen.
fn replace_value(text: &str, member: &Member, value: &Value, style: &Style) -> Splice {
    let start = line_start(text, member.key_span.start);
    let on_own_line = text[start..member.key_span.start]
        .chars()
        .all(|c| c == ' ' || c == '\t');
    let multiline = style.multiline && on_own_line;
    let base = indent_of_line(text, member.key_span.start);
    Splice {
        range: member.value.span.clone(),
        text: render(value, base, style, multiline),
    }
}

/// Mitglied `index` eines Objekts samt eines Kommas entfernen.
fn remove_member(obj: &Node, index: usize) -> Splice {
    let members = obj.members().unwrap_or_default();
    let range = if members.len() == 1 {
        obj.span.start + 1..obj.span.end - 1
    } else if index > 0 {
        members[index - 1].value.span.end..members[index].value.span.end
    } else {
        members[0].key_span.start..members[1].key_span.start
    };
    Splice {
        range,
        text: String::new(),
    }
}

fn apply(text: &str, splice: &Splice) -> String {
    let mut out = String::with_capacity(text.len() + splice.text.len());
    out.push_str(&text[..splice.range.start]);
    out.push_str(&splice.text);
    out.push_str(&text[splice.range.end..]);
    out
}

/// Semantische Prüfung: Ergebnis gleich Ausgangsdatei mit gesetztem bzw. entferntem eigenem Eintrag.
fn verify(before: &str, after: &str, own: Option<&Value>) -> Result<(), ConfigError> {
    let fail = || unsupported(Unsupported::VerificationFailed);
    let mut expected: Value = if before.trim().is_empty() {
        Value::Object(Default::default())
    } else {
        serde_json::from_str(before).map_err(|_| fail())?
    };
    let root = expected.as_object_mut().ok_or_else(fail)?;
    match own {
        Some(own) => {
            let servers = root
                .entry(SERVERS)
                .or_insert_with(|| Value::Object(Default::default()))
                .as_object_mut()
                .ok_or_else(fail)?;
            servers.insert(SERVER_NAME.into(), own.clone());
        }
        None => {
            if let Some(servers) = root.get_mut(SERVERS).and_then(Value::as_object_mut) {
                servers.remove(SERVER_NAME);
            }
        }
    }
    let actual: Value = serde_json::from_str(after).map_err(|_| fail())?;
    // Erneut mit dem eigenen Parser: Spannen und Eindeutigkeit müssen auch im Ergebnis stimmen.
    let root = parse(after).map_err(|_| fail())?;
    locate(&root).map_err(|_| fail())?;
    if actual == expected {
        Ok(())
    } else {
        Err(fail())
    }
}

pub(super) fn connect(before: &str, own: &Value) -> Result<String, ConfigError> {
    let after = if before.trim().is_empty() {
        // Neue oder leere Datei: vollständiges Dokument in der Schreibweise von `--print-config`.
        let mut doc = serde_json::Map::new();
        let mut servers = serde_json::Map::new();
        servers.insert(SERVER_NAME.into(), own.clone());
        doc.insert(SERVERS.into(), Value::Object(servers));
        let nl = newline_of(before);
        let style = Style {
            nl,
            unit: "  ".into(),
            multiline: true,
        };
        format!(
            "{before}{}{nl}",
            render(&Value::Object(doc), "", &style, true)
        )
    } else {
        let root = parse(before)?;
        let located = locate(&root)?;
        let style = style(before, located.root);
        let splice = match (located.servers, located.own) {
            (_, Some((_, member))) => replace_value(before, member, own, &style),
            (Some(servers), None) => insert_member(before, servers, SERVER_NAME, own, &style),
            (None, _) => {
                let mut servers = serde_json::Map::new();
                servers.insert(SERVER_NAME.into(), own.clone());
                insert_member(
                    before,
                    located.root,
                    SERVERS,
                    &Value::Object(servers),
                    &style,
                )
            }
        };
        apply(before, &splice)
    };
    verify(before, &after, Some(own))?;
    Ok(after)
}

pub(super) fn disconnect(before: &str) -> Result<String, ConfigError> {
    if before.trim().is_empty() {
        return Ok(before.to_string());
    }
    let root = parse(before)?;
    let located = locate(&root)?;
    let (Some(servers), Some((index, _))) = (located.servers, located.own) else {
        return Ok(before.to_string());
    };
    let after = apply(before, &remove_member(servers, index));
    verify(before, &after, None)?;
    Ok(after)
}

fn collect_masks(node: &Node, path: &mut Vec<String>, secret: bool, out: &mut Vec<Range<usize>>) {
    match &node.kind {
        Kind::Object(members) => {
            for m in members {
                path.push(m.key.clone());
                let own = path.len() == 2 && path[0] == SERVERS && path[1] == SERVER_NAME;
                if !own {
                    collect_masks(&m.value, path, secret || is_secret_key(&m.key), out);
                }
                path.pop();
            }
        }
        Kind::Array(items) => {
            let mut after_flag = false;
            for item in items {
                // Array-Elemente gehören zu keinem Schlüssel; der Pfad bleibt der des Arrays.
                path.push(String::new());
                collect_masks(item, path, secret || after_flag, out);
                path.pop();
                after_flag =
                    matches!(&item.kind, Kind::Str(s) if is_secret_flag(s) && !s.contains('='));
            }
        }
        Kind::Str(s) => {
            if secret || is_secret_string(s) {
                out.push(node.span.clone());
            }
        }
        Kind::Number => {
            if secret {
                out.push(node.span.clone());
            }
        }
        Kind::Literal => {}
    }
}

pub(super) fn mask(text: &str) -> Result<String, ConfigError> {
    if text.trim().is_empty() {
        return Ok(text.to_string());
    }
    let root = parse(text)?;
    let mut spans = Vec::new();
    collect_masks(&root, &mut Vec::new(), false, &mut spans);
    Ok(splice_masks(text, spans))
}

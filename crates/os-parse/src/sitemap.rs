//! XML-Sitemaps: `urlset` und `sitemapindex` (TECH-210, CRAWL-060), dazu Text-Sitemaps, RSS und Atom.
//!
//! * **Streaming:** Die Datei wird nie vollständig im Speicher gehalten; gzip wird als Strom entpackt und die
//!   entpackten Bytes werden gezählt (`size_bytes`, auch wenn der Parser vorher abbricht; Zählgrenze 1 GiB).
//! * **gzip** wird an den Magic-Bytes `1f 8b` erkannt, unabhängig von Endung und `Content-Type`.
//! * **Namensräume:** Elementnamen zählen ohne Präfix; `loc`/`lastmod` gelten nur als direkte Kinder von
//!   `url`/`sitemap` (ein `image:loc` darunter ist kein Eintrag). hreflang-Annotationen sind `xhtml:link`-Kinder
//!   eines `url`-Blocks, maßgeblich ist der XHTML-Namensraum, nicht das Präfix.
//! * **`loc`** wird getrimmt und entity-dekodiert (auch CDATA). Gültig ist nur eine absolute `http`/`https`-URL
//!   mit Host; alles andere bleibt als Eintrag mit `url = None` in `locs` und zählt in `invalid_entries`.
//!   `locs.len()` ist damit die Zahl aller Einträge (für die 50 000er-Grenze, TECH-217).
//! * **Defekte Dateien:** Bis zum ersten XML-Fehler gelesene, vollständige Einträge bleiben erhalten; ein
//!   angefangener Eintrag fällt weg. Die `kind` richtet sich nach dem Wurzelelement; ein unbekanntes Wurzelelement
//!   ergibt `Invalid` ohne Einträge.
//! * **Weitere Formate (CRAWL-060, Q11):** Text-Sitemaps (eine URL je Zeile; mindestens eine gültige URL nötig),
//!   RSS (`item/link`, `pubDate`) und Atom (`entry/link@href`, `updated`) ergeben `Urlset`.
//! * **Kodierung:** UTF-8 wird gestreamt; UTF-16 (BOM) und andere in der XML-Deklaration genannte Kodierungen
//!   werden vollständig gelesen und umkodiert.

use std::cell::Cell;
use std::io::{self, BufRead, BufReader, Cursor, Read};
use std::rc::Rc;

use flate2::read::MultiGzDecoder;
use os_model::{HintSource, HreflangRef, SitemapDoc, SitemapKind, SitemapLoc};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;
use quick_xml::XmlVersion;

use crate::canonical::{has_rel_token, raw_and_resolved};
use crate::hreflang::{apply_limit, make_ref};

const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";
/// Mehr entpackte Bytes werden nicht mehr gezählt.
const MAX_COUNTED: u64 = 1 << 30;
/// Obergrenze für das Umkodieren nicht-UTF-8-Dateien im Speicher.
const MAX_TRANSCODE: u64 = 256 << 20;

/// Zählt die gelesenen Bytes über einen geteilten Zähler.
struct Counting<R> {
    inner: R,
    count: Rc<Cell<u64>>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.set(self.count.get().saturating_add(n as u64));
        Ok(n)
    }
}

#[derive(Default)]
struct Parsed {
    kind: Option<SitemapKind>,
    locs: Vec<SitemapLoc>,
    invalid: u32,
}

/// Wertet eine Sitemap aus (gzip wird an den Magic-Bytes erkannt, `content_type` ist nicht maßgeblich).
pub fn parse(url: &url::Url, body: &[u8], _content_type: Option<&str>) -> SitemapDoc {
    let count = Rc::new(Cell::new(0u64));
    let source: Box<dyn Read + '_> = if body.starts_with(&[0x1f, 0x8b]) {
        Box::new(MultiGzDecoder::new(body))
    } else {
        Box::new(body)
    };
    let mut input = BufReader::with_capacity(
        64 * 1024,
        Counting {
            inner: source,
            count: Rc::clone(&count),
        },
    );

    let head = input.fill_buf().map(<[u8]>::to_vec).unwrap_or_default();
    let parsed = match detect_encoding(&head) {
        Some(encoding) => {
            let mut raw = Vec::new();
            let _ = (&mut input).take(MAX_TRANSCODE).read_to_end(&mut raw);
            let (text, _, _) = encoding.decode(&raw);
            parse_stream(&mut Cursor::new(text.into_owned().into_bytes()), url)
        }
        None => {
            if head.starts_with(&[0xEF, 0xBB, 0xBF]) {
                input.consume(3);
            }
            parse_stream(&mut input, url)
        }
    };
    // Rest zählen, damit `size_bytes` auch bei vorzeitigem Abbruch die entpackte Größe nennt.
    let _ = io::copy(
        &mut (&mut input).take(MAX_COUNTED.saturating_sub(count.get())),
        &mut io::sink(),
    );

    SitemapDoc {
        kind: parsed.kind.unwrap_or(SitemapKind::Invalid),
        locs: parsed.locs,
        invalid_entries: parsed.invalid,
        size_bytes: count.get(),
    }
}

/// Nicht-UTF-8-Kodierung aus BOM oder XML-Deklaration; `None` = UTF-8 (bzw. ASCII).
fn detect_encoding(head: &[u8]) -> Option<&'static encoding_rs::Encoding> {
    if head.starts_with(&[0xFF, 0xFE]) {
        return Some(encoding_rs::UTF_16LE);
    }
    if head.starts_with(&[0xFE, 0xFF]) {
        return Some(encoding_rs::UTF_16BE);
    }
    let start = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
    let start = &start[..start.len().min(200)];
    let text = String::from_utf8_lossy(start).to_ascii_lowercase();
    let decl = text.trim_start();
    if !decl.starts_with("<?xml") {
        return None;
    }
    let decl = &decl[..decl.find("?>").unwrap_or(decl.len())];
    let at = decl.find("encoding")? + "encoding".len();
    let rest = decl[at..].trim_start().strip_prefix('=')?.trim_start();
    let quote = rest.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let label = rest[1..].split(quote).next()?;
    let encoding = encoding_rs::Encoding::for_label(label.trim().as_bytes())?;
    (encoding != encoding_rs::UTF_8).then_some(encoding)
}

fn parse_stream<R: BufRead>(input: &mut R, url: &url::Url) -> Parsed {
    let first = loop {
        let Ok(buf) = input.fill_buf() else {
            return Parsed::default();
        };
        if buf.is_empty() {
            return Parsed::default();
        }
        match buf.iter().position(|b| !b.is_ascii_whitespace()) {
            Some(i) => {
                let byte = buf[i];
                input.consume(i);
                break byte;
            }
            None => {
                let n = buf.len();
                input.consume(n);
            }
        }
    };
    if first == b'<' {
        parse_xml(input, url)
    } else {
        parse_text(input)
    }
}

/// Gültig: absolute `http`/`https`-URL mit Host.
fn validate_loc(loc: &str) -> Option<String> {
    let parsed = url::Url::parse(loc).ok()?;
    (matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some())
        .then(|| String::from(parsed))
}

fn make_loc(
    loc_raw: String,
    lastmod: Option<String>,
    hreflang: Vec<HreflangRef>,
    invalid: &mut u32,
) -> SitemapLoc {
    let url = validate_loc(&loc_raw);
    if url.is_none() {
        *invalid += 1;
    }
    SitemapLoc {
        loc_raw,
        url,
        lastmod: lastmod.filter(|l| !l.is_empty()),
        hreflang,
    }
}

fn parse_text<R: BufRead>(input: &mut R) -> Parsed {
    let mut parsed = Parsed::default();
    let mut line = Vec::new();
    loop {
        line.clear();
        match input.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        let loc = make_loc(text.to_string(), None, Vec::new(), &mut parsed.invalid);
        parsed.locs.push(loc);
    }
    if parsed.locs.iter().any(|l| l.url.is_some()) {
        parsed.kind = Some(SitemapKind::Urlset);
    } else {
        parsed = Parsed::default();
    }
    parsed
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Urlset,
    Index,
    Rss,
    Atom,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Loc,
    Lastmod,
}

#[derive(Default)]
struct Entry {
    depth: usize,
    loc: Option<String>,
    lastmod: Option<String>,
    hreflang: Vec<HreflangRef>,
}

struct Capture {
    field: Field,
    depth: usize,
    text: String,
}

fn local_name(e: &BytesStart<'_>) -> String {
    e.local_name().as_ref().to_string()
}

fn attr(e: &BytesStart<'_>, name: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (a.key.local_name().as_ref() == name)
            .then(|| {
                a.normalized_value(XmlVersion::Implicit1_0)
                    .ok()
                    .map(|v| v.into_owned())
            })
            .flatten()
    })
}

fn parse_xml<R: BufRead>(input: &mut R, url: &url::Url) -> Parsed {
    let mut reader = NsReader::from_reader(input);
    reader.config_mut().allow_dangling_amp = true;

    let mut parsed = Parsed::default();
    let mut mode: Option<Mode> = None;
    let mut stack: Vec<String> = Vec::new();
    let mut entry: Option<Entry> = None;
    let mut capture: Option<Capture> = None;
    let mut buf = Vec::new();

    loop {
        buf.clear();
        let Ok((ns, event)) = reader.read_resolved_event_into(&mut buf) else {
            break;
        };
        let xhtml = matches!(&ns, ResolveResult::Bound(n) if n.as_ref() == XHTML_NS);
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let name = local_name(e);
                let depth = stack.len();
                if depth == 0 && mode.is_none() {
                    mode = match name.as_str() {
                        "urlset" => Some(Mode::Urlset),
                        "sitemapindex" => Some(Mode::Index),
                        "rss" => Some(Mode::Rss),
                        "feed" => Some(Mode::Atom),
                        _ => break,
                    };
                    parsed.kind = Some(if mode == Some(Mode::Index) {
                        SitemapKind::Index
                    } else {
                        SitemapKind::Urlset
                    });
                } else if let Some(m) = mode {
                    open_element(m, &name, e, depth, xhtml, url, &mut entry, &mut capture);
                }
                if matches!(event, Event::Empty(_)) {
                    close_element(&name, depth, &mut entry, &mut capture, &mut parsed);
                } else {
                    stack.push(name);
                }
            }
            Event::End(_) => {
                let Some(name) = stack.pop() else { continue };
                close_element(&name, stack.len(), &mut entry, &mut capture, &mut parsed);
            }
            Event::Text(t) => {
                if let Some(c) = capture.as_mut() {
                    c.text.push_str(&t.xml10_content());
                }
            }
            Event::CData(t) => {
                if let Some(c) = capture.as_mut() {
                    c.text.push_str(&t.xml10_content());
                }
            }
            Event::GeneralRef(r) => {
                if let Some(c) = capture.as_mut() {
                    match r.resolve_char_ref() {
                        Ok(Some(ch)) => c.text.push(ch),
                        Ok(None) => match &*r {
                            "amp" => c.text.push('&'),
                            "lt" => c.text.push('<'),
                            "gt" => c.text.push('>'),
                            "quot" => c.text.push('"'),
                            "apos" => c.text.push('\''),
                            other => {
                                c.text.push('&');
                                c.text.push_str(other);
                                c.text.push(';');
                            }
                        },
                        Err(_) => {}
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    parsed
}

/// Name des Eintragselements je Art.
fn entry_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Urlset => "url",
        Mode::Index => "sitemap",
        Mode::Rss => "item",
        Mode::Atom => "entry",
    }
}

#[allow(clippy::too_many_arguments)]
fn open_element(
    mode: Mode,
    name: &str,
    e: &BytesStart<'_>,
    depth: usize,
    xhtml: bool,
    url: &url::Url,
    entry: &mut Option<Entry>,
    capture: &mut Option<Capture>,
) {
    let Some(current) = entry.as_mut() else {
        // RSS: `item` liegt unter channel (Tiefe 2); sonst direkt unter dem Wurzelelement
        let wanted = match mode {
            Mode::Rss => name == "item",
            _ => depth == 1 && name == entry_name(mode),
        };
        if wanted {
            *entry = Some(Entry {
                depth,
                ..Entry::default()
            });
        }
        return;
    };
    if depth != current.depth + 1 {
        return;
    }
    match (mode, name) {
        (Mode::Urlset | Mode::Index, "loc") | (Mode::Rss, "link") => {
            *capture = Some(Capture {
                field: Field::Loc,
                depth,
                text: String::new(),
            });
        }
        (Mode::Urlset | Mode::Index, "lastmod")
        | (Mode::Atom, "updated")
        | (Mode::Rss, "pubDate") => {
            *capture = Some(Capture {
                field: Field::Lastmod,
                depth,
                text: String::new(),
            });
        }
        (Mode::Urlset, "link") if xhtml => {
            let (rel, code, href) = (attr(e, "rel"), attr(e, "hreflang"), attr(e, "href"));
            if let (Some(rel), Some(code), Some(href)) = (rel, code, href) {
                if has_rel_token(&rel, "alternate") && !code.trim().is_empty() {
                    let (raw, resolved) = raw_and_resolved(url, &href);
                    let order = current.hreflang.len() as u32;
                    current.hreflang.push(make_ref(
                        HintSource::Sitemap,
                        &code,
                        raw,
                        resolved,
                        true,
                        order,
                        true,
                    ));
                }
            }
        }
        (Mode::Atom, "link") => {
            let rel = attr(e, "rel");
            if rel
                .as_deref()
                .is_none_or(|r| r.trim().eq_ignore_ascii_case("alternate"))
            {
                if let (None, Some(href)) = (&current.loc, attr(e, "href")) {
                    current.loc = Some(href);
                }
            }
        }
        _ => {}
    }
}

fn close_element(
    name: &str,
    depth: usize,
    entry: &mut Option<Entry>,
    capture: &mut Option<Capture>,
    parsed: &mut Parsed,
) {
    if capture.as_ref().is_some_and(|c| c.depth == depth) {
        if let (Some(c), Some(current)) = (capture.take(), entry.as_mut()) {
            let text = c.text.trim().to_string();
            match c.field {
                Field::Loc => current.loc = Some(text),
                Field::Lastmod => current.lastmod = Some(text),
            }
        }
    }
    if entry.as_ref().is_some_and(|e| e.depth == depth) && is_entry_name(name) {
        if let Some(done) = entry.take() {
            let (hreflang, _) = apply_limit(done.hreflang, os_model::HreflangOverLimit::Truncate);
            let loc = make_loc(
                done.loc.unwrap_or_default(),
                done.lastmod,
                hreflang,
                &mut parsed.invalid,
            );
            parsed.locs.push(loc);
        }
    }
}

fn is_entry_name(name: &str) -> bool {
    matches!(name, "url" | "sitemap" | "item" | "entry")
}

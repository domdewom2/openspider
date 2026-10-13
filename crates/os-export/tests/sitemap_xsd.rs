//! Strukturprüfer für Sitemap-Dateien gegen die eingecheckten Schemas `tests/data/xsd/*.xsd` (M1-P17,
//! OUT-120, OUT-129, OUT-131); als `#[path]`-Modul eingebunden. Eigenes Testziel ohne Tests.
//!
//! Es gibt keinen XSD-Validator mit passender Lizenz im Abhängigkeitsgraphen. Der Prüfer liest deshalb die
//! `.xsd`-Dateien selbst und wertet den Teil aus, den die Protokollschemas benutzen:
//!
//! * Wurzelelement, `sequence` mit `element` (`minOccurs`/`maxOccurs`), `any namespace="##other"`,
//! * einfache Typen als `restriction` (`enumeration`, `minLength`, `maxLength`, `minInclusive`,
//!   `maxInclusive`) und `union`, Basistypen `anyURI`, `string`, `decimal`, `date`, `dateTime`,
//! * den Namensraum (`targetNamespace`, `elementFormDefault="qualified"`).
//!
//! Nicht ausgewertet werden `pattern`-Facetten und alle übrigen XSD-Konstrukte. Ändert jemand die
//! Schemadateien um solche Konstrukte, scheitert [`Schema::load`] mit einer Meldung, statt stillschweigend
//! weniger zu prüfen.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;
use quick_xml::XmlVersion;

/// Ein Element des geprüften Dokuments.
#[derive(Debug, Default, Clone)]
pub struct Elem {
    pub ns: String,
    pub name: String,
    pub text: String,
    pub children: Vec<Elem>,
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

/// Liest ein XML-Dokument in einen Baum. Prüft dabei die Wohlgeformtheit (Entitäten, Klammerung).
pub fn parse_xml(bytes: &[u8]) -> Result<Elem, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| format!("kein UTF-8: {e}"))?;
    let mut reader = NsReader::from_str(text);
    let mut stack: Vec<Elem> = Vec::new();
    let mut root: Option<Elem> = None;
    loop {
        let (ns, event) = reader
            .read_resolved_event()
            .map_err(|e| format!("XML-Fehler: {e}"))?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let ns_text = match &ns {
                    ResolveResult::Bound(n) => n.as_ref().to_string(),
                    _ => String::new(),
                };
                let el = Elem {
                    ns: ns_text,
                    name: e.local_name().as_ref().to_string(),
                    ..Elem::default()
                };
                if matches!(event, Event::Empty(_)) {
                    attach(&mut stack, &mut root, el)?;
                } else {
                    stack.push(el);
                }
            }
            Event::End(_) => {
                let el = stack.pop().ok_or("Endtag ohne Starttag")?;
                attach(&mut stack, &mut root, el)?;
            }
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t.xml10_content());
                }
            }
            Event::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t.xml10_content());
                }
            }
            Event::GeneralRef(r) => {
                if let Some(top) = stack.last_mut() {
                    match r.resolve_char_ref() {
                        Ok(Some(ch)) => top.text.push(ch),
                        Ok(None) => match &*r {
                            "amp" => top.text.push('&'),
                            "lt" => top.text.push('<'),
                            "gt" => top.text.push('>'),
                            "quot" => top.text.push('"'),
                            "apos" => top.text.push('\''),
                            other => return Err(format!("unbekannte Entität &{other};")),
                        },
                        Err(e) => return Err(format!("ungültige Zeichenreferenz: {e}")),
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err("Dokument nicht geschlossen".to_string());
    }
    root.ok_or_else(|| "kein Wurzelelement".to_string())
}

fn attach(stack: &mut [Elem], root: &mut Option<Elem>, el: Elem) -> Result<(), String> {
    match stack.last_mut() {
        Some(parent) => parent.children.push(el),
        None => {
            if root.is_some() {
                return Err("mehr als ein Wurzelelement".to_string());
            }
            *root = Some(el);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Schema

#[derive(Debug, Clone)]
enum Particle {
    Element {
        name: String,
        ty: String,
        min: u32,
        max: Option<u32>,
    },
    Any {
        min: u32,
        max: Option<u32>,
    },
}

#[derive(Debug, Clone, Default)]
struct Simple {
    base: String,
    union: Vec<String>,
    enums: Vec<String>,
    min_len: Option<usize>,
    max_len: Option<usize>,
    min_incl: Option<f64>,
    max_incl: Option<f64>,
}

/// Aus einer `.xsd` gelesenes Schema.
#[derive(Debug, Default)]
pub struct Schema {
    target_ns: String,
    roots: HashMap<String, Vec<Particle>>,
    complex: HashMap<String, Vec<Particle>>,
    simple: HashMap<String, Simple>,
}

fn occurs(e: &BytesStart<'_>) -> (u32, Option<u32>) {
    let min = attr(e, "minOccurs")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let max = match attr(e, "maxOccurs").as_deref() {
        None => Some(1),
        Some("unbounded") => None,
        Some(v) => v.parse().ok(),
    };
    (min, max)
}

fn strip_prefix(t: &str) -> &str {
    t.rsplit(':').next().unwrap_or(t)
}

impl Schema {
    /// Liest ein Schema; unbekannte Konstrukte sind ein Fehler.
    pub fn load(path: &Path) -> Schema {
        let text =
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        Schema::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Schema, String> {
        let mut schema = Schema::default();
        let mut reader = NsReader::from_str(text);
        let mut path: Vec<String> = Vec::new();
        // Wo neue Teilchen landen: ein Wurzelelement (`root`) oder ein benannter komplexer Typ.
        let mut root_name: Option<String> = None;
        let mut complex_name: Option<String> = None;
        let mut simple_name: Option<String> = None;
        loop {
            let event = reader
                .read_event()
                .map_err(|e| format!("XSD-Fehler: {e}"))?;
            let (start, is_empty) = match &event {
                Event::Start(e) => (Some(e.clone()), false),
                Event::Empty(e) => (Some(e.clone()), true),
                _ => (None, false),
            };
            if let Some(e) = start {
                let name = e.local_name().as_ref().to_string();
                let depth = path.len();
                match name.as_str() {
                    "schema" => {
                        schema.target_ns = attr(&e, "targetNamespace").unwrap_or_default();
                    }
                    "element" if depth == 1 => {
                        let n = attr(&e, "name").ok_or("Wurzelelement ohne name")?;
                        schema.roots.insert(n.clone(), Vec::new());
                        root_name = Some(n);
                    }
                    "element" => {
                        let (min, max) = occurs(&e);
                        let particle = Particle::Element {
                            name: attr(&e, "name").ok_or("element ohne name")?,
                            ty: strip_prefix(&attr(&e, "type").ok_or("element ohne type")?)
                                .to_string(),
                            min,
                            max,
                        };
                        schema.push(&root_name, &complex_name, particle)?;
                    }
                    "complexType" => {
                        if let Some(n) = attr(&e, "name") {
                            schema.complex.insert(n.clone(), Vec::new());
                            complex_name = Some(n);
                        }
                    }
                    "sequence" => {}
                    "any" => {
                        if attr(&e, "namespace").as_deref() != Some("##other") {
                            return Err("any: nur namespace=\"##other\" wird unterstützt".into());
                        }
                        let (min, max) = occurs(&e);
                        schema.push(&root_name, &complex_name, Particle::Any { min, max })?;
                    }
                    "simpleType" => {
                        let n = attr(&e, "name").ok_or("simpleType ohne name")?;
                        schema.simple.insert(n.clone(), Simple::default());
                        simple_name = Some(n);
                    }
                    "restriction" | "union" | "enumeration" | "minLength" | "maxLength"
                    | "minInclusive" | "maxInclusive" => {
                        let s = simple_name
                            .as_ref()
                            .and_then(|n| schema.simple.get_mut(n))
                            .ok_or("Facette außerhalb eines simpleType")?;
                        let value = attr(&e, "value");
                        match name.as_str() {
                            "restriction" => {
                                s.base =
                                    strip_prefix(&attr(&e, "base").unwrap_or_default()).to_string();
                            }
                            "union" => {
                                s.union = attr(&e, "memberTypes")
                                    .unwrap_or_default()
                                    .split_whitespace()
                                    .map(|t| strip_prefix(t).to_string())
                                    .collect();
                            }
                            "enumeration" => s.enums.push(value.unwrap_or_default()),
                            "minLength" => s.min_len = value.and_then(|v| v.parse().ok()),
                            "maxLength" => s.max_len = value.and_then(|v| v.parse().ok()),
                            "minInclusive" => s.min_incl = value.and_then(|v| v.parse().ok()),
                            "maxInclusive" => s.max_incl = value.and_then(|v| v.parse().ok()),
                            _ => {}
                        }
                    }
                    other => return Err(format!("nicht unterstütztes XSD-Element <{other}>")),
                }
                if !is_empty {
                    path.push(name);
                }
            }
            match event {
                Event::End(_) => match path.pop().as_deref() {
                    Some("element") if path.len() == 1 => root_name = None,
                    Some("complexType") if path.len() == 1 => complex_name = None,
                    Some("simpleType") => simple_name = None,
                    _ => {}
                },
                Event::Eof => break,
                _ => {}
            }
        }
        Ok(schema)
    }

    fn push(
        &mut self,
        root: &Option<String>,
        complex: &Option<String>,
        p: Particle,
    ) -> Result<(), String> {
        let list = match (complex, root) {
            (Some(c), _) => self.complex.get_mut(c),
            (None, Some(r)) => self.roots.get_mut(r),
            _ => None,
        };
        list.ok_or("Teilchen ohne umgebenden Typ")?.push(p);
        Ok(())
    }

    /// Prüft ein Dokument gegen das Schema.
    pub fn validate(&self, doc: &Elem) -> Result<(), String> {
        let particles = self
            .roots
            .get(&doc.name)
            .ok_or_else(|| format!("unerwartetes Wurzelelement <{}>", doc.name))?;
        if doc.ns != self.target_ns {
            return Err(format!(
                "Wurzelelement im Namensraum {:?}, erwartet {:?}",
                doc.ns, self.target_ns
            ));
        }
        self.no_text(doc)?;
        self.children(particles, &doc.children, &doc.name)
    }

    fn no_text(&self, el: &Elem) -> Result<(), String> {
        if el.text.trim().is_empty() {
            Ok(())
        } else {
            Err(format!("<{}>: Text neben Kindelementen", el.name))
        }
    }

    fn children(&self, particles: &[Particle], children: &[Elem], at: &str) -> Result<(), String> {
        let mut pos = 0;
        for p in particles {
            let (min, max) = match p {
                Particle::Element { min, max, .. } | Particle::Any { min, max } => (*min, *max),
            };
            let mut count = 0u32;
            while pos < children.len() && max.is_none_or(|m| count < m) {
                let child = &children[pos];
                let matches = match p {
                    Particle::Element { name, .. } => {
                        child.name == *name && child.ns == self.target_ns
                    }
                    Particle::Any { .. } => child.ns != self.target_ns,
                };
                if !matches {
                    break;
                }
                if let Particle::Element { ty, name, .. } = p {
                    self.element(child, ty)
                        .map_err(|e| format!("{at}/{name}: {e}"))?;
                }
                count += 1;
                pos += 1;
            }
            if count < min {
                let what = match p {
                    Particle::Element { name, .. } => name.clone(),
                    Particle::Any { .. } => "any".to_string(),
                };
                return Err(format!(
                    "{at}: Element {what} kommt {count}-mal vor, erwartet mindestens {min}"
                ));
            }
        }
        if pos < children.len() {
            return Err(format!(
                "{at}: unerwartetes Element <{}> an Stelle {pos}",
                children[pos].name
            ));
        }
        Ok(())
    }

    fn element(&self, el: &Elem, ty: &str) -> Result<(), String> {
        if let Some(particles) = self.complex.get(ty) {
            self.no_text(el)?;
            return self.children(particles, &el.children, &el.name);
        }
        if !el.children.is_empty() {
            return Err(format!("<{}> darf keine Kindelemente haben", el.name));
        }
        self.text(ty, &el.text)
    }

    fn text(&self, ty: &str, text: &str) -> Result<(), String> {
        if let Some(s) = self.simple.get(ty) {
            if !s.union.is_empty() {
                if s.union.iter().any(|m| self.text(m, text).is_ok()) {
                    return Ok(());
                }
                return Err(format!(
                    "{text:?} passt zu keinem Typ der Vereinigung {:?}",
                    s.union
                ));
            }
            self.text(&s.base, text)?;
            if !s.enums.is_empty() && !s.enums.iter().any(|v| v == text) {
                return Err(format!("{text:?} ist keiner von {:?}", s.enums));
            }
            let len = text.chars().count();
            if s.min_len.is_some_and(|m| len < m) {
                return Err(format!("{text:?}: kürzer als {:?} Zeichen", s.min_len));
            }
            if s.max_len.is_some_and(|m| len > m) {
                return Err(format!(
                    "Wert mit {len} Zeichen: länger als {:?}",
                    s.max_len
                ));
            }
            if s.min_incl.is_some() || s.max_incl.is_some() {
                let v: f64 = text
                    .trim()
                    .parse()
                    .map_err(|_| format!("{text:?} keine Zahl"))?;
                if s.min_incl.is_some_and(|m| v < m) || s.max_incl.is_some_and(|m| v > m) {
                    return Err(format!(
                        "{v} außerhalb von {:?}..{:?}",
                        s.min_incl, s.max_incl
                    ));
                }
            }
            return Ok(());
        }
        builtin(ty, text)
    }
}

/// Eingebaute Typen (nur soweit die Schemas sie nutzen).
fn builtin(ty: &str, text: &str) -> Result<(), String> {
    match ty {
        "string" => Ok(()),
        "anyURI" => {
            if text.is_empty()
                || !text.is_ascii()
                || text.chars().any(|c| c.is_whitespace() || c.is_control())
            {
                return Err(format!(
                    "{text:?} ist keine gültige anyURI (nur ASCII, ohne Leerraum)"
                ));
            }
            url::Url::parse(text)
                .map(|_| ())
                .map_err(|e| format!("{text:?}: {e}"))
        }
        "decimal" => {
            let t = text.trim();
            let ok = !t.is_empty()
                && t.chars()
                    .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '+')
                && t.parse::<f64>().is_ok();
            ok.then_some(())
                .ok_or_else(|| format!("{text:?} ist keine Dezimalzahl"))
        }
        "date" => is_date(text)
            .then_some(())
            .ok_or_else(|| format!("{text:?} ist kein Datum")),
        "dateTime" => is_date_time(text)
            .then_some(())
            .ok_or_else(|| format!("{text:?} ist keine Datums-/Zeitangabe")),
        other => Err(format!("nicht unterstützter Basistyp {other}")),
    }
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        && matches!(s[5..7].parse::<u32>(), Ok(1..=12))
        && matches!(s[8..10].parse::<u32>(), Ok(1..=31))
}

fn is_date_time(s: &str) -> bool {
    let Some((date, rest)) = s.split_once('T') else {
        return false;
    };
    if !is_date(date) {
        return false;
    }
    let (clock, zone) = match rest.find(['Z', '+', '-']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => return false,
    };
    let clock_ok = {
        let main = clock.split('.').next().unwrap_or("");
        let parts: Vec<&str> = main.split(':').collect();
        parts.len() == 3
            && parts
                .iter()
                .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_digit()))
    };
    let zone_ok = zone == "Z"
        || (zone.len() == 6
            && (zone.starts_with('+') || zone.starts_with('-'))
            && zone.as_bytes()[3] == b':');
    clock_ok && zone_ok
}

/// Pfad der eingecheckten Schemas.
pub fn xsd_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xsd")
}

/// Schema `sitemap.xsd`.
pub fn sitemap_schema() -> Schema {
    Schema::load(&xsd_dir().join("sitemap.xsd"))
}

/// Schema `siteindex.xsd`.
pub fn index_schema() -> Schema {
    Schema::load(&xsd_dir().join("siteindex.xsd"))
}

/// Liest, prüft und validiert eine Datei gegen das Schema; Fehler als Meldung.
pub fn validate_file(schema: &Schema, path: &Path) -> Result<Elem, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let doc = parse_xml(&bytes)?;
    schema.validate(&doc)?;
    Ok(doc)
}

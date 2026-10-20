//! Kleiner, bewusst begrenzter XML-Sitemap-Leser ohne DTD, Entities oder Netzwerkzugriff.
//! Unterstützt urlset/sitemapindex und Bildattribute. Unbekannte Elemente werden abgewiesen,
//! damit ihr Inhalt nicht unbemerkt aus dem Vergleich fällt.

use std::collections::BTreeMap;
use std::path::Path;

use crate::csvio::Table;
use crate::error::ParityError;

const SITEMAP: &str = "http://www.sitemaps.org/schemas/sitemap/0.9";
const IMAGE: &str = "http://www.google.com/schemas/sitemap-image/1.1";

fn invalid() -> ParityError {
    ParityError::Invalid("ungültige oder nicht unterstützte Sitemap-XML".into())
}

fn unescape(s: &str) -> Result<String, ParityError> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let (entity, tail) = rest[i + 1..].split_once(';').ok_or_else(invalid)?;
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            n if n.starts_with('#') => {
                let cp = if let Some(hex) = n.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    n[1..].parse().ok()
                };
                out.push(
                    cp.and_then(char::from_u32)
                        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
                        .ok_or_else(invalid)?,
                );
            }
            _ => return Err(invalid()),
        }
        rest = tail;
    }
    out.push_str(rest);
    Ok(out)
}

#[derive(Default)]
struct Element {
    qualified: String,
    name: String,
    namespace: String,
    namespaces: BTreeMap<String, String>,
    text: String,
    children: Vec<Element>,
}

fn start_tag(tag: &str, parent: Option<&Element>) -> Result<Element, ParityError> {
    let mut parts = tag.trim().splitn(2, char::is_whitespace);
    let qualified = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(invalid)?
        .to_string();
    let mut namespaces = parent.map(|p| p.namespaces.clone()).unwrap_or_default();
    let mut attrs = parts.next().unwrap_or("").trim();
    while !attrs.is_empty() {
        let (key, rest) = attrs.split_once('=').ok_or_else(invalid)?;
        let key = key.trim();
        let rest = rest.trim_start();
        let quote = rest
            .chars()
            .next()
            .filter(|c| matches!(c, '\'' | '"'))
            .ok_or_else(invalid)?;
        let (value, tail) = rest[1..].split_once(quote).ok_or_else(invalid)?;
        // Nur Namespace-Deklarationen beeinflussen die Elementauflösung.
        // Andere Attribute (z. B. xsi:schemaLocation) enthalten keine Vergleichsdaten.
        let value = unescape(value)?;
        if key == "xmlns" {
            namespaces.insert(String::new(), value);
        } else if let Some(prefix) = key.strip_prefix("xmlns:") {
            namespaces.insert(prefix.to_string(), value);
        }
        attrs = tail.trim_start();
    }
    let (prefix, name) = qualified.split_once(':').unwrap_or(("", &qualified));
    let namespace = namespaces.get(prefix).cloned().ok_or_else(invalid)?;
    let name = name.to_string();
    Ok(Element {
        qualified,
        name,
        namespace,
        namespaces,
        ..Default::default()
    })
}

fn document(text: &str) -> Result<Element, ParityError> {
    let mut stack: Vec<Element> = Vec::new();
    let mut root = None;
    let mut rest = text.trim_start_matches('\u{feff}');
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("<!--") {
            rest = r.split_once("-->").ok_or_else(invalid)?.1;
            continue;
        }
        if let Some(r) = rest.strip_prefix("<?xml ") {
            if root.is_some() || !stack.is_empty() {
                return Err(invalid());
            }
            rest = r.split_once("?>").ok_or_else(invalid)?.1;
            continue;
        }
        if let Some(r) = rest.strip_prefix("<![CDATA[") {
            let (value, tail) = r.split_once("]]>").ok_or_else(invalid)?;
            stack.last_mut().ok_or_else(invalid)?.text.push_str(value);
            rest = tail;
            continue;
        }
        if !rest.starts_with('<') {
            let end = rest.find('<').unwrap_or(rest.len());
            let value = unescape(&rest[..end])?;
            if let Some(element) = stack.last_mut() {
                element.text.push_str(&value);
            } else if !value.trim().is_empty() {
                return Err(invalid());
            }
            rest = &rest[end..];
            continue;
        }
        // Ein > innerhalb eines Attributwerts beendet das Tag nicht.
        let mut quote = None;
        let end = rest
            .char_indices()
            .skip(1)
            .find_map(|(i, c)| {
                if quote == Some(c) {
                    quote = None;
                } else if quote.is_none() {
                    if matches!(c, '\'' | '"') {
                        quote = Some(c);
                    } else if c == '>' {
                        return Some(i);
                    }
                }
                None
            })
            .ok_or_else(invalid)?;
        let tag = &rest[1..end];
        rest = &rest[end + 1..];
        if let Some(name) = tag.strip_prefix('/') {
            if stack.last().map(|e| e.qualified.as_str()) != Some(name.trim()) {
                return Err(invalid());
            }
        } else {
            // Acht Ebenen reichen für sitemapindex/sitemap/loc und urlset/url/image/loc.
            // Die Grenze verhindert unbeschränkt tiefe, nicht unterstützte Strukturen.
            if tag.starts_with('!') || stack.len() >= 8 || root.is_some() {
                return Err(invalid());
            }
            let empty = tag.ends_with('/');
            stack.push(start_tag(tag.trim_end_matches('/'), stack.last())?);
            if !empty {
                continue;
            }
        }
        let element = stack.pop().ok_or_else(invalid)?;
        if let Some(parent) = stack.last_mut() {
            parent.children.push(element);
        } else {
            root = Some(element);
        }
    }
    if !stack.is_empty() {
        return Err(invalid());
    }
    root.ok_or_else(invalid)
}

/// Nur lokale Dateien; externe Entities/Includes werden nie aufgelöst.
pub(crate) fn read_sitemap(path: &Path) -> Result<Table, ParityError> {
    parse(&std::fs::read_to_string(path)?)
}

fn parse(text: &str) -> Result<Table, ParityError> {
    let root = document(text)?;
    if root.namespace != SITEMAP
        || !matches!(root.name.as_str(), "urlset" | "sitemapindex")
        || !root.text.trim().is_empty()
    {
        return Err(invalid());
    }
    let mut table = Table {
        columns: ["Address", "Lastmod", "Changefreq", "Priority", "Images"]
            .map(String::from)
            .to_vec(),
        ..Default::default()
    };
    for entry in root.children {
        if entry.namespace != SITEMAP
            || entry.name
                != if root.name == "urlset" {
                    "url"
                } else {
                    "sitemap"
                }
            || !entry.text.trim().is_empty()
        {
            return Err(invalid());
        }
        let mut fields = BTreeMap::new();
        let mut images = Vec::new();
        for child in entry.children {
            if child.namespace == IMAGE && child.name == "image" && child.text.trim().is_empty() {
                let mut attrs = BTreeMap::new();
                for attr in child.children {
                    if attr.namespace != IMAGE
                        || !matches!(
                            attr.name.as_str(),
                            "loc" | "title" | "caption" | "geo_location" | "license"
                        )
                        || !attr.children.is_empty()
                        || attrs
                            .insert(attr.name, attr.text.trim().to_string())
                            .is_some()
                    {
                        return Err(invalid());
                    }
                }
                if !attrs.get("loc").is_some_and(|s| !s.is_empty()) {
                    return Err(invalid());
                }
                // JSON vermeidet Mehrdeutigkeiten durch Trennzeichen in Bildtiteln/URLs.
                images.push(serde_json::to_string(&attrs).map_err(|_| invalid())?);
            } else if child.namespace != SITEMAP
                || !matches!(
                    child.name.as_str(),
                    "loc" | "lastmod" | "priority" | "changefreq"
                )
                || !child.children.is_empty()
                || fields
                    .insert(child.name, child.text.trim().to_string())
                    .is_some()
            {
                return Err(invalid());
            }
        }
        images.sort();
        table.rows.push(vec![
            fields
                .remove("loc")
                .filter(|s| !s.is_empty())
                .ok_or_else(invalid)?,
            fields.remove("lastmod").unwrap_or_default(),
            fields.remove("changefreq").unwrap_or_default(),
            fields.remove("priority").unwrap_or_default(),
            images.join("\n"),
        ]);
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: OUT-120
    // spec: OUT-126
    #[test]
    fn sitemap_elements_entities_and_images() {
        let t = parse(&format!(r#"<?xml version="1.0"?><urlset xmlns="{SITEMAP}" xmlns:i="{IMAGE}"><url><loc>http://127.0.0.1:8906/?a=1&amp;b=2</loc><priority>0.9</priority><i:image><i:loc>http://localhost:8906/a.png</i:loc><i:title>River &amp; stone</i:title></i:image></url></urlset>"#)).unwrap();
        assert_eq!(t.rows[0][0], "http://127.0.0.1:8906/?a=1&b=2");
        assert!(t.rows[0][4].contains("River & stone"));
        assert!(t.rows[0][4].contains("http://localhost:8906/a.png"));
        assert!(parse(&format!("<urlset xmlns='{SITEMAP}'/>"))
            .unwrap()
            .rows
            .is_empty());
        assert_eq!(parse(&format!("<s:sitemapindex xmlns:s='{SITEMAP}'><s:sitemap><s:loc><![CDATA[http://127.0.0.1:8906/sitemap.xml]]></s:loc></s:sitemap></s:sitemapindex>")).unwrap().rows.len(), 1);
    }

    // spec: OUT-120
    // spec: OUT-126
    #[test]
    fn schema_location_and_non_namespace_attributes_do_not_change_sitemap_data() {
        let plain = format!(
            r#"<urlset xmlns="{SITEMAP}" xmlns:i="{IMAGE}"><url><loc>http://localhost/a</loc><i:image><i:loc>http://localhost/a.png</i:loc></i:image></url></urlset>"#
        );
        let attributed = plain
            .replace("<urlset ", r#"<urlset xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://www.sitemaps.org/schemas/sitemap/0.9 sitemap.xsd" "#)
            .replace("<url>", r#"<url id="one" xsi:type="entry">"#)
            .replace("<loc>", r#"<loc data-note="a &amp; b > c">"#)
            .replace("<i:image>", r#"<i:image id="picture">"#);
        let expected = parse(&plain).unwrap();
        let actual = parse(&attributed).unwrap();
        assert_eq!(actual.columns, expected.columns);
        assert_eq!(actual.rows, expected.rows);
    }

    // spec: OUT-120
    #[test]
    fn malformed_or_unsupported_xml_is_never_silently_accepted() {
        for xml in [
            "<!DOCTYPE a><urlset/>",
            "<urlset xmlns='wrong'/>",
            "<urlset/>",
            "<urlset><url><loc>&file;</loc></url></urlset>",
            "<urlset><url></url></urlset>",
            "<urlset><url><loc>a</loc></url>",
        ] {
            assert!(parse(xml).is_err(), "{xml}");
        }
    }
}

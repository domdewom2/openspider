//! Bildinstanzen (ONPAGE-170, CRAWL-052): jede Einbindung eines Bildes auf der Seite.
//!
//! Quellen: `img[src]` (`Img`), `a[href]` auf eine Bildendung (`A`, ohne Alt-Information, zusätzlich zum
//! Hyperlink), `input[type=image][src]` (`InputImage`), `video[poster]` (`Poster`) und alle Kandidaten aus
//! `srcset` an `img` und `picture > source` (`Srcset`; die Engine fragt sie in M1 nicht ab). `data:`-URIs
//! werden ignoriert, `data-src` und andere Lazy-Load-Attribute nicht ausgewertet (Textmodus). CSS-Bilder
//! werden mit `images.parse_inline_css_backgrounds` zusätzlich als Instanzen erfasst.

use os_model::{ImageInstance, ImageSource};
use scraper::ElementRef;

use crate::base::{clean_url_text, is_data_uri, resolve_http};
use crate::dom::{self, Document};
use crate::links::{is_html, walk};
use crate::text;

/// Dateiendungen, an denen ein `<a href>` als Bildlink erkannt wird (ONPAGE-170).
const IMAGE_EXTENSIONS: [&str; 9] = [
    "jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "bmp", "ico",
];

/// Alt- und Größenangaben des `<img>`, zu dem eine Instanz gehört.
#[derive(Clone, Default)]
struct ImgAttrs {
    alt_present: bool,
    alt: Option<String>,
    alt_len: Option<u32>,
    width: Option<String>,
    height: Option<String>,
}

impl ImgAttrs {
    fn of(el: ElementRef) -> Self {
        let v = el.value();
        let alt = v.attr("alt").map(text::normalize_ws);
        ImgAttrs {
            alt_present: alt.is_some(),
            alt_len: alt.as_deref().map(text::char_len),
            alt,
            width: v.attr("width").map(str::to_string),
            height: v.attr("height").map(str::to_string),
        }
    }
}

/// Alle URLs aus einem `srcset`-Attribut (Parser nach dem HTML-Standard: Kommas innerhalb einer URL wie in
/// `img.jpg?x=1,2 2x` trennen nicht; Deskriptoren werden verworfen).
pub(crate) fn srcset_urls(srcset: &str) -> Vec<String> {
    let chars: Vec<char> = srcset.chars().collect();
    let n = chars.len();
    let is_space = |c: char| matches!(c, '\t' | '\n' | '\u{c}' | '\r' | ' ');
    let mut out = Vec::new();
    let mut i = 0usize;
    loop {
        while i < n && (is_space(chars[i]) || chars[i] == ',') {
            i += 1;
        }
        if i >= n {
            break;
        }
        let start = i;
        while i < n && !is_space(chars[i]) {
            i += 1;
        }
        let token: String = chars[start..i].iter().collect();
        let url = token.trim_end_matches(',');
        if token.ends_with(',') {
            // URL mit angehängtem Komma: keine Deskriptoren
        } else {
            // Deskriptoren bis zum nächsten Komma außerhalb von Klammern überspringen
            let mut parens = 0usize;
            while i < n {
                match chars[i] {
                    '(' => parens += 1,
                    ')' => parens = parens.saturating_sub(1),
                    ',' if parens == 0 => {
                        i += 1;
                        break;
                    }
                    _ => {}
                }
                i += 1;
            }
        }
        if !url.is_empty() {
            out.push(url.to_string());
        }
    }
    out
}

fn has_image_extension(url: &url::Url) -> bool {
    let last = url.path().rsplit('/').next().unwrap_or("");
    match last.rsplit_once('.') {
        Some((_, ext)) => IMAGE_EXTENSIONS
            .iter()
            .any(|known| ext.eq_ignore_ascii_case(known)),
        None => false,
    }
}

/// Extrahiert alle Bildinstanzen in Dokumentreihenfolge.
pub fn extract(doc: &Document, base: &url::Url) -> Vec<ImageInstance> {
    let mut out: Vec<ImageInstance> = Vec::new();

    let push = |raw: &str,
                source: ImageSource,
                attrs: &ImgAttrs,
                el: ElementRef,
                ordinal: u32,
                out: &mut Vec<ImageInstance>| {
        let cleaned = clean_url_text(raw);
        if cleaned.is_empty() || is_data_uri(&cleaned) {
            return;
        }
        out.push(ImageInstance {
            src_raw: raw.trim_matches(|c: char| c <= ' ').to_string(),
            resolved: resolve_http(base, &cleaned).map(String::from),
            source,
            alt_present: attrs.alt_present,
            alt: attrs.alt.clone(),
            alt_len: attrs.alt_len,
            width_attr: attrs.width.clone(),
            height_attr: attrs.height.clone(),
            xpath: dom::xpath(el),
            doc_order: ordinal,
            ..Default::default()
        });
    };

    walk(doc, |el, ordinal, _position| {
        if !is_html(el) {
            return;
        }
        let v = el.value();
        match v.name() {
            "img" => {
                let attrs = ImgAttrs::of(el);
                if let Some(src) = v.attr("src") {
                    push(src, ImageSource::Img, &attrs, el, ordinal, &mut out);
                }
                if let Some(srcset) = v.attr("srcset") {
                    for url in srcset_urls(srcset) {
                        push(&url, ImageSource::Srcset, &attrs, el, ordinal, &mut out);
                    }
                }
            }
            "source" => {
                let parent = el.parent().and_then(ElementRef::wrap);
                let Some(picture) = parent.filter(|p| is_html(*p) && p.value().name() == "picture")
                else {
                    return;
                };
                if let Some(srcset) = v.attr("srcset") {
                    // Alt-Information liefert das `<img>` desselben `<picture>`.
                    let attrs = picture
                        .children()
                        .filter_map(ElementRef::wrap)
                        .find(|c| is_html(*c) && c.value().name() == "img")
                        .map(ImgAttrs::of)
                        .unwrap_or_default();
                    for url in srcset_urls(srcset) {
                        push(&url, ImageSource::Srcset, &attrs, el, ordinal, &mut out);
                    }
                }
            }
            "a" => {
                if let Some(href) = v.attr("href") {
                    let cleaned = clean_url_text(href);
                    let is_image = resolve_http(base, &cleaned)
                        .as_ref()
                        .is_some_and(has_image_extension);
                    if is_image {
                        // Bildlink: Instanz ohne Alt-Information, Breite/Höhe unbekannt
                        push(
                            href,
                            ImageSource::A,
                            &ImgAttrs::default(),
                            el,
                            ordinal,
                            &mut out,
                        );
                    }
                }
            }
            "input" => {
                let is_image = v
                    .attr("type")
                    .is_some_and(|t| t.trim().eq_ignore_ascii_case("image"));
                if let (true, Some(src)) = (is_image, v.attr("src")) {
                    let attrs = ImgAttrs {
                        width: v.attr("width").map(str::to_string),
                        height: v.attr("height").map(str::to_string),
                        ..ImgAttrs::default()
                    };
                    push(src, ImageSource::InputImage, &attrs, el, ordinal, &mut out);
                }
            }
            "video" => {
                if let Some(poster) = v.attr("poster") {
                    push(
                        poster,
                        ImageSource::Poster,
                        &ImgAttrs::default(),
                        el,
                        ordinal,
                        &mut out,
                    );
                }
            }
            _ => {}
        }
    });
    out
}

/// Optionale Dateikopfmaße, unabhängig von interner/externer URL (ONPAGE-172).
pub fn response_facts(
    input: &crate::ResponseInput<'_>,
) -> Vec<(os_model::ExtKey, os_model::Value)> {
    if !input.cfg.crawl.images.real_dimensions_from_header || !(200..300).contains(&input.status) {
        return Vec::new();
    }
    let Some((w, h)) = header_dimensions(input.body) else {
        return Vec::new();
    };
    [("image.real_w", w), ("image.real_h", h)]
        .into_iter()
        .map(|(key, value)| {
            (
                os_model::ExtKey::from_static(key),
                os_model::Value::Int(i64::from(value)),
            )
        })
        .collect()
}

/// Instanzen bleiben auch bei ausgeschalteter srcset-Entdeckung erhalten.
pub fn extract_with(
    doc: &crate::dom::Document,
    base: &url::Url,
    cfg: &crate::ParseConfig,
) -> (Vec<os_model::ImageInstance>, Vec<os_model::ExtractedLink>) {
    let mut instances = extract(doc, base);
    // Ein Durchlauf statt einer DOM-Suche je Instanz; eigene Pfad-/Positionsregeln liefert D16.
    let mut metadata = std::collections::HashMap::new();
    walk(doc, |el, ordinal, landmark| {
        let path = crate::link_position::element_path(doc, el);
        let position = crate::link_position::classify(&path, landmark, &cfg.link_positions);
        metadata.insert(ordinal, (path, position, dom::in_head(el)));
    });
    let mut links = Vec::new();
    for instance in &mut instances {
        if let Some((path, position, in_head)) = metadata.get(&instance.doc_order) {
            instance.xpath.clone_from(path);
            instance.link_position = Some(position.clone());
            if cfg.extraction.srcset && instance.source == ImageSource::Srcset {
                links.push(os_model::ExtractedLink {
                    link_type: os_model::LinkType::Image,
                    href_raw: instance.src_raw.clone(),
                    resolved: instance.resolved.clone(),
                    alt_text: instance.alt.clone(),
                    path_type: crate::base::path_type(&instance.src_raw),
                    xpath: path.clone(),
                    position: position.clone(),
                    in_head: *in_head,
                    doc_order: instance.doc_order,
                    ..Default::default()
                });
            }
        }
    }
    if cfg.crawl.images.parse_inline_css_backgrounds {
        for link in crate::css::extract_inline(doc, base) {
            if link.link_type != os_model::LinkType::Image || link.resolved.is_none() {
                continue;
            }
            let position =
                crate::link_position::classify(&link.xpath, link.position, &cfg.link_positions);
            instances.push(ImageInstance {
                src_raw: link.href_raw,
                resolved: link.resolved,
                source: ImageSource::Css,
                xpath: link.xpath,
                link_position: Some(position),
                doc_order: link.doc_order,
                ..Default::default()
            });
        }
        // Die CSS-Links kommen bereits aus css::extract_inline in parse(); nicht doppelt entdecken.
        instances.sort_by_key(|i| i.doc_order);
    }
    (instances, links)
}

fn be32(b: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        b.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn le16(b: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes(
        b.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
    )))
}
fn le24(b: &[u8], offset: usize) -> Option<u32> {
    let b = b.get(offset..offset.checked_add(3)?)?;
    Some(u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16)
}

/// Nur Metadaten lesen, niemals Pixel dekodieren; abgeschnittene/ungültige Köpfe bleiben leer.
fn header_dimensions(b: &[u8]) -> Option<(u32, u32)> {
    let dims = if b.starts_with(b"\x89PNG\r\n\x1a\n")
        && b.get(12..16) == Some(b"IHDR")
        && be32(b, 8) == Some(13)
    {
        (be32(b, 16)?, be32(b, 20)?)
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        (le16(b, 6)?, le16(b, 8)?)
    } else if b.starts_with(b"\xff\xd8") {
        jpeg_dimensions(b)?
    } else if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        webp_dimensions(b)?
    } else if b.get(4..8) == Some(b"ftyp") {
        avif_dimensions(b)?
    } else {
        return None;
    };
    (dims.0 > 0 && dims.1 > 0).then_some(dims)
}

fn jpeg_dimensions(b: &[u8]) -> Option<(u32, u32)> {
    let mut p = 2;
    while b.get(p) == Some(&0xff) {
        while b.get(p) == Some(&0xff) {
            p += 1;
        }
        let marker = *b.get(p)?;
        p += 1;
        if matches!(marker, 0xd9 | 0xda | 0x00) {
            return None;
        }
        if matches!(marker, 0x01 | 0xd0..=0xd8) {
            continue;
        }
        let len = usize::from(u16::from_be_bytes(b.get(p..p + 2)?.try_into().ok()?));
        if len < 2 {
            return None;
        }
        let segment = b.get(p..p.checked_add(len)?)?;
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            let height = u32::from(u16::from_be_bytes(segment.get(3..5)?.try_into().ok()?));
            let width = u32::from(u16::from_be_bytes(segment.get(5..7)?.try_into().ok()?));
            return Some((width, height));
        }
        p += len;
    }
    None
}

fn webp_dimensions(b: &[u8]) -> Option<(u32, u32)> {
    let mut p = 12usize;
    while p.checked_add(8)? <= b.len() {
        let kind = b.get(p..p + 4)?;
        let len =
            usize::try_from(u32::from_le_bytes(b.get(p + 4..p + 8)?.try_into().ok()?)).ok()?;
        let payload = b.get(p + 8..p.checked_add(8)?.checked_add(len)?)?;
        match kind {
            b"VP8X" if payload.len() >= 10 => {
                return Some((le24(payload, 4)? + 1, le24(payload, 7)? + 1))
            }
            b"VP8 " if payload.get(3..6) == Some(b"\x9d\x01\x2a") && payload[0] & 1 == 0 => {
                return Some((le16(payload, 6)? & 0x3fff, le16(payload, 8)? & 0x3fff))
            }
            b"VP8L" if payload.first() == Some(&0x2f) => {
                let bits = u32::from_le_bytes(payload.get(1..5)?.try_into().ok()?);
                return Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1));
            }
            _ => {}
        }
        p = p.checked_add(8)?.checked_add(len)?.checked_add(len % 2)?;
    }
    None
}

// ISO BMFF: die räumliche Eigenschaft des primären Bild-Items, nicht ein Thumbnail/Alpha-Item.
fn avif_dimensions(b: &[u8]) -> Option<(u32, u32)> {
    fn boxes(mut data: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
        let mut out = Vec::new();
        while !data.is_empty() {
            let size = be32(data, 0)?;
            let kind = data.get(4..8)?.try_into().ok()?;
            let (size, head) = match size {
                0 => (data.len(), 8),
                1 => (
                    usize::try_from(u64::from_be_bytes(data.get(8..16)?.try_into().ok()?)).ok()?,
                    16,
                ),
                n => (usize::try_from(n).ok()?, 8),
            };
            if size < head {
                return None;
            }
            out.push((kind, data.get(head..size)?));
            data = data.get(size..)?;
        }
        Some(out)
    }
    let top = boxes(b)?;
    let ftyp = top.iter().find(|(k, _)| k == b"ftyp")?.1;
    if ftyp.len() < 8
        || !std::iter::once(&ftyp[..4])
            .chain(
                ftyp[8..]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|brand| brand.as_slice()),
            )
            .any(|brand| matches!(brand, b"avif" | b"avis"))
    {
        return None;
    }
    let meta = boxes(top.iter().find(|(k, _)| k == b"meta")?.1.get(4..)?)?;
    let pitm = meta.iter().find(|(k, _)| k == b"pitm")?.1;
    let primary = if *pitm.first()? == 0 {
        u32::from(u16::from_be_bytes(pitm.get(4..6)?.try_into().ok()?))
    } else {
        be32(pitm, 4)?
    };
    let iprp = boxes(meta.iter().find(|(k, _)| k == b"iprp")?.1)?;
    let props = boxes(iprp.iter().find(|(k, _)| k == b"ipco")?.1)?;
    for (_, ipma) in iprp.iter().filter(|(k, _)| k == b"ipma") {
        let version = *ipma.first()?;
        let wide = ipma.get(3)? & 1 != 0;
        let count = be32(ipma, 4)?;
        let mut p = 8;
        for _ in 0..count {
            let id = if version == 0 {
                let id = u32::from(u16::from_be_bytes(ipma.get(p..p + 2)?.try_into().ok()?));
                p += 2;
                id
            } else {
                let id = be32(ipma, p)?;
                p += 4;
                id
            };
            let count = *ipma.get(p)?;
            p += 1;
            for _ in 0..count {
                let index = if wide {
                    let v = u16::from_be_bytes(ipma.get(p..p + 2)?.try_into().ok()?) & 0x7fff;
                    p += 2;
                    usize::from(v)
                } else {
                    let v = ipma.get(p)? & 0x7f;
                    p += 1;
                    usize::from(v)
                };
                if id == primary && index > 0 {
                    let (kind, data) = props.get(index - 1)?;
                    if kind == b"ispe" {
                        return Some((be32(data, 4)?, be32(data, 8)?));
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srcset_parser() {
        assert_eq!(srcset_urls("a.jpg 1x, b.jpg 2x"), ["a.jpg", "b.jpg"]);
        assert_eq!(srcset_urls("a.jpg 480w, b.jpg 800w"), ["a.jpg", "b.jpg"]);
        assert_eq!(
            srcset_urls("img.jpg?x=1,2 2x, c.jpg"),
            ["img.jpg?x=1,2", "c.jpg"]
        );
        assert_eq!(srcset_urls("a.jpg,b.jpg"), ["a.jpg,b.jpg"]);
        assert_eq!(srcset_urls("a.jpg, b.jpg"), ["a.jpg", "b.jpg"]);
        assert!(srcset_urls("  ,, ").is_empty());
    }

    #[test]
    fn image_extension_check() {
        let u = |s: &str| url::Url::parse(s).unwrap();
        assert!(has_image_extension(&u("https://a.test/x/Foto.JPG?v=1#f")));
        assert!(has_image_extension(&u("https://a.test/logo.svg")));
        assert!(!has_image_extension(&u("https://a.test/jpg")));
        assert!(!has_image_extension(&u("https://a.test/a.jpg/index.html")));
    }
}

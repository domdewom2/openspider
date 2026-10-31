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
    if !input.cfg.crawl.images.real_dimensions_from_header
        || !(200..300).contains(&input.status)
        || input.truncated
    {
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
        for link in crate::css::extract_inline(doc, base, true) {
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

/// Bounded top-left preview of an 8-bit RGB/RGBA PNG (the render screenshot encoding).
/// Reads only the required scanline prefix; never allocates a full long-page bitmap.
/// Returns None for unsupported encodings or invalid data. Does not render or fetch.
pub fn png_preview(data: &[u8], max_side: u32, max_bytes: usize) -> Option<(Vec<u8>, u32, u32)> {
    use std::io::{Read, Write};
    if !data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let mut cursor = 8usize;
    let mut header = None;
    let mut compressed = Vec::new();
    while cursor.checked_add(12)? <= data.len() {
        let size = u32::from_be_bytes(data[cursor..cursor + 4].try_into().ok()?) as usize;
        let end = cursor.checked_add(12)?.checked_add(size)?;
        if end > data.len() {
            return None;
        }
        let kind = &data[cursor + 4..cursor + 8];
        let chunk = &data[cursor + 8..end - 4];
        match kind {
            b"IHDR" if size == 13 => header = Some(chunk.to_vec()),
            b"IDAT" => compressed.extend_from_slice(chunk),
            b"IEND" => break,
            _ => {}
        }
        cursor = end;
    }
    let mut header = header?;
    let original_width = u32::from_be_bytes(header[0..4].try_into().ok()?);
    let original_height = u32::from_be_bytes(header[4..8].try_into().ok()?);
    let channels = match header[9] {
        2 => 3usize,
        6 => 4,
        _ => return None,
    };
    if header[8] != 8
        || header[10..13] != [0, 0, 0]
        || original_width == 0
        || original_width > 16384
        || original_height == 0
    {
        return None;
    }
    let width = original_width.min(max_side);
    if width == 0 {
        return None;
    }
    let stride = original_width as usize * channels;
    let cropped_stride = width as usize * channels;
    // Signature + IHDR/IDAT/IEND = 57 bytes; a flushed zlib stream needs
    // at most eight more bytes for finalization. Height follows compressed size.
    let compressed_budget = max_bytes.checked_sub(57 + 8)?;
    let mut height = original_height.min(max_side);
    if height == 0 {
        return None;
    }
    let compressed = 'encode: loop {
        let mut decoder = flate2::read::ZlibDecoder::new(compressed.as_slice());
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        let mut previous = vec![0u8; stride];
        let mut current = vec![0u8; stride];
        let mut filter = [0u8; 1];
        let rows = height;
        for row in 0..rows {
            decoder.read_exact(&mut filter).ok()?;
            decoder.read_exact(&mut current).ok()?;
            for x in 0..stride {
                let left = if x >= channels {
                    current[x - channels]
                } else {
                    0
                };
                let up = previous[x];
                let diagonal = if x >= channels {
                    previous[x - channels]
                } else {
                    0
                };
                let prediction = match filter[0] {
                    0 => 0,
                    1 => left,
                    2 => up,
                    3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                    4 => {
                        let p = i32::from(left) + i32::from(up) - i32::from(diagonal);
                        let a = (p - i32::from(left)).abs();
                        let b = (p - i32::from(up)).abs();
                        let c = (p - i32::from(diagonal)).abs();
                        if a <= b && a <= c {
                            left
                        } else if b <= c {
                            up
                        } else {
                            diagonal
                        }
                    }
                    _ => return None,
                };
                current[x] = current[x].wrapping_add(prediction);
            }
            encoder.write_all(&[0]).ok()?;
            encoder.write_all(&current[..cropped_stride]).ok()?;
            // Flush makes total_out reflect this row rather than buffered input. Keep
            // space for the final deflate block/checksum as well as the PNG chunks.
            encoder.flush().ok()?;
            if encoder.total_out() > compressed_budget as u64 {
                height = row;
                if height == 0 {
                    return None;
                }
                // Replay only the accepted prefix with a fresh encoder. This avoids
                // retaining a full decoded bitmap or publishing a partial zlib stream.
                continue 'encode;
            }
            std::mem::swap(&mut previous, &mut current);
        }
        break encoder.finish().ok()?;
    };
    header[0..4].copy_from_slice(&width.to_be_bytes());
    header[4..8].copy_from_slice(&height.to_be_bytes());
    let mut output = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, bytes) in [
        (b"IHDR", header.as_slice()),
        (b"IDAT", compressed.as_slice()),
        (b"IEND", &[]),
    ] {
        output.extend_from_slice(&u32::try_from(bytes.len()).ok()?.to_be_bytes());
        output.extend_from_slice(kind);
        output.extend_from_slice(bytes);
        let mut crc = u32::MAX;
        for byte in kind.iter().chain(bytes) {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        output.extend_from_slice(&(!crc).to_be_bytes());
    }
    (output.len() <= max_bytes).then_some((output, width, height))
}

/// Image-header dimensions for local previews (M2-K09, K2-61).
pub fn preview_dimensions(body: &[u8]) -> Option<(u32, u32)> {
    header_dimensions(body)
}

/// Restrictive SVG preview: retain geometry, remove active content, external references and CSS.
/// The source file is never modified. Unknown elements are omitted with their entire subtree.
pub fn safe_svg_preview(body: &[u8]) -> Option<Vec<u8>> {
    use quick_xml::{
        events::{BytesStart, Event},
        Reader, Writer,
    };
    let mut reader = Reader::from_reader(body);
    let mut writer = Writer::new(Vec::new());
    let mut skip = 0usize;
    let mut root = false;
    loop {
        match reader.read_event().ok()? {
            Event::Start(e) | Event::Empty(e) => {
                let empty =
                    body.get(reader.buffer_position().saturating_sub(2) as usize) == Some(&b'/');
                let name = e.name();
                let allowed = matches!(
                    name.as_ref(),
                    "svg"
                        | "g"
                        | "path"
                        | "rect"
                        | "circle"
                        | "ellipse"
                        | "line"
                        | "polyline"
                        | "polygon"
                        | "text"
                        | "tspan"
                        | "defs"
                        | "linearGradient"
                        | "radialGradient"
                        | "stop"
                        | "clipPath"
                        | "mask"
                        | "title"
                        | "desc"
                );
                if skip > 0 || !allowed {
                    if !empty {
                        skip += 1;
                    }
                    continue;
                }
                let label = name.as_ref();
                if !root {
                    if label != "svg" {
                        return None;
                    }
                    root = true;
                }
                let mut tag = BytesStart::new(label);
                for attr in e.attributes() {
                    let attr = attr.ok()?;
                    let key = attr.key.as_ref();
                    let value = attr
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .ok()?;
                    let allowed = matches!(
                        key,
                        "xmlns"
                            | "viewBox"
                            | "width"
                            | "height"
                            | "x"
                            | "y"
                            | "x1"
                            | "y1"
                            | "x2"
                            | "y2"
                            | "cx"
                            | "cy"
                            | "r"
                            | "rx"
                            | "ry"
                            | "d"
                            | "points"
                            | "transform"
                            | "fill"
                            | "fill-rule"
                            | "fill-opacity"
                            | "stroke"
                            | "stroke-width"
                            | "stroke-opacity"
                            | "opacity"
                            | "offset"
                            | "stop-color"
                            | "stop-opacity"
                            | "font-size"
                            | "font-family"
                            | "text-anchor"
                            | "id"
                            | "gradientUnits"
                            | "gradientTransform"
                    );
                    if allowed
                        && !value.to_lowercase().contains("url(")
                        && !value.contains(['<', '>'])
                    {
                        tag.push_attribute((key, value.as_ref()));
                    }
                }
                writer
                    .write_event(if empty {
                        Event::Empty(tag)
                    } else {
                        Event::Start(tag)
                    })
                    .ok()?;
            }
            Event::End(e) => {
                if skip > 0 {
                    skip -= 1;
                } else {
                    writer.write_event(Event::End(e)).ok()?;
                }
            }
            Event::Text(e) if skip == 0 => {
                writer.write_event(Event::Text(e)).ok()?;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.then(|| writer.into_inner())
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

//! Bildinstanzen (ONPAGE-170, CRAWL-052): jede Einbindung eines Bildes auf der Seite.
//!
//! Quellen: `img[src]` (`Img`), `a[href]` auf eine Bildendung (`A`, ohne Alt-Information, zusätzlich zum
//! Hyperlink), `input[type=image][src]` (`InputImage`), `video[poster]` (`Poster`) und alle Kandidaten aus
//! `srcset` an `img` und `picture > source` (`Srcset`; die Engine fragt sie in M1 nicht ab). `data:`-URIs
//! werden ignoriert, `data-src` und andere Lazy-Load-Attribute nicht ausgewertet (Textmodus). CSS-Bilder
//! entstehen als Links in `css`, nicht als Instanzen.

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

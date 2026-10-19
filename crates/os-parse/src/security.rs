//! Security facts for HTML and HTTP responses (ONPAGE-033 … 043).
use os_model::{ExtKey, FormRef, ParsedPage, Value};
use serde_json::json;

use crate::{base, dom, links, PageCtx, ResponseInput};

fn fact(key: &'static str, value: impl Into<Value>) -> (ExtKey, Value) {
    (ExtKey::from_static(key), value.into())
}

/// Extracts the exact resource element set from ONPAGE-033, independently of crawl switches.
/// Reference identities let exports select individual causes, including repeated occurrences.
pub fn run(ctx: &PageCtx<'_>, page: &mut ParsedPage) {
    let mut resources = Vec::new();
    let mut forms = Vec::new();
    links::walk(ctx.doc, |el, ordinal, position| {
        if !links::is_html(el) {
            return;
        }
        let v = el.value();
        let name = v.name();
        if name == "form" {
            let action = v.attr("action").map(str::to_owned);
            let cleaned = base::clean_url_text(action.as_deref().unwrap_or_default());
            // An absent/empty action submits to the document URL, even with <base>.
            let resolved = if cleaned.is_empty() {
                Some(ctx.url.to_string())
            } else {
                base::resolve_string(ctx.base, &cleaned)
            };
            forms.push(json!({"action": resolved, "path": dom::xpath(el),
                "position": position.to_string(), "raw": action}));
            page.forms.push(FormRef {
                idx: ordinal,
                action,
                action_resolved: resolved,
                method: v
                    .attr("method")
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase(),
            });
        }
        let mut raw = Vec::new();
        let attrs: &[&str] = match name {
            "img" | "script" | "iframe" | "audio" | "source" | "track" | "embed" => &["src"],
            "video" => &["src", "poster"],
            "object" => &["data"],
            "link"
                if v.attr("rel").is_some_and(|r| {
                    r.split_ascii_whitespace()
                        .any(|t| t.eq_ignore_ascii_case("stylesheet"))
                }) =>
            {
                &["href"]
            }
            _ => &[],
        };
        for attr in attrs {
            raw.extend(v.attr(attr).map(str::to_owned));
        }
        let picture_source = name == "source"
            && el
                .parent()
                .and_then(scraper::ElementRef::wrap)
                .is_some_and(|p| links::is_html(p) && p.value().name() == "picture");
        if name == "img" || picture_source {
            if let Some(srcset) = v.attr("srcset") {
                raw.extend(crate::images::srcset_urls(srcset));
            }
        }
        for raw in raw {
            let cleaned = base::clean_url_text(&raw);
            if cleaned.is_empty() {
                continue;
            }
            if let Some(resolved) = base::resolve_http(ctx.base, &cleaned) {
                let kind = match name {
                    "img" => "Image",
                    "source" if picture_source => "Image",
                    "video" if v.attr("poster") == Some(raw.as_str()) => "Image",
                    "video" | "audio" | "source" | "track" => "Media",
                    "script" => "JavaScript",
                    "link" => "CSS",
                    "iframe" => "Iframe",
                    _ => "Other",
                };
                let tokens: Vec<_> = v
                    .attr("rel")
                    .unwrap_or_default()
                    .split_ascii_whitespace()
                    .map(str::to_ascii_lowercase)
                    .collect();
                let nofollow = tokens
                    .iter()
                    .any(|t| matches!(t.as_str(), "nofollow" | "ugc" | "sponsored"));
                let rel = (!tokens.is_empty()).then(|| tokens.join(" "));
                resources.push(json!({"path": dom::xpath(el), "raw": raw,
                    "resolved": resolved.as_str(), "type":kind, "order":ordinal,
                    "position":position.to_string(), "in_head":dom::in_head(el),
                    "path_type":base::path_type(&cleaned).to_string(),
                    "alt":v.attr("alt").map(crate::text::normalize_ws), "rel":rel, "follow":page.effective_follow && !nofollow,
                    "http": resolved.scheme() == "http", "protocol_relative": raw.trim().starts_with("//")}));
            }
        }
    });
    let mixed = ctx.url.scheme() == "https" && resources.iter().any(|r| r["http"] == true);
    let protorel = resources.iter().any(|r| r["protocol_relative"] == true);
    let insecure_form = page.forms.iter().any(|f| {
        f.action_resolved
            .as_deref()
            .is_some_and(|a| a.starts_with("http://"))
    });
    // Preserve references omitted from the generic link table by store-only switches (notably hreflang).
    // Exports merge these with generic links by occurrence, so enabling crawling never duplicates a row.
    let mut references = resources.clone();
    references.extend(page.hreflang.iter().enumerate().filter(|(_, h)| h.source == os_model::HintSource::Html).filter_map(|(order, h)| {
        h.resolved.as_ref().map(|resolved| json!({"path":h.xpath, "raw":h.href_raw, "resolved":resolved,
            "type":"Hreflang", "order":order, "position":if h.in_head {"Head"} else {"Content"},
            "path_type":base::path_type(&base::clean_url_text(&h.href_raw)).to_string(), "rel":"alternate", "follow":true}))
    }));
    page.ext.extend([
        fact(
            "security.references",
            serde_json::to_string(&references).expect("reference facts"),
        ),
        fact("security.mixed_content", mixed),
        fact("security.protocol_relative", protorel),
        fact("security.form_insecure", insecure_form),
        fact(
            "security.form_on_http",
            ctx.url.scheme() == "http" && !page.forms.is_empty(),
        ),
        fact(
            "security.resources",
            serde_json::to_string(&resources).expect("resource facts"),
        ),
        fact(
            "security.forms",
            serde_json::to_string(&forms).expect("form facts"),
        ),
    ]);
}

/// Header and MIME facts for every internal HTTP response, including redirects and non-HTML.
pub fn response_facts(input: &ResponseInput<'_>) -> Vec<(ExtKey, Value)> {
    if !input.internal || input.status == 0 || !matches!(input.url.scheme(), "http" | "https") {
        return Vec::new();
    }
    let values = |name: &str| -> Vec<&str> {
        input
            .headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect()
    };
    let present = |name: &str| !values(name).is_empty();
    let exact = |name: &str, allowed: &[&str]| {
        let vals = values(name);
        vals.len() == 1
            && allowed
                .iter()
                .any(|a| vals[0].trim().eq_ignore_ascii_case(a))
    };
    let policies = values("referrer-policy");
    let effective_policy = policies
        .iter()
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .rfind(|v| {
            matches!(
                *v,
                "no-referrer"
                    | "no-referrer-when-downgrade"
                    | "origin"
                    | "origin-when-cross-origin"
                    | "same-origin"
                    | "strict-origin"
                    | "strict-origin-when-cross-origin"
                    | "unsafe-url"
            )
        });
    let secure_policy = effective_policy.is_some_and(|v| {
        matches!(
            v,
            "no-referrer"
                | "no-referrer-when-downgrade"
                | "strict-origin"
                | "strict-origin-when-cross-origin"
        )
    });
    let content_types = values("content-type");
    let declared = content_types
        .first()
        .map(|s| {
            s.split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        })
        .unwrap_or_default();
    let extension = input
        .url
        .path()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase());
    let expected = extension.as_deref().and_then(expected_types);
    let sniffed = sniff(input.body);
    let bad = content_types.len() != 1
        || !valid_mime(&declared)
        || expected.is_some_and(|types| {
            !extension_matches(extension.as_deref().unwrap_or_default(), &declared, types)
        })
        || sniffed.is_some_and(|types| !types.contains(&declared.as_str()))
        || input
            .cfg
            .crawl
            .security
            .obsolete_mime_types
            .iter()
            .any(|m| m.eq_ignore_ascii_case(&declared));
    let expected_text = sniffed
        .or(expected)
        .map(|types| types[0])
        .unwrap_or_default();
    vec![
        fact(
            "security.missing_hsts",
            input.url.scheme() == "https" && !present("strict-transport-security"),
        ),
        fact("security.missing_csp", !present("content-security-policy")),
        fact(
            "security.missing_xcto",
            !exact("x-content-type-options", &["nosniff"]),
        ),
        fact(
            "security.missing_xfo",
            !exact("x-frame-options", &["deny", "sameorigin"]),
        ),
        fact("security.missing_referrer_policy", !secure_policy),
        fact("security.bad_content_type", bad),
        fact("security.expected_content_type", expected_text),
    ]
}

fn valid_mime(mime: &str) -> bool {
    fn token(s: &str) -> bool {
        !s.is_empty()
            && s.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c))
    }
    mime.split_once('/')
        .is_some_and(|(a, b)| token(a) && token(b))
}

fn expected_types(ext: &str) -> Option<&'static [&'static str]> {
    Some(match ext {
        "html" | "htm" => &["text/html", "application/xhtml+xml"],
        "css" => &["text/css"],
        "js" | "mjs" => &[
            "application/javascript",
            "text/javascript",
            "application/x-javascript",
            "application/ecmascript",
            "text/ecmascript",
            "application/x-ecmascript",
            "text/x-javascript",
        ],
        "png" => &["image/png"],
        "jpg" | "jpeg" => &["image/jpeg"],
        "gif" => &["image/gif"],
        "webp" => &["image/webp"],
        "avif" => &["image/avif"],
        "svg" => &["image/svg+xml"],
        "ico" => &["image/x-icon", "image/vnd.microsoft.icon"],
        "pdf" => &["application/pdf"],
        "xml" => &["application/xml", "text/xml"],
        "json" => &["application/json", "text/json"],
        "woff" => &[
            "font/woff",
            "application/font-woff",
            "application/x-font-woff",
        ],
        "woff2" => &["font/woff2"],
        "ttf" => &["font/ttf", "application/x-font-ttf"],
        "otf" => &["font/otf", "application/x-font-opentype"],
        "mp4" => &["video/mp4", "audio/mp4", "application/mp4"],
        "webm" => &["video/webm", "audio/webm"],
        "mp3" => &["audio/mpeg", "audio/mp3"],
        _ => return None,
    })
}

// Conservative signatures only: ordinary text/JavaScript/CSS cannot be identified reliably.
fn sniff(body: &[u8]) -> Option<&'static [&'static str]> {
    let ext = if body.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if body.starts_with(b"\xff\xd8\xff") {
        "jpg"
    } else if body.starts_with(b"GIF87a") || body.starts_with(b"GIF89a") {
        "gif"
    } else if body.starts_with(b"RIFF") && body.get(8..12) == Some(b"WEBP") {
        "webp"
    } else if body.get(4..8) == Some(b"ftyp") && matches!(body.get(8..12), Some(b"avif" | b"avis"))
    {
        "avif"
    } else if body.get(4..8) == Some(b"ftyp")
        && matches!(
            body.get(8..12),
            Some(b"isom" | b"iso2" | b"mp41" | b"mp42" | b"M4V " | b"M4A ")
        )
    {
        "mp4"
    } else if body.starts_with(b"ID3") {
        "mp3"
    } else if body.starts_with(b"\0\0\x01\0") {
        "ico"
    } else if body.starts_with(b"%PDF-") {
        "pdf"
    } else if body.starts_with(b"wOFF") {
        "woff"
    } else if body.starts_with(b"wOF2") {
        "woff2"
    } else if body.starts_with(b"\0\x01\0\0") {
        "ttf"
    } else if body.starts_with(b"OTTO") {
        "otf"
    } else {
        let text = String::from_utf8_lossy(&body[..body.len().min(512)]);
        let text = text
            .trim_start_matches('\u{feff}')
            .trim_start()
            .to_ascii_lowercase();
        if text.starts_with("<!doctype html")
            || text.starts_with("<html")
            || text.starts_with("<head")
            || text.starts_with("<body")
        {
            "html"
        } else if text.starts_with("<svg") || (text.starts_with("<?xml") && text.contains("<svg")) {
            "svg"
        } else {
            return None;
        }
    };
    expected_types(ext)
}

// ONPAGE-020 groups extension checks by content class; magic bytes identify the specific format.
fn extension_matches(ext: &str, declared: &str, expected: &[&str]) -> bool {
    match ext {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "svg" | "ico" => {
            declared.starts_with("image/")
        }
        "woff" | "woff2" | "ttf" | "otf" => {
            declared.starts_with("font/")
                || matches!(
                    declared,
                    "application/font-woff"
                        | "application/font-woff2"
                        | "application/x-font-ttf"
                        | "application/x-font-otf"
                        | "application/vnd.ms-fontobject"
                )
        }
        "mp4" | "webm" | "mp3" => declared.starts_with("video/") || declared.starts_with("audio/"),
        "xml" => matches!(
            declared,
            "application/xml" | "text/xml" | "application/rss+xml" | "application/atom+xml"
        ),
        "json" => {
            !["html", "js", "css", "png", "pdf", "woff", "mp4", "xml"]
                .iter()
                .any(|class| {
                    extension_matches(class, declared, expected_types(class).expect("known class"))
                })
                && !matches!(
                    declared,
                    "application/x-shockwave-flash" | "application/futuresplash"
                )
        }
        _ => expected.contains(&declared),
    }
}

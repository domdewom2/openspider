//! Dokumenteigenschaften von PDF-Dateien (ONPAGE-166, Stufe 1: Titel und Keywords).
//!
//! Gelesen werden `Title` und `Keywords` aus dem Info-Dictionary des Trailers (nur Metadaten, das Dokument
//! wird nicht vollständig geladen). Text-Strings werden nach PDF 2.0 dekodiert (PDFDocEncoding, UTF-16BE mit
//! BOM, UTF-8 mit BOM) und mit N(x) normalisiert. Fehlt eine Eigenschaft, ist sie leer oder besteht sie nur
//! aus Leerraum, ist sie `None` – es gibt keinen Ersatz durch Dateinamen oder erste Textzeile. Jeder Fehler
//! (kaputte Datei, keine PDF, Passwortschutz ohne leeres Benutzerkennwort) ergibt leere Eigenschaften, nie
//! einen Abbruch des Crawls.

use std::panic::{catch_unwind, AssertUnwindSafe};

use os_model::PdfMeta;

use crate::text;

fn clean(value: Option<String>) -> Option<String> {
    value
        // lopdf lässt die UTF-8-BOM (PDF 2.0) im dekodierten String stehen.
        .map(|v| text::normalize(v.trim_start_matches('\u{FEFF}')))
        .filter(|v| !v.is_empty())
}

/// Liest Titel und Schlüsselwörter aus dem Info-Dictionary.
pub fn meta(body: &[u8]) -> PdfMeta {
    // Der Parser verarbeitet fremde Eingaben: ein interner Fehler darf nie den Crawl abbrechen.
    let loaded = catch_unwind(AssertUnwindSafe(|| {
        lopdf::Document::load_metadata_mem(body)
    }));
    match loaded {
        Ok(Ok(info)) => PdfMeta {
            title: clean(info.title),
            keywords: clean(info.keywords),
        },
        _ => PdfMeta::default(),
    }
}

/// Extrahiert URI-Annotationen unabhängig vom Textspeicher-Schalter.
pub fn parse(body: &[u8], cfg: &crate::ParseConfig) -> os_model::PdfResult {
    let mut result =
        catch_unwind(AssertUnwindSafe(|| parse_inner(body, cfg))).unwrap_or_else(|_| {
            os_model::PdfResult {
                error: Some("PDF konnte nicht gelesen werden".into()),
                ..Default::default()
            }
        });
    if let Some(error) = &result.error {
        result.ext.push((
            os_model::ExtKey::from_static("content.pdf_error"),
            os_model::Value::Text(error.clone()),
        ));
    }
    result
}

fn parse_inner(body: &[u8], cfg: &crate::ParseConfig) -> os_model::PdfResult {
    use os_model::{ExtKey, Value};
    let mut result = os_model::PdfResult::default();
    let doc = match lopdf::Document::load_mem(body) {
        Ok(doc) if !doc.is_encrypted() => doc,
        _ => {
            result.error = Some("PDF ist beschädigt oder verschlüsselt".into());
            return result;
        }
    };
    result.meta = meta(body);
    let pages = doc.get_pages();
    for (&number, &id) in &pages {
        if let Ok(annotations) = doc.get_page_annotations(id) {
            for annotation in annotations {
                let action = annotation
                    .get(b"A")
                    .ok()
                    .and_then(|a| doc.dereference(a).ok())
                    .and_then(|(_, a)| a.as_dict().ok());
                let Some(action) = action else {
                    continue;
                };
                if action.get(b"S").and_then(|s| s.as_name()).ok() != Some(b"URI".as_slice()) {
                    continue;
                }
                let Some(raw) = action.get(b"URI").ok().and_then(|v| v.as_str().ok()) else {
                    continue;
                };
                let cleaned: String = String::from_utf8_lossy(raw)
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                let Ok(url) = url::Url::parse(&cleaned) else {
                    continue;
                };
                if !matches!(url.scheme(), "http" | "https") {
                    continue;
                }
                result.links.push(os_model::ExtractedLink {
                    link_type: os_model::LinkType::PdfLink,
                    href_raw: cleaned,
                    resolved: Some(url.to_string()),
                    pdf_page: Some(number),
                    doc_order: result.links.len() as u32,
                    ..Default::default()
                });
            }
        }
    }
    if cfg.crawl.pdf.store {
        let numbers: Vec<_> = pages.keys().copied().collect();
        match doc.extract_text(&numbers) {
            Ok(text) => {
                if text.trim().is_empty() {
                    result.error = Some("PDF enthält keinen Textlayer".into());
                }
                result.word_count =
                    Some(text.split_whitespace().count().min(u32::MAX as usize) as u32);
                if cfg.readability {
                    result.ext.extend(crate::readability::facts(&text));
                }
                result.ext.push((
                    ExtKey::from_static("content.text"),
                    Value::Text(text.clone()),
                ));
                result.text = Some(text);
            }
            Err(_) => result.error = Some("PDF-Textlayer konnte nicht gelesen werden".into()),
        }
    }
    result
}

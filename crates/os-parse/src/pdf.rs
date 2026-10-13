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

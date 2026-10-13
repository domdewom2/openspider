//! PDF-Dokumenteigenschaften (ONPAGE-166, Stufe 1): Titel und Keywords aus dem Info-Dictionary.
//!
//! Die PDFs werden im Test mit `lopdf` erzeugt (Nachbau der Fixtures `doc.pdf`, `untitled.pdf`, `scan.pdf`,
//! `encrypted.pdf` aus `onpage-pdf/`).

use lopdf::{dictionary, Document, Object, Stream, StringFormat};
use os_model::PdfMeta;
use os_parse::pdf::meta;
use os_parse::text::char_len;

fn base_doc(content: &str) -> Document {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.as_bytes().to_vec()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents" => content_id,
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    doc
}

fn literal(bytes: &[u8]) -> Object {
    Object::String(bytes.to_vec(), StringFormat::Literal)
}

fn to_bytes(mut doc: Document) -> Vec<u8> {
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

fn pdf_with_info(info: lopdf::Dictionary) -> Vec<u8> {
    let mut doc = base_doc("BT /F1 12 Tf 72 720 Td (Einleitung) Tj ET");
    let info_id = doc.add_object(info);
    doc.trailer.set("Info", info_id);
    to_bytes(doc)
}

// spec: ONPAGE-166
#[test]
fn title_and_keywords_come_from_the_info_dictionary() {
    // doc.pdf: Titel „Preisliste 2026“, Keywords „preise, liste“
    let pdf = pdf_with_info(dictionary! {
        "Title" => literal(b"Preisliste 2026"),
        "Keywords" => literal(b"preise, liste"),
        "Author" => literal(b"Jemand"),
    });
    let m = meta(&pdf);
    assert_eq!(m.title.as_deref(), Some("Preisliste 2026"));
    assert_eq!(m.keywords.as_deref(), Some("preise, liste"));
    assert_eq!(char_len(m.title.as_deref().unwrap()), 15);
}

// spec: ONPAGE-166
#[test]
fn missing_title_stays_empty_without_fallback() {
    // untitled.pdf: keine Titel-Eigenschaft, erste Textzeile „Einleitung“
    let pdf = pdf_with_info(dictionary! { "Author" => literal(b"Jemand") });
    assert_eq!(meta(&pdf), PdfMeta::default());
    // Gar kein Info-Dictionary.
    assert_eq!(meta(&to_bytes(base_doc("BT ET"))), PdfMeta::default());
    // Nur Keywords.
    let pdf = pdf_with_info(dictionary! { "Keywords" => literal(b"a, b") });
    assert_eq!(
        meta(&pdf),
        PdfMeta {
            title: None,
            keywords: Some("a, b".into())
        }
    );
}

// spec: ONPAGE-166
#[test]
fn strings_are_decoded_and_normalized() {
    // UTF-16BE mit BOM: „Größe & Maß“
    let mut utf16 = vec![0xFE, 0xFF];
    for u in "Größe & Maß".encode_utf16() {
        utf16.extend_from_slice(&u.to_be_bytes());
    }
    // PDFDocEncoding (Latin-1-kompatibel für Umlaute): „Über uns“
    let pdfdoc = b"\xDCber   uns\r\n".to_vec();
    let pdf = pdf_with_info(dictionary! {
        "Title" => Object::String(utf16, StringFormat::Hexadecimal),
        "Keywords" => Object::String(pdfdoc, StringFormat::Literal),
    });
    let m = meta(&pdf);
    assert_eq!(m.title.as_deref(), Some("Größe & Maß"));
    assert_eq!(m.keywords.as_deref(), Some("Über uns"));

    // UTF-8 mit BOM (PDF 2.0) und NBSP/Zeilenumbrüche im Titel.
    let mut utf8 = b"\xEF\xBB\xBF".to_vec();
    utf8.extend_from_slice("  Preise\u{00A0}\n 2026  ".as_bytes());
    let pdf = pdf_with_info(dictionary! { "Title" => Object::String(utf8, StringFormat::Literal) });
    assert_eq!(meta(&pdf).title.as_deref(), Some("Preise 2026"));
}

// spec: ONPAGE-166
#[test]
fn blank_values_are_none() {
    let pdf = pdf_with_info(dictionary! {
        "Title" => literal(b"   "),
        "Keywords" => literal(b""),
    });
    assert_eq!(meta(&pdf), PdfMeta::default());
}

// spec: ONPAGE-166
#[test]
fn non_string_title_is_ignored() {
    let pdf =
        pdf_with_info(dictionary! { "Title" => 42, "Keywords" => Object::Name(b"x".to_vec()) });
    assert_eq!(meta(&pdf), PdfMeta::default());
}

// spec: ONPAGE-166
#[test]
fn scan_without_text_layer_still_reads_properties() {
    // scan.pdf: nur Bild/leerer Inhalt; Eigenschaften werden unabhängig vom Textlayer gelesen.
    let mut doc = base_doc("");
    let info_id = doc.add_object(dictionary! { "Title" => literal(b"Scan 1") });
    doc.trailer.set("Info", info_id);
    assert_eq!(meta(&to_bytes(doc)).title.as_deref(), Some("Scan 1"));
}

// spec: ONPAGE-166
#[test]
fn broken_input_yields_empty_properties_without_panic() {
    assert_eq!(meta(b""), PdfMeta::default());
    assert_eq!(meta(b"%PDF"), PdfMeta::default());
    assert_eq!(meta(b"das ist keine pdf-datei"), PdfMeta::default());
    assert_eq!(meta(&[0xFF; 64]), PdfMeta::default());
    assert_eq!(meta(b"<html><title>x</title></html>"), PdfMeta::default());
    // Abgeschnittene PDF.
    let full = pdf_with_info(dictionary! { "Title" => literal(b"Titel") });
    for cut in [10, full.len() / 2, full.len() - 20] {
        let _ = meta(&full[..cut]); // darf nicht abstürzen; Ergebnis je nach Rekonstruktion leer oder Titel
    }
    // Zufällige Verstümmelung des Anfangs.
    let mut broken = full.clone();
    broken[..8].copy_from_slice(b"XXXXXXXX");
    let _ = meta(&broken);
}

// spec: ONPAGE-166
#[test]
fn password_protected_pdf_yields_empty_properties() {
    // encrypted.pdf (Leseschutz): Benutzerkennwort gesetzt, ohne Kennwort sind die Eigenschaften nicht lesbar.
    use lopdf::encryption::{EncryptionState, EncryptionVersion, Permissions};

    let mut doc = base_doc("BT ET");
    let info_id = doc.add_object(dictionary! { "Title" => literal(b"Geheim") });
    doc.trailer.set("Info", info_id);
    doc.trailer.set(
        "ID",
        Object::Array(vec![
            literal(b"0123456789abcdef"),
            literal(b"0123456789abcdef"),
        ]),
    );
    let version = EncryptionVersion::V2 {
        document: &doc,
        owner_password: "owner",
        user_password: "user",
        key_length: 128,
        permissions: Permissions::all(),
    };
    let state = EncryptionState::try_from(version).unwrap();
    doc.encrypt(&state).unwrap();
    let bytes = to_bytes(doc);
    assert_eq!(meta(&bytes), PdfMeta::default());
}

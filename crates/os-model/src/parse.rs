//! Ausgabe von `os-parse` (eine HTML-Antwort) – Modell, siehe M1-Plan 3.3.

use crate::enums::{
    DirectiveSource, HintSource, ImageSource, LinkPosition, LinkType, PathType, RefreshSource,
    SitemapKind,
};
use crate::ext::ExtKey;
use crate::parse_m2::{ExtractedValue, FormRef, SdFinding, SdItem, SearchHit, ValidationFacts};
use crate::value::Value;

/// Ein extrahierter Text, bereits mit N(x) normalisiert (ONPAGE-003).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedText {
    /// Normalisierter Text N(x).
    pub text: String,
    /// Länge in Codepoints.
    pub len: u32,
    /// Breite in Pixeln (nur wenn berechnet).
    pub px: Option<u32>,
    /// Liegt im `<head>` (TECH-003).
    pub in_head: bool,
}

/// Ein Link aus dem Dokument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedLink {
    pub crawlable: bool,
    pub uncrawlable_reason: Option<crate::UncrawlableReason>,
    pub pdf_page: Option<u32>,
    /// Linktyp.
    pub link_type: LinkType,
    /// Rohwert des Attributs.
    pub href_raw: String,
    /// Aufgelöste absolute URL, noch nicht normalisiert.
    pub resolved: Option<String>,
    /// Ankertext.
    pub anchor: Option<String>,
    /// Alt-Text (Bild im Link bzw. Bild-Linktyp).
    pub alt_text: Option<String>,
    /// `rel`: klein, in Originalreihenfolge, durch Leerzeichen getrennt.
    pub rel: Option<String>,
    /// `rel` enthält nofollow, ugc oder sponsored.
    pub nofollow_rel: bool,
    /// Wert des `target`-Attributs.
    pub target: Option<String>,
    /// Pfadart.
    pub path_type: PathType,
    /// Absoluter XPath des Elements.
    pub xpath: String,
    /// Position im Dokument.
    pub position: LinkPosition,
    /// Liegt im `<head>`.
    pub in_head: bool,
    /// Reihenfolge im Dokument.
    pub doc_order: u32,
}

/// Eine Bildinstanz.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageInstance {
    pub link_position: Option<LinkPosition>,
    pub origin: crate::LinkOrigin,
    pub render: Option<ImageRenderFacts>,
    /// Rohwert der Bildquelle.
    pub src_raw: String,
    /// Aufgelöste absolute URL.
    pub resolved: Option<String>,
    /// Quelle der Instanz.
    pub source: ImageSource,
    /// `alt`-Attribut vorhanden (auch leer).
    pub alt_present: bool,
    /// Alt-Text, normalisiert.
    pub alt: Option<String>,
    /// Länge des Alt-Textes in Codepoints.
    pub alt_len: Option<u32>,
    /// `width`-Attribut roh.
    pub width_attr: Option<String>,
    /// `height`-Attribut roh.
    pub height_attr: Option<String>,
    /// Absoluter XPath.
    pub xpath: String,
    /// Reihenfolge im Dokument.
    pub doc_order: u32,
}

/// Eine Canonical-Angabe.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CanonicalRef {
    pub invalid_attribute: bool,
    /// Herkunft (HTML oder HTTP-Header).
    pub source: HintSource,
    /// Rohwert.
    pub raw: String,
    /// Aufgelöste URL.
    pub resolved: Option<String>,
    /// Liegt im `<head>`.
    pub in_head: bool,
    /// Pfad des Elements im Dokument (`//head/link[3]` im Kopf, sonst absolut; `None` für Header-Quellen und
    /// Sitemaps), OUT-031/TECH-131.
    pub xpath: Option<String>,
    /// Reihenfolge.
    pub order: u32,
}

/// Eine Robots-Direktive (Meta oder X-Robots-Tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    /// Herkunft.
    pub source: DirectiveSource,
    /// Ziel-Agent; `None` = generisch.
    pub agent: Option<String>,
    /// Rohwert.
    pub raw: String,
    /// Normalisierte Tokens.
    pub tokens: Vec<String>,
    /// Gilt für den eigenen Crawler.
    pub applies: bool,
    /// Liegt im `<head>`.
    pub in_head: bool,
    /// Reihenfolge.
    pub order: u32,
}

/// Eine Refresh-Angabe (Meta oder Header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshRef {
    /// Herkunft.
    pub source: RefreshSource,
    /// Rohwert.
    pub raw: String,
    /// Verzögerung in Sekunden.
    pub delay: Option<u32>,
    /// Ziel roh.
    pub target_raw: Option<String>,
    /// Ziel aufgelöst.
    pub target: Option<String>,
    /// Liegt im `<head>`.
    pub in_head: bool,
    /// Pfad des Elements im Dokument (`//head/link[3]` im Kopf, sonst absolut; `None` für Header-Quellen und
    /// Sitemaps), OUT-031/TECH-131.
    pub xpath: Option<String>,
    /// Reihenfolge.
    pub order: u32,
}

/// Eine hreflang-Annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HreflangRef {
    /// Herkunft (HTML, HTTP, Sitemap).
    pub source: HintSource,
    /// Sprachcode roh.
    pub code_raw: String,
    /// Sprachcode normalisiert.
    pub code_norm: String,
    /// Sprachcode gültig.
    pub code_valid: bool,
    /// Grund der Ungültigkeit.
    pub invalid_reason: Option<String>,
    /// URL roh.
    pub href_raw: String,
    /// URL aufgelöst.
    pub resolved: Option<String>,
    /// Liegt im `<head>`.
    pub in_head: bool,
    /// Pfad des Elements im Dokument (`//head/link[3]` im Kopf, sonst absolut; `None` für Header-Quellen und
    /// Sitemaps), OUT-031/TECH-131.
    pub xpath: Option<String>,
    /// Reihenfolge.
    pub order: u32,
}

/// Eine Paginierungsangabe (`rel=next`/`prev`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaginationRef {
    /// `true` = next, `false` = prev.
    pub rel_next: bool,
    /// Herkunft.
    pub source: HintSource,
    /// Rohwert.
    pub raw: String,
    /// Aufgelöste URL.
    pub resolved: Option<String>,
    /// Liegt im `<head>`.
    pub in_head: bool,
    /// Pfad des Elements im Dokument (`//head/link[3]` im Kopf, sonst absolut; `None` für Header-Quellen und
    /// Sitemaps), OUT-031/TECH-131.
    pub xpath: Option<String>,
    /// Reihenfolge.
    pub order: u32,
}

/// Ergebnis der Auswertung einer HTML-Seite.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParsedPage {
    /// Basis-URL (nach `<base href>`).
    pub base_url: String,
    /// Erkannter Zeichensatz.
    pub charset: Option<String>,
    /// Alle Titel.
    pub titles: Vec<ExtractedText>,
    /// Alle Meta-Descriptions.
    pub meta_descriptions: Vec<ExtractedText>,
    /// Alle Meta-Keywords.
    pub meta_keywords: Vec<ExtractedText>,
    /// Alle H1 (vollständige Liste; Spalten 1/2 bildet die Engine).
    pub h1: Vec<ExtractedText>,
    /// Alle H2.
    pub h2: Vec<ExtractedText>,
    /// Links.
    pub links: Vec<ExtractedLink>,
    /// Bildinstanzen.
    pub images: Vec<ImageInstance>,
    /// Canonicals.
    pub canonicals: Vec<CanonicalRef>,
    /// Robots-Direktiven.
    pub directives: Vec<Directive>,
    /// Refresh-Angaben.
    pub refresh: Vec<RefreshRef>,
    /// hreflang-Annotationen.
    pub hreflang: Vec<HreflangRef>,
    /// Wegen Limit verworfene hreflang-Annotationen.
    pub hreflang_truncated: u32,
    /// Paginierung.
    pub pagination: Vec<PaginationRef>,
    /// Wortanzahl im Inhaltsbereich.
    pub word_count: Option<u32>,
    /// Textanteil.
    pub text_ratio: Option<f32>,
    /// Wirksames index (TECH-066).
    pub effective_index: bool,
    /// Wirksames follow (TECH-066).
    pub effective_follow: bool,
    /// Skalare Zusatzfakten (Schlüssel in den Namensräumen von [`crate::ext::NAMESPACES`], ab M2).
    pub ext: Vec<(ExtKey, Value)>,
    /// Strukturierte Daten (ab M2).
    pub structured_data: Vec<SdItem>,
    /// Befunde der Validierung strukturierter Daten (ab M2-D20, TECH-187/188; K2-61).
    pub sd_findings: Vec<SdFinding>,
    /// Formulare (ab M2).
    pub forms: Vec<FormRef>,
    /// Ergebnis der HTML-Validierung, wenn `advanced.html_validation` an ist (ab M2).
    pub validation: Option<ValidationFacts>,
    /// Treffer der Custom-Search-Filter (ab M2).
    pub custom_search: Vec<SearchHit>,
    /// Werte der Custom-Extractors (ab M2).
    pub custom_extraction: Vec<ExtractedValue>,
    /// MinHash-Signatur des Inhaltsbereichs (ab M2; Nahezu-Duplikate).
    pub minhash: Option<Vec<u32>>,
}

/// Angaben aus HTTP-Headern (auch für Nicht-HTML).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeaderHints {
    pub hreflang_truncated: u32,
    pub mobile_alternates: Vec<ExtractedLink>,
    /// Canonicals aus `Link`-Headern.
    pub canonicals: Vec<CanonicalRef>,
    /// `X-Robots-Tag`.
    pub directives: Vec<Directive>,
    /// `Refresh`-Header.
    pub refresh: Vec<RefreshRef>,
    /// hreflang aus `Link`-Headern.
    pub hreflang: Vec<HreflangRef>,
    /// Paginierung aus `Link`-Headern.
    pub pagination: Vec<PaginationRef>,
}

/// Eine ausgewertete Sitemap-Datei.
#[derive(Debug, Clone, PartialEq)]
pub struct SitemapDoc {
    /// Art der Datei.
    pub kind: SitemapKind,
    /// Einträge.
    pub locs: Vec<SitemapLoc>,
    /// Ungültige Einträge.
    pub invalid_entries: u32,
    /// Größe der (dekomprimierten) Datei.
    pub size_bytes: u64,
}

/// Ein Eintrag einer Sitemap.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SitemapLoc {
    pub hreflang_truncated: u32,
    pub changefreq: Option<String>,
    pub priority: Option<f64>,
    pub extensions: Vec<SitemapExtension>,
    /// `<loc>` roh.
    pub loc_raw: String,
    /// Aufgelöste URL.
    pub url: Option<String>,
    /// `<lastmod>` roh.
    pub lastmod: Option<String>,
    /// hreflang-Alternativen des Eintrags.
    pub hreflang: Vec<HreflangRef>,
}

/// Dokumenteigenschaften einer PDF-Datei (ONPAGE-166).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PdfMeta {
    /// Titel.
    pub title: Option<String>,
    /// Schlüsselwörter.
    pub keywords: Option<String>,
}

/// Bildabmessungen aus Header und Rendering (ONPAGE-172).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageRenderFacts {
    pub real_w: Option<u32>,
    pub real_h: Option<u32>,
    pub display_w: Option<u32>,
    pub display_h: Option<u32>,
    pub in_viewport: Option<bool>,
    pub bytes: Option<u64>,
}
/// Verlustfrei gespeicherte Sitemap-Erweiterung (CRAWL-060).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitemapExtension {
    pub kind: crate::SitemapExtKind,
    pub json: String,
}
/// PDF-Fakten und Verweise (ONPAGE-166).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PdfResult {
    pub meta: PdfMeta,
    pub links: Vec<ExtractedLink>,
    pub word_count: Option<u32>,
    pub text: Option<String>,
    pub ext: Vec<(ExtKey, Value)>,
    pub error: Option<String>,
}
impl Default for ExtractedLink {
    fn default() -> Self {
        Self {
            link_type: Default::default(),
            href_raw: String::new(),
            resolved: None,
            anchor: None,
            alt_text: None,
            rel: None,
            nofollow_rel: false,
            target: None,
            path_type: Default::default(),
            xpath: String::new(),
            position: Default::default(),
            in_head: false,
            doc_order: 0,
            crawlable: true,
            uncrawlable_reason: None,
            pdf_page: None,
        }
    }
}

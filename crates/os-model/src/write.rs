//! Schreibmodell Engine → Store (eine Transaktion je [`WriteBatch`]), siehe M1-Plan 3.3.

use crate::enums::{
    ContentClass, DiscoveryKind, Indexability, IndexabilityStatus, LinkOrigin, LinkPosition,
    LinkType, NoResponseReason, NotCrawledReason, PathType, RedirectType, SitemapKind, UrlState,
};
use crate::ext::ExtWrite;
use crate::ids::{SitemapId, UrlId};
use crate::parse::{
    CanonicalRef, Directive, ExtractedText, HreflangRef, ImageInstance, PaginationRef, RefreshRef,
};
use crate::time::Timestamp;
use crate::write_m2::{
    A11yWrite, ApiValueWrite, CustomExtractionWrite, CustomJsWrite, CustomSearchWrite, FormWrite,
    RenderWrite, SdIssueWrite, SdItemWrite, SpellingWrite,
};

/// Alles, was die Engine in einer Transaktion schreibt.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WriteBatch {
    /// Neu entdeckte URLs.
    pub new_urls: Vec<NewUrl>,
    /// Änderungen an vorhandenen URLs.
    pub url_updates: Vec<UrlUpdate>,
    /// Abgerufene Seiten.
    pub pages: Vec<PageWrite>,
    /// Queue-Operationen.
    pub queue: Vec<QueueOp>,
    /// Sitemaps.
    pub sitemaps: Vec<SitemapWrite>,
    /// robots.txt-Dateien.
    pub robots: Vec<RobotsWrite>,
    /// Meta-Schlüssel (Upsert).
    pub meta: Vec<(String, String)>,
    /// Ereignisse (Log).
    pub events: Vec<CrawlEvent>,
    /// Zähler-Deltas.
    pub counters: Vec<(String, i64)>,
    /// URL-Flags `(URL, Flag, Wert)` (`url_flags`, z. B. `redirect_loop`, `redirect_limit_reached`); der Writer
    /// schreibt sie in derselben Transaktion und stempelt `change_seq` der URL (M1.md 3.11).
    pub url_flags: Vec<(UrlId, String, Option<String>)>,
    /// Skalare Zusatzfakten je URL → `url_ext` (ab M2, Namensräume laut [`crate::ext::NAMESPACES`]).
    pub ext: Vec<ExtWrite>,
    /// Strukturierte Datenelemente → `sd_items` (ab M2).
    pub structured_data: Vec<SdItemWrite>,
    /// Befunde der strukturierten Daten → `sd_issues` (ab M2).
    pub sd_issues: Vec<SdIssueWrite>,
    /// Gerenderte Seiten: HTML, Konsole, blockierte Ressourcen, Netzwerk, Screenshot-Kacheln (ab M2).
    pub render: Vec<RenderWrite>,
    /// Barrierefreiheit (axe-core) → `a11y_rules`, `a11y_nodes` (ab M2).
    pub a11y: Vec<A11yWrite>,
    /// Rechtschreib- und Grammatikbefunde → `spelling_findings` (ab M2).
    pub spelling: Vec<SpellingWrite>,
    /// Custom Search → `custom_search_hits` (ab M2).
    pub custom_search: Vec<CustomSearchWrite>,
    /// Custom Extraction → `custom_extraction_values` (ab M2).
    pub custom_extraction: Vec<CustomExtractionWrite>,
    /// Custom JavaScript → `custom_js_values` (ab M2).
    pub custom_js: Vec<CustomJsWrite>,
    /// Formulare → `forms` (ab M2).
    pub forms: Vec<FormWrite>,
    /// MinHash-Signaturen → `minhash` (ab M2).
    pub minhash: Vec<(UrlId, Vec<u32>)>,
    /// Werte der API-Quellen → `api_values`; nur vom Integrationspfad (ab M2).
    pub api_values: Vec<ApiValueWrite>,
}

/// Eine neue URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUrl {
    /// Vergebene Kennung.
    pub id: UrlId,
    /// URL Encoded Address (Schlüssel).
    pub url: String,
    /// Anzeigeform.
    pub address: String,
    /// `scheme://host[:port]`.
    pub origin: String,
    /// Intern (nach Bereichsregeln).
    pub is_internal: bool,
    /// Erscheint in den Tabs.
    pub in_tables: bool,
    /// Wie entdeckt.
    pub discovered_via: DiscoveryKind,
    /// Crawl-Tiefe.
    pub crawl_depth: Option<u32>,
    /// Ordnertiefe.
    pub folder_depth: u32,
    /// URL-Länge.
    pub url_length: u32,
    /// Zustand.
    pub state: UrlState,
    /// Grund, falls nicht gecrawlt.
    pub not_crawled_reason: Option<NotCrawledReason>,
}

/// Änderung an einer vorhandenen URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlUpdate {
    /// Neue (geringere) Crawl-Tiefe.
    Depth {
        /// URL.
        id: UrlId,
        /// Tiefe.
        depth: u32,
    },
    /// Neuer Zustand.
    State {
        /// URL.
        id: UrlId,
        /// Zustand.
        state: UrlState,
        /// Grund.
        reason: Option<NotCrawledReason>,
    },
    /// Sichtbarkeit in den Tabs.
    InTables {
        /// URL.
        id: UrlId,
        /// Wert.
        value: bool,
    },
}

/// Fakten des Abrufs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FetchFacts {
    /// Zeitpunkt des Abrufs.
    pub crawl_timestamp: Timestamp,
    /// Statuscode; 0 = keine Antwort.
    pub status_code: u16,
    /// Statustext.
    pub status: String,
    /// Grund bei fehlender Antwort.
    pub no_response_reason: Option<NoResponseReason>,
    /// `Content-Type`.
    pub content_type: Option<String>,
    /// Inhaltsklasse.
    pub content_class: ContentClass,
    /// HTTP-Version.
    pub http_version: Option<String>,
    /// Größe (dekomprimiert).
    pub size_bytes: u64,
    /// Übertragene Bytes (Body).
    pub transferred_bytes: u64,
    /// Übertragene Bytes insgesamt.
    pub total_transferred_bytes: u64,
    /// Antwortzeit in ms.
    pub response_time_ms: Option<u32>,
    /// `Last-Modified` roh.
    pub last_modified_raw: Option<String>,
    /// `Last-Modified` geparst.
    pub last_modified: Option<Timestamp>,
    /// Weiterleitungsziel.
    pub redirect_url: Option<String>,
    /// Weiterleitungsziel als URL-ID.
    pub redirect_target: Option<UrlId>,
    /// Art der Weiterleitung.
    pub redirect_type: Option<RedirectType>,
    /// Verzögerung des Meta-Refresh.
    pub meta_refresh_delay: Option<u32>,
    /// MD5 des Bodys.
    pub hash_md5: Option<String>,
    /// Zeichensatz.
    pub charset: Option<String>,
    /// Durch robots.txt gesperrt.
    pub robots_blocked: bool,
    /// Treffende robots.txt-Zeile.
    pub robots_matched_line: Option<String>,
    /// robots.txt-URL.
    pub robots_txt_url: Option<String>,
}

/// Erstes und zweites Vorkommen eines Elements plus Anzahl.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ElementPair {
    /// Anzahl der Vorkommen.
    pub count: u32,
    /// Erstes Vorkommen.
    pub first: Option<ExtractedText>,
    /// Zweites Vorkommen.
    pub second: Option<ExtractedText>,
}

/// Seitenfakten aus dem Parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct PageFacts {
    /// Titel.
    pub title: ElementPair,
    /// Meta-Description.
    pub meta_description: ElementPair,
    /// Meta-Keywords.
    pub meta_keywords: ElementPair,
    /// H1.
    pub h1: ElementPair,
    /// H2.
    pub h2: ElementPair,
    /// Wortanzahl.
    pub word_count: Option<u32>,
    /// Textanteil.
    pub text_ratio: Option<f32>,
    /// Anzahl Canonicals.
    pub canonical_count: u32,
    /// Maßgebliche Canonical-URL (TECH-017).
    pub canonical_url: Option<String>,
    /// Canonical-Ziel als URL-ID.
    pub canonical_target: Option<UrlId>,
    /// Canonical zeigt auf sich selbst.
    pub canonical_self: Option<bool>,
    /// Wirksames index.
    pub effective_index: bool,
    /// Wirksames follow.
    pub effective_follow: bool,
    /// Hat `rel=next`.
    pub has_rel_next: bool,
    /// Hat `rel=prev`.
    pub has_rel_prev: bool,
    /// Anzahl hreflang.
    pub hreflang_count: u32,
}

/// Eine abgerufene URL samt Kindzeilen.
#[derive(Debug, Clone, PartialEq)]
pub struct PageWrite {
    /// URL.
    pub url_id: UrlId,
    /// Alte Kinder löschen und Zähler korrigieren (CRAWL-131).
    pub replace: bool,
    /// Abruf.
    pub fetch: FetchFacts,
    /// Seitenfakten (nur HTML).
    pub page: Option<PageFacts>,
    /// Indexierbarkeit.
    pub indexability: Indexability,
    /// Begründung.
    pub indexability_status: Option<IndexabilityStatus>,
    /// Links.
    pub links: Vec<LinkRow>,
    /// Bilder.
    pub images: Vec<ImageRow>,
    /// Canonicals.
    pub canonicals: Vec<CanonicalRow>,
    /// Direktiven.
    pub directives: Vec<DirectiveRow>,
    /// Refresh-Angaben.
    pub refresh: Vec<RefreshRow>,
    /// hreflang.
    pub hreflang: Vec<HreflangRow>,
    /// Paginierung.
    pub pagination: Vec<PaginationRow>,
    /// HTTP-Header.
    pub headers: Vec<HeaderRow>,
    /// Gespeicherter Quelltext (`page_html`, CRAWL-063, ab M2); `None` = nicht speichern bzw. unverändert lassen.
    /// Nachtrag M2-F02 (`docs/plan/M2.md` 3.3a); befüllt von M2-E01.
    pub html: Option<PageHtml>,
}

/// Der gespeicherte Quelltext einer Seite (`page_html`, CRAWL-063).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageHtml {
    /// Zeichensatz, mit dem `body` zu lesen ist (z. B. `utf-8`).
    pub encoding: Option<String>,
    /// Der Quelltext (dekomprimiert, nicht umkodiert).
    pub body: Vec<u8>,
}

/// Eine Linkzeile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRow {
    /// Ziel (falls bekannt).
    pub target_id: Option<UrlId>,
    /// Rohziel.
    pub target_raw: String,
    /// Linktyp.
    pub link_type: LinkType,
    /// Position innerhalb der Seite.
    pub position: u32,
    /// Ankertext.
    pub anchor: Option<String>,
    /// Alt-Text.
    pub alt_text: Option<String>,
    /// Follow.
    pub follow: bool,
    /// `rel`.
    pub rel: Option<String>,
    /// `target`-Attribut.
    pub target_attr: Option<String>,
    /// Pfadart.
    pub path_type: Option<PathType>,
    /// XPath.
    pub link_path: Option<String>,
    /// Position im Dokument.
    pub link_position: Option<LinkPosition>,
    /// Herkunft.
    pub link_origin: LinkOrigin,
    /// Liegt im `<head>`.
    pub in_head: Option<bool>,
}

/// Eine Bildzeile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRow {
    /// Bild-URL (falls bekannt).
    pub image_id: Option<UrlId>,
    /// Instanz.
    pub instance: ImageInstance,
}

/// Eine Canonical-Zeile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalRow {
    /// Ziel.
    pub target_id: Option<UrlId>,
    /// Angabe.
    pub r: CanonicalRef,
}

/// Eine Direktiven-Zeile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveRow(pub Directive);

/// Eine Refresh-Zeile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshRow {
    /// Ziel.
    pub target_id: Option<UrlId>,
    /// Angabe.
    pub r: RefreshRef,
}

/// Eine hreflang-Zeile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HreflangRow {
    /// Ziel.
    pub target_id: Option<UrlId>,
    /// Sitemap, falls aus einer Sitemap.
    pub sitemap_id: Option<SitemapId>,
    /// Angabe.
    pub r: HreflangRef,
}

/// Eine Paginierungs-Zeile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaginationRow {
    /// Ziel.
    pub target_id: Option<UrlId>,
    /// Angabe.
    pub r: PaginationRef,
}

/// Eine HTTP-Header-Zeile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderRow {
    /// `true` = Request-Header, `false` = Response-Header.
    pub request: bool,
    /// Reihenfolge.
    pub position: u32,
    /// Name.
    pub name: String,
    /// Wert, bereits redigiert (OUT-243).
    pub value: String,
}

/// Operation auf der Crawl-Queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueOp {
    /// URL einreihen.
    Push {
        /// URL.
        id: UrlId,
        /// Laufende Nummer.
        seq: u64,
        /// Tiefe.
        depth: Option<u32>,
        /// 0 = Link, 1 = nur Sitemap.
        priority: u8,
    },
    /// URL aus der Queue nehmen.
    Pop {
        /// URL.
        id: UrlId,
    },
}

/// Eine Sitemap-Datei samt Einträgen.
#[derive(Debug, Clone, PartialEq)]
pub struct SitemapWrite {
    /// Kennung.
    pub id: SitemapId,
    /// URL der Sitemap.
    pub url: String,
    /// URL-Zeile der Sitemap, falls vorhanden.
    pub url_id: Option<UrlId>,
    /// Eltern-Index.
    pub parent: Option<SitemapId>,
    /// Art.
    pub kind: SitemapKind,
    /// Anzahl Einträge.
    pub entries: u32,
    /// Ungültige Einträge.
    pub invalid_entries: u32,
    /// Größe.
    pub size_bytes: u64,
    /// Statuscode des Abrufs.
    pub status_code: Option<u16>,
    /// Tiefe in der Index-Hierarchie.
    pub depth: u32,
    /// Fundort (`start`, `list`, `config`, `robots`, …).
    pub found_via: String,
    /// Einträge: (URL, `loc` roh, `lastmod`).
    pub entry_urls: Vec<(UrlId, String, Option<String>)>,
    /// hreflang-Angaben aus den `<url>`-Blöcken, zugeordnet der `loc`-URL. Der Writer speichert sie mit
    /// `source = 'sitemap'`, `sitemap_id` = Kennung dieser Sitemap und `position` = `r.order` (M1.md 3.11).
    pub hreflang: Vec<(UrlId, HreflangRow)>,
    /// Fehlervermerk bei ungültigem XML (CRAWL-060, K-27): alle bis zum Fehler gelesenen Einträge gelten, der Fehler
    /// steht an der Sitemap (`sitemaps.error`). Nachtrag M2-F02 (3.3a); befüllt von M2-E02.
    pub error: Option<String>,
    /// Zusatzangaben zu Einträgen (`sitemap_entries.changefreq` … `news`, CRAWL-060-Rest). Je URL höchstens ein Eintrag;
    /// URLs ohne Eintrag behalten `NULL`. Nachtrag M2-F02 (3.3a); befüllt von M2-E02/M2-D13.
    pub entry_meta: Vec<SitemapEntryMeta>,
}

/// Zusatzangaben eines Sitemap-Eintrags (CRAWL-060: `changefreq`, `priority`, Bild-/Video-/News-Erweiterungen).
#[derive(Debug, Clone, PartialEq)]
pub struct SitemapEntryMeta {
    /// URL des Eintrags (muss in `SitemapWrite.entry_urls` stehen).
    pub url: UrlId,
    /// `<changefreq>` roh, klein geschrieben.
    pub changefreq: Option<String>,
    /// `<priority>`.
    pub priority: Option<f64>,
    /// Anzahl `image:image`-Erweiterungen.
    pub images: u32,
    /// Anzahl `video:video`-Erweiterungen.
    pub videos: u32,
    /// Anzahl `news:news`-Erweiterungen.
    pub news: u32,
}

/// Eine robots.txt-Datei.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotsWrite {
    /// Origin.
    pub origin: String,
    /// URL der robots.txt.
    pub robots_url: String,
    /// `ok` | `not_found` | `unreachable` | `ignored`.
    pub fetch_status: String,
    /// Statuscode.
    pub status_code: Option<u16>,
    /// Anzahl Versuche.
    pub attempts: u32,
    /// Inhalt (höchstens 500 KiB).
    pub body: Option<String>,
    /// Hinweis.
    pub note: Option<String>,
}

/// Ein Eintrag im Crawl-Log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawlEvent {
    /// Zeitpunkt.
    pub at: Timestamp,
    /// `warn` | `info`.
    pub kind: String,
    /// Maschinenlesbarer Code.
    pub code: String,
    /// Text.
    pub message: String,
}

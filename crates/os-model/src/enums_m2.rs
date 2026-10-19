//! Enums der Konfiguration v2, der App-Einstellungen und des Schreibmodells ab M2 (M2-F01).
//!
//! Gleiche Regeln wie in [`crate::enums`]: `as_str()` = TOML-/DB-Wert, `FromStr`, `Display`, `ALL`, Serde und
//! JSON-Schema über genau diese Zeichenketten.

use crate::enums::string_enum;

string_enum! {
    /// Speicherstufe des Ressourcenschutzes (CRAWL-129): ab 80 % der RAM-Obergrenze `Warning`, ab 90 % `Paused`.
    #[derive(Default)]
    pub enum MemoryStage {
        #[default]
        Normal => "normal",
        Warning => "warning",
        Paused => "paused",
    }
}

string_enum! {
    /// Bildschirmformat der gerenderten Screenshots (CRAWL-082).
    #[derive(Default)]
    pub enum ScreenshotFormat {
        #[default]
        Png => "png",
        Jpeg => "jpeg",
    }
}

string_enum! {
    /// HTTP-Version (CRAWL-024, P2-Erweiterung): ohne Schalter immer `Auto` (ALPN).
    #[derive(Default)]
    pub enum HttpVersion {
        #[default]
        Auto => "auto",
        Http1 => "http1",
        Http2Preferred => "http2_preferred",
    }
}

string_enum! {
    /// Suchbereich eines Custom-Search-Filters (INTEG-021 ff.).
    #[derive(Default)]
    pub enum SearchScope {
        #[default]
        Html => "html",
        PageText => "page_text",
        PageTextNoAnchors => "page_text_no_anchors",
        HtmlHead => "html_head",
        HtmlBody => "html_body",
        XPath => "xpath",
        ContentArea => "content_area",
    }
}

string_enum! {
    /// Abfragetyp eines Custom-Search-Filters (INTEG-020).
    #[derive(Default)]
    pub enum SearchKind {
        #[default]
        Text => "text",
        Regex => "regex",
    }
}

string_enum! {
    /// Modus eines Extraktors (INTEG-040).
    #[derive(Default)]
    pub enum ExtractorMode {
        #[default]
        XPath => "xpath",
        Css => "css",
        Regex => "regex",
    }
}

string_enum! {
    /// Ausgabeart eines XPath-/CSS-Extraktors (INTEG-041 ff.).
    #[derive(Default)]
    pub enum ExtractorOutput {
        #[default]
        Text => "text",
        InnerHtml => "inner_html",
        HtmlElement => "html_element",
        FunctionValue => "function_value",
    }
}

string_enum! {
    /// Art eines JavaScript-Snippets (INTEG-060).
    #[derive(Default)]
    pub enum JsSnippetKind {
        #[default]
        Extraction => "extraction",
        Action => "action",
    }
}

string_enum! {
    /// Modus der Rechtschreibprüfung (ONPAGE-153).
    #[derive(Default)]
    pub enum SpellingMode {
        #[default]
        Auto => "auto",
        Manual => "manual",
    }
}

string_enum! {
    /// Die 14 Elemente der Change Detection in Filterreihenfolge (TECH-300).
    pub enum ChangeElement {
        Indexability => "indexability",
        PageTitles => "page_titles",
        MetaDescription => "meta_description",
        H1 => "h1",
        WordCount => "word_count",
        CrawlDepth => "crawl_depth",
        Inlinks => "inlinks",
        UniqueInlinks => "unique_inlinks",
        InternalOutlinks => "internal_outlinks",
        UniqueInternalOutlinks => "unique_internal_outlinks",
        ExternalOutlinks => "external_outlinks",
        UniqueExternalOutlinks => "unique_external_outlinks",
        StructuredDataUniqueTypes => "structured_data_unique_types",
        Content => "content",
    }
}

string_enum! {
    /// Quelle von `lastmod` im Sitemap-Generator (OUT-123).
    #[derive(Default)]
    pub enum LastmodSource {
        #[default]
        Server => "server",
        Custom => "custom",
    }
}

string_enum! {
    /// Basis von `changefreq` im Sitemap-Generator (OUT-125).
    #[derive(Default)]
    pub enum ChangeFreqBasis {
        #[default]
        LastModified => "last_modified",
        Depth => "depth",
    }
}

string_enum! {
    /// Wert von `changefreq` (sitemaps.org).
    pub enum ChangeFreq {
        Always => "always",
        Hourly => "hourly",
        Daily => "daily",
        Weekly => "weekly",
        Monthly => "monthly",
        Yearly => "yearly",
        Never => "never",
    }
}

string_enum! {
    /// Datenquellen der API-Integration (04 A – J); Kennung in `api.<quelle>`, `api_values.source` und `SecretStore`.
    pub enum ApiKind {
        Ga4 => "ga4",
        Gsc => "gsc",
        Psi => "psi",
        Majestic => "majestic",
        Ahrefs => "ahrefs",
        Moz => "moz",
        Drive => "drive",
    }
}

string_enum! {
    /// Ebene von Linkmetriken (INTEG-0xx Majestic/Ahrefs/Moz).
    #[derive(Default)]
    pub enum ApiLevel {
        #[default]
        Url => "url",
        UrlBoth => "url_both",
        Subdomain => "subdomain",
        Domain => "domain",
    }
}

string_enum! {
    /// Zeitraum einer GA4-/GSC-Abfrage.
    #[derive(Default)]
    pub enum DateRangePreset {
        Last7Days => "last_7_days",
        Last28Days => "last_28_days",
        #[default]
        Last30Days => "last_30_days",
        Last90Days => "last_90_days",
        Custom => "custom",
    }
}

string_enum! {
    /// Suchtyp der Search Console.
    #[derive(Default)]
    pub enum SearchType {
        #[default]
        Web => "web",
        Image => "image",
        Video => "video",
        News => "news",
        Discover => "discover",
        GoogleNews => "google_news",
    }
}

string_enum! {
    /// Gerätetyp von PageSpeed Insights.
    #[derive(Default)]
    pub enum PsiDevice {
        #[default]
        Mobile => "mobile",
        Desktop => "desktop",
    }
}

string_enum! {
    /// Sprache der Oberfläche (UI-121): `System` folgt der Systemsprache.
    #[derive(Default)]
    pub enum UiLanguage {
        #[default]
        System => "system",
        De => "de",
        En => "en",
    }
}

string_enum! {
    /// Theme (UI-120).
    #[derive(Default)]
    pub enum ThemeMode {
        #[default]
        System => "system",
        Light => "light",
        Dark => "dark",
    }
}

string_enum! {
    /// Zeilenhöhe der Datenansichten (UI-120): kompakt 22 px (Standard), normal 26 px bei 100 % Zoom.
    #[derive(Default)]
    pub enum RowDensity {
        #[default]
        Compact => "compact",
        Normal => "normal",
    }
}

string_enum! {
    /// Cache-Leistungsprofil (CRAWL-124).
    #[derive(Default)]
    pub enum PerformanceProfile {
        #[default]
        Balanced => "balanced",
        MaxSpeed => "max_speed",
        MinMemory => "min_memory",
    }
}

string_enum! {
    /// Aufbewahrung von Crawls (OUT-229).
    #[derive(Default)]
    pub enum RetentionPolicy {
        #[default]
        Never => "never",
        OneWeek => "one_week",
        OneMonth => "one_month",
        ThreeMonths => "three_months",
        SixMonths => "six_months",
        OneYear => "one_year",
        Custom => "custom",
    }
}

string_enum! {
    /// Proxy-Modus (CRAWL-122, OUT-269).
    #[derive(Default)]
    pub enum ProxyMode {
        #[default]
        System => "system",
        None => "none",
        Manual => "manual",
    }
}

string_enum! {
    /// Proxy-Typ (CRAWL-122).
    #[derive(Default)]
    pub enum ProxyKind {
        #[default]
        Http => "http",
        Https => "https",
        Socks5 => "socks5",
    }
}

string_enum! {
    /// Standardformat der Exporte (OUT-002, OUT-222).
    #[derive(Default)]
    pub enum ExportFormat {
        #[default]
        Csv => "csv",
        Xlsx => "xlsx",
    }
}

string_enum! {
    /// Verschlüsselung eines SMTP-Kontos (OUT-280).
    #[derive(Default)]
    pub enum MailTls {
        #[default]
        StartTls => "starttls",
        Tls => "tls",
        None => "none",
    }
}

string_enum! {
    /// Operator einer Segmentbedingung (CRAWL-134).
    #[derive(Default)]
    pub enum SegmentOp {
        #[default]
        Contains => "contains",
        NotContains => "not_contains",
        Equals => "equals",
        NotEquals => "not_equals",
        StartsWith => "starts_with",
        EndsWith => "ends_with",
        RegexMatches => "regex_matches",
        RegexNotMatches => "regex_not_matches",
        GreaterThan => "greater_than",
        LessThan => "less_than",
        GreaterOrEqual => "greater_or_equal",
        LessOrEqual => "less_or_equal",
        IsEmpty => "is_empty",
        IsNotEmpty => "is_not_empty",
        HasIssue => "has_issue",
        HasNoIssue => "has_no_issue",
        InSegment => "in_segment",
        InNoSegment => "in_no_segment",
    }
}

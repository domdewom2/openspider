//! Enums des gemeinsamen Modells.
//!
//! Jedes Enum (außer [`FilterState`]) hat `as_str()` (= DB-Wert = kanonischer Exportwert), `FromStr`,
//! `Display`, `ALL` sowie Serde-/JSON-Schema-Unterstützung über genau diese Zeichenketten.
//! Wo die Spec einen Anzeige-/Exportwert festlegt, ist dieser der `as_str()`-Wert (z. B. `ContentClass`,
//! `LinkType`, `IndexabilityStatus`); alle übrigen Werte sind `snake_case` (auch als TOML-Wert der Konfiguration).

/// Erzeugt ein Enum mit `as_str`, `FromStr`, `Display`, Serde (als String) und `JsonSchema`.
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $text:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        $vis enum $name {
            $( $(#[$vmeta])* $variant ),+
        }

        impl $name {
            /// Alle Werte in Deklarationsreihenfolge.
            pub const ALL: &'static [$name] = &[ $( $name::$variant ),+ ];

            /// DB-Wert und kanonischer Exportwert.
            pub fn as_str(&self) -> &'static str {
                match self { $( $name::$variant => $text ),+ }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $crate::error::ModelError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $( $text => Ok($name::$variant), )+
                    _ => Err($crate::error::ModelError::UnknownValue {
                        kind: stringify!($name),
                        value: s.to_string(),
                    }),
                }
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = <::std::borrow::Cow<'de, str> as ::serde::Deserialize>::deserialize(d)?;
                <$name as ::std::str::FromStr>::from_str(&s).map_err(|_| {
                    ::serde::de::Error::unknown_variant(&s, &[ $( $text ),+ ])
                })
            }
        }

        impl ::schemars::JsonSchema for $name {
            fn schema_name() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed(stringify!($name))
            }

            fn json_schema(_: &mut ::schemars::SchemaGenerator) -> ::schemars::Schema {
                ::schemars::json_schema!({
                    "type": "string",
                    "enum": [ $( $text ),+ ]
                })
            }
        }
    };
}
pub(crate) use string_enum;

string_enum! {
    /// Inhaltsklasse einer URL (ONPAGE-020).
    #[derive(Default)]
    pub enum ContentClass {
        Html => "HTML",
        JavaScript => "JavaScript",
        Css => "CSS",
        Images => "Images",
        Pdf => "PDF",
        Plugins => "Plugins",
        Media => "Media",
        Fonts => "Fonts",
        Xml => "XML",
        Other => "Other",
        #[default]
        Unknown => "Unknown",
    }
}

string_enum! {
    /// Grund, warum keine HTTP-Antwort vorliegt (Statuscode 0). `as_str()` ist der DB-Wert;
    /// der Anzeigetext steht in [`NoResponseReason::status_text`] (nicht eindeutig, K-04).
    pub enum NoResponseReason {
        DnsLookupFailed => "dns_lookup_failed",
        ConnectionRefused => "connection_refused",
        ConnectionTimeout => "connection_timeout",
        ResponseTimeout => "response_timeout",
        ConnectionError => "connection_error",
        TlsError => "tls_error",
        Malformed => "malformed",
        BlockedByRobots => "blocked_by_robots",
        BlockedByRobotsUnreachable => "blocked_by_robots_unreachable",
        TooManyRedirects => "too_many_redirects",
    }
}

impl NoResponseReason {
    /// Statustext in Tabellen und Exporten.
    pub fn status_text(&self) -> &'static str {
        match self {
            NoResponseReason::DnsLookupFailed => "DNS Lookup Failed",
            NoResponseReason::ConnectionRefused => "Connection Refused",
            NoResponseReason::ConnectionTimeout => "Connection Timeout",
            // K-04: Antwort-Timeout erscheint ebenfalls als „Connection Timeout“.
            NoResponseReason::ResponseTimeout => "Connection Timeout",
            NoResponseReason::ConnectionError => "Connection Error",
            NoResponseReason::TlsError => "TLS Error",
            NoResponseReason::Malformed => "Malformed",
            NoResponseReason::BlockedByRobots => "Blocked by Robots.txt",
            NoResponseReason::BlockedByRobotsUnreachable => {
                "Blocked by Robots.txt (robots.txt unreachable)"
            }
            NoResponseReason::TooManyRedirects => "Too Many Redirects",
        }
    }
}

string_enum! {
    /// Art einer Weiterleitung.
    pub enum RedirectType {
        Http => "http",
        HstsPolicy => "hsts_policy",
        JavaScript => "javascript",
        MetaRefresh => "meta_refresh",
        HttpRefresh => "http_refresh",
    }
}

impl RedirectType {
    /// Bezeichnung in den Tabs (ONPAGE-018).
    pub fn tab_label(&self) -> &'static str {
        match self {
            RedirectType::Http => "HTTP Redirect",
            RedirectType::HstsPolicy => "HSTS Policy",
            RedirectType::JavaScript => "JavaScript Redirect",
            RedirectType::MetaRefresh => "Meta Refresh Redirect",
            RedirectType::HttpRefresh => "HTTP Refresh Redirect",
        }
    }

    /// Bezeichnung in den Reports (OUT-081, K-03): wie [`tab_label`](Self::tab_label), aber „HTTP Refresh“.
    pub fn report_label(&self) -> &'static str {
        match self {
            RedirectType::HttpRefresh => "HTTP Refresh",
            other => other.tab_label(),
        }
    }
}

string_enum! {
    /// Linktyp (OUT-031). `as_str()` ist der Wert der Spalte `Type`.
    pub enum LinkType {
        Hyperlink => "Hyperlink",
        Image => "Image",
        Css => "CSS",
        JavaScript => "JavaScript",
        Media => "Media",
        Iframe => "Iframe",
        Canonical => "Canonical",
        HttpCanonical => "HTTP Canonical",
        RelNext => "Rel Next",
        RelPrev => "Rel Prev",
        Hreflang => "Hreflang",
        HttpHreflang => "HTTP Hreflang",
        Amp => "AMP",
        MobileAlternate => "Mobile Alternate",
        MetaRefresh => "Meta Refresh",
        HttpRefresh => "HTTP Refresh",
        HttpRedirect => "HTTP Redirect",
        HstsPolicy => "HSTS Policy",
        JavaScriptRedirect => "JavaScript Redirect",
        XmlSitemap => "XML Sitemap",
        PdfLink => "PDF Link",
        /// `object`/`embed`/`preload` u. ä. ohne eigenen Typ.
        Other => "Other",
    }
}

impl LinkType {
    /// Navigations-Hyperlink (zählt für `inlinks`/`outlinks`, ONPAGE-015).
    pub fn is_hyperlink(&self) -> bool {
        matches!(self, LinkType::Hyperlink)
    }

    /// Eingebundene Ressource (Bild, CSS, JavaScript, Medien, Iframe); zählt nur in `inlinks_all`.
    pub fn is_resource(&self) -> bool {
        matches!(
            self,
            LinkType::Image
                | LinkType::Css
                | LinkType::JavaScript
                | LinkType::Media
                | LinkType::Iframe
        )
    }
}

string_enum! {
    /// Pfadart eines Links (OUT-031).
    pub enum PathType {
        Absolute => "Absolute",
        ProtocolRelative => "Protocol-Relative",
        RootRelative => "Root-Relative",
        PathRelative => "Path-Relative",
    }
}

string_enum! {
    /// Position eines Links im Dokument (OUT-031); `Head` für Elemente im `<head>` (TECH-131, K-63).
    pub enum LinkPosition {
        Navigation => "Navigation",
        Header => "Header",
        Content => "Content",
        Sidebar => "Sidebar",
        Aside => "Aside",
        Footer => "Footer",
        Head => "Head",
    }
}

string_enum! {
    /// Herkunft eines Links (nur mit JS-Rendering ungleich `Html`).
    #[derive(Default)]
    pub enum LinkOrigin {
        #[default]
        Html => "HTML",
        RenderedHtml => "Rendered HTML",
        Both => "HTML & Rendered HTML",
    }
}

string_enum! {
    /// Indexierbarkeit einer URL.
    pub enum Indexability {
        Indexable => "Indexable",
        NonIndexable => "Non-Indexable",
    }
}

string_enum! {
    /// Begründung der Nicht-Indexierbarkeit (TECH-001). Die Variantenreihenfolge ist die Prüfreihenfolge.
    pub enum IndexabilityStatus {
        BlockedByRobots => "Blocked by Robots.txt",
        NoResponse => "No Response",
        Redirected => "Redirected",
        ClientError => "Client Error",
        ServerError => "Server Error",
        SelfReferencingMetaRefresh => "Self Referencing Meta Refresh",
        Noindex => "noindex",
        Canonicalised => "Canonicalised",
    }
}

string_enum! {
    /// Wie eine URL entdeckt wurde.
    pub enum DiscoveryKind {
        Start => "start",
        ListInput => "list_input",
        Link => "link",
        Redirect => "redirect",
        Sitemap => "sitemap",
        RobotsTxt => "robots_txt",
    }
}

string_enum! {
    /// Bearbeitungszustand einer URL.
    pub enum UrlState {
        Discovered => "discovered",
        Queued => "queued",
        Done => "done",
    }
}

string_enum! {
    /// Grund, warum eine URL nicht gecrawlt wurde (AGENT-012); nur gesetzt, wenn der Zustand nicht `Done` ist.
    pub enum NotCrawledReason {
        Excluded => "excluded",
        NotIncluded => "not_included",
        OutsideScope => "outside_scope",
        StoreOnly => "store_only",
        CrawlDisabled => "crawl_disabled",
        Nofollow => "nofollow",
        DepthLimit => "depth_limit",
        UrlLimit => "url_limit",
        RedirectLimit => "redirect_limit",
        ExternalNotChecked => "external_not_checked",
        ListModeDepth => "list_mode_depth",
    }
}

string_enum! {
    /// Crawl-Modus (CRAWL-026/028).
    #[derive(Default)]
    pub enum CrawlMode {
        #[default]
        Spider => "spider",
        List => "list",
    }
}

string_enum! {
    /// Zustand eines Crawls (CRAWL-128; Schlüssel wie OUT-228). `Incomplete` = „Unterbrochen“.
    pub enum CrawlState {
        Running => "Running",
        Paused => "Paused",
        Completed => "Completed",
        Incomplete => "Incomplete",
        Failed => "Failed",
    }
}

string_enum! {
    /// Grund für das Ende eines Crawls.
    pub enum EndReason {
        QueueEmpty => "queue_empty",
        LimitReached => "limit_reached",
        Stopped => "stopped",
    }
}

string_enum! {
    /// Umgang mit robots.txt (CRAWL-099).
    #[derive(Default)]
    pub enum RobotsMode {
        #[default]
        Respect => "respect",
        Ignore => "ignore",
        IgnoreButReport => "ignore_but_report",
    }
}

string_enum! {
    /// Verhalten bei nicht erreichbarer robots.txt (CRAWL-100).
    #[derive(Default)]
    pub enum RobotsUnreachablePolicy {
        #[default]
        BlockHost => "block_host",
        AllowAll => "allow_all",
    }
}

string_enum! {
    /// Cookie-Verhalten (CRAWL-088).
    #[derive(Default)]
    pub enum CookieMode {
        #[default]
        SessionOnly => "session_only",
        Persistent => "persistent",
        None => "none",
    }
}

string_enum! {
    /// Rendering-Modus (CRAWL-077); M1 nur `Text`.
    #[derive(Default)]
    pub enum RenderingMode {
        #[default]
        Text => "text",
        JavaScript => "javascript",
    }
}

string_enum! {
    /// Herkunft von Canonical/Hreflang/Pagination-Angaben.
    pub enum HintSource {
        Html => "html",
        Http => "http",
        Sitemap => "sitemap",
    }
}

string_enum! {
    /// Quelle einer Robots-Direktive.
    pub enum DirectiveSource {
        MetaRobots => "meta_robots",
        XRobotsTag => "x_robots_tag",
    }
}

string_enum! {
    /// Quelle einer Refresh-Angabe.
    pub enum RefreshSource {
        Meta => "meta",
        Header => "header",
    }
}

string_enum! {
    /// Quelle einer Bildinstanz.
    pub enum ImageSource {
        Img => "img",
        A => "a",
        Srcset => "srcset",
        InputImage => "input_image",
        Poster => "poster",
        Css => "css",
    }
}

string_enum! {
    /// Teilschritt der Crawl-Analyse (CRAWL-141); `as_str()` entspricht den Schlüsseln `analysis.<schritt>`.
    pub enum AnalysisStep {
        LinkScore => "link_score",
        Canonicals => "canonicals",
        Pagination => "pagination",
        Hreflang => "hreflang",
        Links => "links",
        Sitemaps => "sitemaps",
        Content => "content",
        Images => "images",
        Analytics => "analytics",
        SearchConsole => "search_console",
        LinkMetrics => "link_metrics",
        Amp => "amp",
    }
}

string_enum! {
    /// Zustand eines Analyse-Teilschritts.
    pub enum StepState {
        Pending => "pending",
        Done => "done",
        Skipped => "skipped",
        NotApplicable => "not_applicable",
    }
}

string_enum! {
    /// Art einer Sitemap-Datei.
    pub enum SitemapKind {
        Urlset => "urlset",
        Index => "index",
        Invalid => "invalid",
    }
}

string_enum! {
    /// Umgang mit mehr als 500 hreflang-Annotationen (TECH-004).
    #[derive(Default)]
    pub enum HreflangOverLimit {
        #[default]
        Truncate => "truncate",
        Skip => "skip",
    }
}

string_enum! {
    /// Zeilenende der CSV-Exporte (OUT-002).
    #[derive(Default)]
    pub enum LineEnding {
        #[default]
        Crlf => "crlf",
        Lf => "lf",
    }
}

/// Zustand eines Filters (ONPAGE-007 / TECH-005). Trägt Daten (`NeedsConfig`) und hat deshalb kein `FromStr`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterState {
    /// Zahl liegt vor (live oder nach abgeschlossener Analyse).
    Ready,
    /// Nachlauf-Filter, Crawl-Analyse noch nicht gelaufen („Crawl-Analyse erforderlich“).
    NeedsCrawlAnalysis,
    /// Konfigurationsschlüssel, der `true` sein muss, ist aus.
    NeedsConfig(String),
    /// Nur mit JS-Rendering befüllbar.
    NeedsRendering,
    /// Analyse veraltet (nach „Neu crawlen“/Entfernen).
    Stale,
}

impl FilterState {
    /// Kurzname des Zustands (ohne Nutzdaten).
    pub fn as_str(&self) -> &'static str {
        match self {
            FilterState::Ready => "ready",
            FilterState::NeedsCrawlAnalysis => "needs_crawl_analysis",
            FilterState::NeedsConfig(_) => "needs_config",
            FilterState::NeedsRendering => "needs_rendering",
            FilterState::Stale => "stale",
        }
    }
}

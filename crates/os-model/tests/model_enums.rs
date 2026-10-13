//! Enums: kanonische Strings, Rundreise, Anzeigebezeichnungen.

use std::str::FromStr;

use os_model::*;

fn round_trip<T>(all: &[T])
where
    T: Copy
        + PartialEq
        + std::fmt::Debug
        + FromStr
        + serde::Serialize
        + serde::de::DeserializeOwned,
    <T as FromStr>::Err: std::fmt::Debug,
{
    let str_of = |v: &T| {
        serde_json::to_string(v)
            .unwrap()
            .trim_matches('"')
            .to_string()
    };
    for v in all {
        let s = str_of(v);
        assert_eq!(T::from_str(&s).unwrap(), *v, "FromStr {s}");
        let back: T = serde_json::from_str(&format!("\"{s}\"")).unwrap();
        assert_eq!(back, *v, "serde {s}");
    }
}

#[test]
fn all_string_enums_round_trip() {
    round_trip(ContentClass::ALL);
    round_trip(NoResponseReason::ALL);
    round_trip(RedirectType::ALL);
    round_trip(LinkType::ALL);
    round_trip(PathType::ALL);
    round_trip(LinkPosition::ALL);
    round_trip(LinkOrigin::ALL);
    round_trip(Indexability::ALL);
    round_trip(IndexabilityStatus::ALL);
    round_trip(DiscoveryKind::ALL);
    round_trip(UrlState::ALL);
    round_trip(NotCrawledReason::ALL);
    round_trip(CrawlMode::ALL);
    round_trip(CrawlState::ALL);
    round_trip(EndReason::ALL);
    round_trip(RobotsMode::ALL);
    round_trip(RobotsUnreachablePolicy::ALL);
    round_trip(CookieMode::ALL);
    round_trip(RenderingMode::ALL);
    round_trip(HintSource::ALL);
    round_trip(DirectiveSource::ALL);
    round_trip(RefreshSource::ALL);
    round_trip(ImageSource::ALL);
    round_trip(AnalysisStep::ALL);
    round_trip(StepState::ALL);
    round_trip(SitemapKind::ALL);
    round_trip(HreflangOverLimit::ALL);
    round_trip(LineEnding::ALL);
    round_trip(IssueType::ALL);
    round_trip(IssuePriority::ALL);
}

#[test]
fn as_str_values_are_unique_per_enum() {
    fn unique<T: Copy>(all: &[T], f: impl Fn(&T) -> &'static str) {
        let mut seen = std::collections::HashSet::new();
        for v in all {
            assert!(seen.insert(f(v)), "doppelter Wert {}", f(v));
        }
    }
    unique(NoResponseReason::ALL, NoResponseReason::as_str);
    unique(LinkType::ALL, LinkType::as_str);
    unique(IndexabilityStatus::ALL, IndexabilityStatus::as_str);
}

#[test]
fn unknown_value_is_rejected_with_kind() {
    let err = LinkType::from_str("Teleporter").unwrap_err();
    assert_eq!(
        err,
        ModelError::UnknownValue {
            kind: "LinkType",
            value: "Teleporter".into()
        }
    );
    assert!(serde_json::from_str::<CrawlState>("\"running\"").is_err());
}

// spec: ONPAGE-020
#[test]
fn content_class_labels() {
    let labels: Vec<_> = ContentClass::ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        labels,
        [
            "HTML",
            "JavaScript",
            "CSS",
            "Images",
            "PDF",
            "Plugins",
            "Media",
            "Fonts",
            "XML",
            "Other",
            "Unknown"
        ]
    );
    assert_eq!(ContentClass::default(), ContentClass::Unknown);
}

// spec: ONPAGE-018, OUT-081
#[test]
fn redirect_type_labels_differ_only_for_http_refresh() {
    assert_eq!(RedirectType::Http.tab_label(), "HTTP Redirect");
    assert_eq!(RedirectType::HstsPolicy.tab_label(), "HSTS Policy");
    assert_eq!(RedirectType::JavaScript.tab_label(), "JavaScript Redirect");
    assert_eq!(
        RedirectType::MetaRefresh.tab_label(),
        "Meta Refresh Redirect"
    );
    assert_eq!(
        RedirectType::HttpRefresh.tab_label(),
        "HTTP Refresh Redirect"
    );
    assert_eq!(RedirectType::HttpRefresh.report_label(), "HTTP Refresh");
    for t in [
        RedirectType::Http,
        RedirectType::HstsPolicy,
        RedirectType::JavaScript,
        RedirectType::MetaRefresh,
    ] {
        assert_eq!(t.tab_label(), t.report_label());
    }
}

// spec: ONPAGE-065
#[test]
fn no_response_status_texts() {
    assert_eq!(
        NoResponseReason::DnsLookupFailed.status_text(),
        "DNS Lookup Failed"
    );
    assert_eq!(
        NoResponseReason::ConnectionRefused.status_text(),
        "Connection Refused"
    );
    assert_eq!(
        NoResponseReason::ConnectionTimeout.status_text(),
        "Connection Timeout"
    );
    // K-04: Antwort-Timeout hat denselben Text, aber einen eigenen DB-Wert.
    assert_eq!(
        NoResponseReason::ResponseTimeout.status_text(),
        "Connection Timeout"
    );
    assert_ne!(
        NoResponseReason::ResponseTimeout.as_str(),
        NoResponseReason::ConnectionTimeout.as_str()
    );
    assert_eq!(NoResponseReason::TlsError.status_text(), "TLS Error");
    assert_eq!(
        NoResponseReason::BlockedByRobotsUnreachable.status_text(),
        "Blocked by Robots.txt (robots.txt unreachable)"
    );
}

// spec: OUT-031
#[test]
fn link_type_export_values() {
    assert_eq!(LinkType::Css.as_str(), "CSS");
    assert_eq!(LinkType::HttpCanonical.as_str(), "HTTP Canonical");
    assert_eq!(LinkType::RelNext.as_str(), "Rel Next");
    assert_eq!(LinkType::RelPrev.as_str(), "Rel Prev");
    assert_eq!(LinkType::Amp.as_str(), "AMP");
    assert_eq!(LinkType::XmlSitemap.as_str(), "XML Sitemap");
    assert_eq!(LinkType::PdfLink.as_str(), "PDF Link");
    assert_eq!(PathType::ProtocolRelative.as_str(), "Protocol-Relative");
    assert_eq!(LinkOrigin::Both.as_str(), "HTML & Rendered HTML");
    assert_eq!(LinkPosition::Aside.as_str(), "Aside");
    assert!(LinkType::Hyperlink.is_hyperlink());
    assert!(!LinkType::Image.is_hyperlink());
    for t in [
        LinkType::Image,
        LinkType::Css,
        LinkType::JavaScript,
        LinkType::Media,
        LinkType::Iframe,
    ] {
        assert!(t.is_resource(), "{t}");
    }
    assert!(!LinkType::Hyperlink.is_resource());
    assert!(!LinkType::Canonical.is_resource());
}

// spec: TECH-001
#[test]
fn indexability_status_order_and_labels() {
    let labels: Vec<_> = IndexabilityStatus::ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Blocked by Robots.txt",
            "No Response",
            "Redirected",
            "Client Error",
            "Server Error",
            "Self Referencing Meta Refresh",
            "noindex",
            "Canonicalised"
        ]
    );
    assert_eq!(Indexability::NonIndexable.as_str(), "Non-Indexable");
}

// spec: CRAWL-141
#[test]
fn analysis_step_keys_match_config_keys() {
    let cfg = CrawlConfig::default();
    let json = serde_json::to_value(cfg.analysis).unwrap();
    for step in AnalysisStep::ALL {
        assert!(json.get(step.as_str()).is_some(), "analysis.{step} fehlt");
        assert!(cfg.analysis.step_enabled(*step));
    }
}

#[test]
fn filter_state_names() {
    assert_eq!(
        FilterState::NeedsConfig("x".into()).as_str(),
        "needs_config"
    );
    let v = serde_json::to_value(FilterState::NeedsConfig("extraction.store_html".into())).unwrap();
    assert_eq!(
        v,
        serde_json::json!({"needs_config": "extraction.store_html"})
    );
}

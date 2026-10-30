//! Tab „Security“ (ONPAGE-030, ONPAGE-031, ONPAGE-032).
//!
//! Grundmenge („All“): alle internen URLs, die in den Tabellen erscheinen (alle Inhaltstypen). `HTTP URLs`
//! und `HTTPS URLs` unterscheiden nur das Schema der URL (F-TLS); da Schema-Wechsel desselben Hosts intern
//! bleiben, ist die Vereinigung beider Filter die Grundmenge. Externe URLs erscheinen nie.
//!
//! P1-Fakten stammen aus dem Parser; die Seitenfilter gelten nur für HTML mit Status 2xx.

use crate::defs::{ColumnKind, FilterDef, FilterKind, TabDef};
use crate::tabs::internal::{
    base_columns, ext_col, multi, CANONICAL_LIST, META_ROBOTS_LIST, POPULATION, X_ROBOTS_LIST,
};

fn filter(id: &'static str, predicate: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable: false,
        default_columns: if id == "Bad Content Type" {
            &[
                "Address",
                "Content",
                "Status Code",
                "Status",
                "Indexability",
                "Indexability Status",
                "Canonical Link Element",
                "Meta Robots",
                "X-Robots-Tag",
                "Expected Content Type",
            ]
        } else {
            &[]
        },
    }
}

/// Definition des Tabs „Security“.
pub fn tab() -> TabDef {
    let mut columns = base_columns();
    columns.extend([
        multi("Canonical Link Element", CANONICAL_LIST, ColumnKind::Url),
        multi("Meta Robots", META_ROBOTS_LIST, ColumnKind::Text),
        multi("X-Robots-Tag", X_ROBOTS_LIST, ColumnKind::Text),
    ]);
    columns.push(ext_col(
        "Expected Content Type",
        "(SELECT v_text FROM url_ext WHERE url_id=u.id AND key='security.expected_content_type')",
        ColumnKind::Text,
    ));
    TabDef {
        id: "Security",
        population: POPULATION,
        columns,
        filters: vec![
            filter("All", "1"),
            filter("HTTP URLs", "u.url LIKE 'http://%'"),
            filter("HTTPS URLs", "u.url LIKE 'https://%'"),
            filter("Mixed Content", "u.content_class='HTML' AND u.status_code BETWEEN 200 AND 299 AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id=u.id AND e.key='security.mixed_content' AND e.v_int=1)"),
            filter("Form URL Insecure", "u.content_class='HTML' AND u.status_code BETWEEN 200 AND 299 AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id=u.id AND e.key='security.form_insecure' AND e.v_int=1)"),
            filter("Form on HTTP URL", "u.content_class='HTML' AND u.status_code BETWEEN 200 AND 299 AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id=u.id AND e.key='security.form_on_http' AND e.v_int=1)"),
            filter("Unsafe Cross-Origin Links", "u.content_class='HTML' AND u.status_code BETWEEN 200 AND 299 AND EXISTS (SELECT 1 FROM links l JOIN urls t ON t.id=l.target_id WHERE l.source_id=u.id AND t.is_internal=0 AND l.link_type='Hyperlink' AND lower(l.target_attr)='_blank' AND instr(' ' || lower(COALESCE(l.rel,'')) || ' ', ' noopener ')=0 AND instr(' ' || lower(COALESCE(l.rel,'')) || ' ', ' noreferrer ')=0)"),
            filter("Protocol-Relative Resource Links", "u.content_class='HTML' AND u.status_code BETWEEN 200 AND 299 AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id=u.id AND e.key='security.protocol_relative' AND e.v_int=1)"),
            filter("Missing HSTS Header", "u.status_code > 0 AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id=u.id AND e.key='security.missing_hsts' AND e.v_int=1)"),
            filter("Missing Content-Security-Policy Header", "u.status_code BETWEEN 200 AND 299 AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id=u.id AND e.key='security.missing_csp' AND e.v_int=1)"),
            filter("Missing X-Content-Type-Options Header", "u.status_code BETWEEN 200 AND 299 AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id=u.id AND e.key='security.missing_xcto' AND e.v_int=1)"),
            filter("Missing X-Frame-Options Header", "u.status_code BETWEEN 200 AND 299 AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id=u.id AND e.key='security.missing_xfo' AND e.v_int=1)"),
            filter("Missing Secure Referrer-Policy Header", "u.status_code BETWEEN 200 AND 299 AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id=u.id AND e.key='security.missing_referrer_policy' AND e.v_int=1)"),
            filter("Bad Content Type", "u.status_code > 0 AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id=u.id AND e.key='security.bad_content_type' AND e.v_int=1)"),
        ],
        default_sort: Vec::new(),
        params: |_| Vec::new(),
        hidden_population: None,
    }
}

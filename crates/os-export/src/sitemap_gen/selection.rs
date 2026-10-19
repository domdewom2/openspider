//! Auswahl aus unabhängigen Ausschlussgründen (OUT-120/121/122).
use os_model::SitemapGenConfig;

pub(super) fn sql(cfg: &SitemapGenConfig) -> String {
    let mut status = vec!["status_code = 200"];
    let mut reasons = vec!["indexability = 'Indexable'"];
    if cfg.include_3xx {
        status.push("status_code BETWEEN 300 AND 399");
        reasons.push("indexability_status = 'Redirected'");
    }
    if cfg.include_4xx {
        status.push("status_code BETWEEN 400 AND 499");
        reasons.push("indexability_status = 'Client Error'");
    }
    if cfg.include_5xx {
        status.push("status_code BETWEEN 500 AND 599");
        reasons.push("indexability_status = 'Server Error'");
    }
    if cfg.include_no_response {
        status.push("status_code = 0");
        reasons.push("indexability_status = 'No Response'");
    }
    if cfg.include_noindex {
        reasons.push("indexability_status = 'noindex'");
    }
    if cfg.include_canonicalised {
        reasons.push("indexability_status = 'Canonicalised'");
    }
    let mut filters = vec![
        "is_internal = 1".to_string(),
        "in_tables = 1".to_string(),
        "robots_blocked = 0".to_string(),
        // Unbekannter Inhaltstyp ist bei fehlender Antwort/Redirect normal. Bekannte Ressourcen
        // (CSS, Bilder usw.) werden durch die Status-Optionen nicht zu Seiten.
        format!(
            "(content_class = 'HTML' OR (content_class = 'PDF' AND {}) OR \
                 (content_class = 'Unknown' AND status_code != 200))",
            u8::from(cfg.include_pdfs)
        ),
        format!("({})", status.join(" OR ")),
        format!("({})", reasons.join(" OR ")),
    ];
    if !cfg.include_noindex {
        filters.push("COALESCE(effective_index, 1) = 1".into());
    }
    if !cfg.include_canonicalised {
        filters.push(
            "(canonical_url IS NULL OR canonical_url = '' OR COALESCE(canonical_self, 1) = 1)"
                .into(),
        );
    }
    if !cfg.include_paginated {
        filters.push("COALESCE(has_rel_prev, 0) = 0".into());
    }
    format!("SELECT url, id, crawl_depth, last_modified, crawl_timestamp FROM urls WHERE {} ORDER BY id", filters.join(" AND "))
}

//! Fragmentzeilen teilen den fragmentlosen HTTP-Abruf (CRAWL-004, K2-35).
use crate::{writer_thread::WriterSender, EngineError};
use os_model::*;
use os_store::{CrawlFile, OpenMode, StoreError};

pub fn normalize(url: &url::Url, keep_fragment: bool) -> os_url::NormUrl {
    let mut norm = os_url::normalize(url);
    if keep_fragment {
        if let Some(fragment) = url.fragment().filter(|f| !f.starts_with('!')) {
            norm.url.set_fragment(Some(fragment));
            norm.encoded = norm.url.as_str().to_string();
            norm.display.push('#');
            norm.display.push_str(fragment);
        }
    }
    norm
}

/// Arbeitet in begrenzten Batches; weder Response-Bodys noch ein crawlweiter Fakten-Cache im RAM.
pub(crate) async fn materialize(
    path: &std::path::Path,
    writer: &WriterSender,
) -> Result<(), EngineError> {
    let file = CrawlFile::open(path, OpenMode::ReadOnly)?;
    let mut after = 0;
    loop {
        let batch = inherited_batch(&file, after)?;
        let Some(page) = batch.pages.last() else {
            break;
        };
        after = page.url_id.0;
        writer
            .send(batch)
            .await
            .map_err(|_| EngineError::Join("Fragment-Fakten nicht geschrieben".into()))?;
    }
    Ok(())
}

fn inherited_batch(file: &CrawlFile, after: i64) -> Result<WriteBatch, EngineError> {
    let mut batch = WriteBatch::default();
    let mut stmt = file.conn().prepare("SELECT b.*, f.id AS fragment_id FROM url_ext e JOIN urls f ON f.id = e.url_id JOIN urls b ON b.url = e.v_text WHERE e.key = 'crawl.fragment_base' AND f.id > ?1 AND b.state = 'done' ORDER BY f.id LIMIT 128").map_err(StoreError::from)?;
    let mut rows = stmt.query([after]).map_err(StoreError::from)?;
    while let Some(r) = rows.next().map_err(StoreError::from)? {
        macro_rules! get {
            ($name:expr, $ty:ty) => {
                r.get::<_, $ty>($name).map_err(StoreError::from)?
            };
        }
        macro_rules! opt_enum {
            ($name:expr) => {
                get!($name, Option<String>)
                    .as_deref()
                    .map(str::parse)
                    .transpose()?
            };
        }
        let fetch = FetchFacts {
            status_code: get!("status_code", u16),
            status: get!("status", String),
            content_type: get!("content_type", Option<String>),
            content_class: get!("content_class", String).parse()?,
            http_version: get!("http_version", Option<String>),
            size_bytes: get!("size_bytes", u64),
            transferred_bytes: get!("transferred_bytes", u64),
            total_transferred_bytes: get!("total_transferred_bytes", u64),
            response_time_ms: get!("response_time_ms", Option<u32>),
            last_modified_raw: get!("last_modified_raw", Option<String>),
            redirect_url: get!("redirect_url", Option<String>),
            meta_refresh_delay: get!("meta_refresh_delay", Option<u32>),
            hash_md5: get!("hash_md5", Option<String>),
            charset: get!("charset", Option<String>),
            robots_blocked: get!("robots_blocked", bool),
            robots_matched_line: get!("robots_matched_line", Option<String>),
            robots_txt_url: get!("robots_txt_url", Option<String>),
            crawl_timestamp: Timestamp::from_db(&get!("crawl_timestamp", String))?,
            no_response_reason: opt_enum!("no_response_reason"),
            redirect_target: get!("redirect_target_id", Option<i64>).map(UrlId),
            redirect_type: opt_enum!("redirect_type"),
            last_modified: get!("last_modified", Option<String>)
                .as_deref()
                .map(Timestamp::from_db)
                .transpose()?,
        };
        let mut pairs = Vec::new();
        for (prefix, decorated) in [
            ("title", true),
            ("meta_desc", true),
            ("meta_kw", false),
            ("h1", false),
            ("h2", false),
        ] {
            let mut pair = ElementPair {
                count: get!(format!("{prefix}_count").as_str(), Option<u32>).unwrap_or(0),
                ..Default::default()
            };
            for (n, slot) in [(1, &mut pair.first), (2, &mut pair.second)] {
                if let Some(text) = get!(format!("{prefix}_{n}").as_str(), Option<String>) {
                    *slot = Some(ExtractedText {
                        text,
                        len: get!(format!("{prefix}_{n}_len").as_str(), Option<u32>).unwrap_or(0),
                        px: if decorated {
                            get!(format!("{prefix}_{n}_px").as_str(), Option<u32>)
                        } else {
                            None
                        },
                        in_head: decorated
                            && get!(format!("{prefix}_{n}_in_head").as_str(), Option<bool>)
                                .unwrap_or(false),
                    });
                }
            }
            pairs.push(pair);
        }
        let mut pairs = pairs.into_iter();
        let page = Some(PageFacts {
            title: pairs.next().unwrap(),
            meta_description: pairs.next().unwrap(),
            meta_keywords: pairs.next().unwrap(),
            h1: pairs.next().unwrap(),
            h2: pairs.next().unwrap(),
            word_count: get!("word_count", Option<u32>),
            text_ratio: get!("text_ratio", Option<f32>),
            canonical_count: get!("canonical_count", u32),
            canonical_url: get!("canonical_url", Option<String>),
            canonical_self: get!("canonical_self", Option<bool>),
            effective_index: get!("effective_index", Option<bool>).unwrap_or(true),
            effective_follow: get!("effective_follow", Option<bool>).unwrap_or(true),
            has_rel_next: get!("has_rel_next", Option<bool>).unwrap_or(false),
            has_rel_prev: get!("has_rel_prev", Option<bool>).unwrap_or(false),
            hreflang_count: get!("hreflang_count", u32),
            canonical_target: get!("canonical_target_id", Option<i64>).map(UrlId),
        });
        let id = UrlId(get!("fragment_id", i64));
        batch.pages.push(PageWrite {
            url_id: id,
            replace: false,
            fetch,
            page,
            indexability: get!("indexability", String).parse()?,
            indexability_status: opt_enum!("indexability_status"),
            links: vec![],
            images: vec![],
            canonicals: vec![],
            directives: vec![],
            refresh: vec![],
            hreflang: vec![],
            pagination: vec![],
            headers: vec![],
            html: None,
        });
        let hidden: Option<String> = file
            .conn()
            .query_row(
                "SELECT value FROM url_flags WHERE url_id = ?1 AND flag = 'respect_hidden'",
                [get!("id", i64)],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        if let Some(reason) = hidden {
            batch
                .url_updates
                .push(UrlUpdate::InTables { id, value: false });
            batch
                .url_flags
                .push((id, os_model::ext::FLAG_RESPECT_HIDDEN.into(), Some(reason)));
        }
    }
    Ok(batch)
}

/// Die M1-Listenplanung normalisiert ohne Fragmente. Vor der Zulassung werden die vollständigen
/// Schlüssel wiederhergestellt und Duplikate anhand dieser Schlüssel bestimmt.
pub(crate) fn restore_list_fragments(
    plan: &mut crate::list_input::ListPlan,
    cfg: &CrawlConfig,
) -> Result<(), EngineError> {
    use crate::list_input::ListOutcome;
    use os_url::RuleDecision;
    let rules = os_url::RuleSet::compile(
        &cfg.scope.include,
        &cfg.scope.exclude,
        cfg.scope.regex_partial_match,
    )
    .map_err(|e| EngineError::Invalid(e.to_string()))?;
    let mut seen = std::collections::HashSet::new();
    plan.counts.duplicates = 0;
    plan.counts.excluded = 0;
    plan.counts.not_included = 0;
    plan.counts.to_crawl = 0;
    for entry in &mut plan.entries {
        if matches!(
            entry.outcome,
            ListOutcome::Ok
                | ListOutcome::Duplicate
                | ListOutcome::Excluded
                | ListOutcome::NotIncluded
        ) {
            if let Some(norm) = &mut entry.url {
                if let Some((_, fragment)) = entry.original.split_once('#') {
                    norm.url.set_fragment(Some(fragment));
                    *norm = normalize(&norm.url, true);
                }
                entry.outcome = if !seen.insert(norm.encoded.clone()) {
                    ListOutcome::Duplicate
                } else {
                    match rules.decide(&norm.encoded) {
                        RuleDecision::Allowed => ListOutcome::Ok,
                        RuleDecision::Excluded(_) => ListOutcome::Excluded,
                        RuleDecision::NotIncluded => ListOutcome::NotIncluded,
                    }
                };
            }
        }
        match entry.outcome {
            ListOutcome::Ok => plan.counts.to_crawl += 1,
            ListOutcome::Duplicate => plan.counts.duplicates += 1,
            ListOutcome::Excluded => plan.counts.excluded += 1,
            ListOutcome::NotIncluded => plan.counts.not_included += 1,
            _ => {}
        }
    }
    Ok(())
}

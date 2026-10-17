//! Neuberechnung der Wortzahl aus gespeichertem HTML (ONPAGE-140).
pub fn run(
    file: &mut os_store::CrawlFile,
    old: &os_model::CrawlConfig,
    new: &os_model::CrawlConfig,
    cancel: &os_model::CancelToken,
) -> Result<Vec<os_model::UrlId>, crate::EngineError> {
    if old.content.area == new.content.area {
        return Ok(Vec::new());
    }
    let mut last = 0i64;
    let mut changed = Vec::new();
    let sql_error = |e| crate::EngineError::Store(os_store::StoreError::Sqlite(e));
    loop {
        cancel
            .checkpoint()
            .map_err(|_| crate::EngineError::Cancelled)?;
        let ids:Vec<i64>=file.conn().prepare("SELECT u.id FROM urls u JOIN page_html h ON h.url_id=u.id WHERE u.id>?1 AND u.is_internal=1 AND u.content_class='HTML' AND u.status_code BETWEEN 200 AND 299 ORDER BY u.id LIMIT 128").map_err(sql_error)?.query_map([last],|r|r.get(0)).map_err(sql_error)?.collect::<Result<_,_>>().map_err(sql_error)?;
        if ids.is_empty() {
            break;
        }
        let mut updates = os_store::writer::FactUpdates::default();
        let mut signatures = Vec::new();
        for id in ids {
            cancel
                .checkpoint()
                .map_err(|_| crate::EngineError::Cancelled)?;
            last = id;
            let Some(html) = os_store::queries::stored_html(file, os_model::UrlId(id))? else {
                continue;
            };
            let ct = html.charset.map(|c| format!("text/html; charset={c}"));
            let (text, _) = os_parse::charset::decode(&html.bytes, ct.as_deref(), &[]);
            let doc = os_parse::dom::Document::parse(&text);
            let area = os_parse::content::area_text(&doc, &new.content.area);
            updates.word_count.push((
                os_model::UrlId(id),
                Some(os_parse::content::word_count(&doc, &new.content.area)),
            ));
            let address: String = file
                .conn()
                .query_row("SELECT url FROM urls WHERE id=?1", [id], |r| r.get(0))
                .map_err(sql_error)?;
            let url = url::Url::parse(&address)
                .map_err(|e| crate::EngineError::Invalid(e.to_string()))?;
            let pcfg = os_parse::ParseConfig::from_crawl_config(new);
            let ctx = os_parse::PageCtx {
                url: &url,
                base: &url,
                doc: &doc,
                html: &text,
                headers: &[],
                cfg: &pcfg,
            };
            let mut page = os_model::ParsedPage::default();
            os_parse::content::run(&ctx, &mut page);
            updates
                .ext
                .extend(page.ext.into_iter().map(|(key, value)| os_model::ExtWrite {
                    url: os_model::UrlId(id),
                    key,
                    value,
                }));
            let facts = if new.extraction.readability {
                os_parse::readability::facts(&area)
            } else {
                Vec::new()
            };
            for key in [
                "readability.sentences",
                "readability.average",
                "readability.flesch",
                "readability.label",
            ] {
                let value = facts
                    .iter()
                    .find(|(k, _)| k.as_str() == key)
                    .map(|(_, v)| v.clone())
                    .unwrap_or(os_model::Value::Null);
                updates.ext.push(os_model::ExtWrite {
                    url: os_model::UrlId(id),
                    key: os_model::ExtKey::from_static(key),
                    value,
                });
            }
            if new.duplicates.near_enabled {
                signatures.push((
                    id,
                    os_store::encode_minhash(&os_parse::readability::minhash(&area)),
                ));
            }
        }
        changed.extend(os_store::writer::apply_fact_updates(file, &updates)?);
        for (id, signature) in signatures {
            file.conn()
                .execute(
                    "INSERT OR REPLACE INTO minhash VALUES (?1,?2)",
                    (id, signature),
                )
                .map_err(sql_error)?;
        }
    }
    Ok(changed)
}

//! Aufnahme abgerufener Sitemaps; Besitzer M2-E02.
use crate::{
    decide,
    frontier::{Cand, Classify, Core, Purpose, F_QUEUED, F_RULE_OK},
    sitemaps::{self, FetchedSitemap, SitemapSeed},
    worker::{header_rows, plain_page, synth_link, Failure, Shared},
};
use os_fetch::{Fetch, FetchOutcome};
use os_model::{
    CrawlConfig, DiscoveryKind, HintSource, HreflangRef, HreflangRow, LinkType, SitemapId,
    SitemapKind, SitemapWrite, Timestamp, UrlId, UrlState, WriteBatch,
};
use os_url::ScopeClass;
use std::sync::Arc;
use url::Url;
pub(crate) async fn ingest_sitemaps<F: Fetch>(
    sh: &Arc<Shared<F>>,
    seeds: Vec<SitemapSeed>,
) -> Result<(), Failure> {
    if seeds.is_empty() {
        return Ok(());
    }
    let (fresh, known) = {
        let mut core = sh.lock_core();
        let known = core.sitemap_keys.clone();
        let mut fresh = Vec::new();
        for seed in seeds {
            if core.sitemap_keys.insert(sitemaps::sitemap_key(&seed.url)) {
                fresh.push(seed);
            }
        }
        (fresh, known)
    };
    if fresh.is_empty() {
        return Ok(());
    }
    let out = sitemaps::ingest_configured(&*sh.fetcher, fresh, 3, &known, &sh.cfg.rewrite).await;
    let fetched = out.sitemaps;
    if fetched.is_empty() && out.warnings.is_empty() {
        return Ok(());
    }
    let events: Vec<os_model::CrawlEvent> = out
        .warnings
        .iter()
        .map(|w| os_model::CrawlEvent {
            at: sh.clock.now(),
            kind: "warn".to_string(),
            code: w.code.to_string(),
            message: w.message.clone(),
        })
        .collect();
    {
        let mut core = sh.lock_core();
        for fs in &fetched {
            if let Ok(u) = Url::parse(&fs.url) {
                core.sitemap_keys.insert(sitemaps::sitemap_key(&u));
            }
            core.sitemap_keys
                .insert(sitemaps::sitemap_key(&fs.fetch.url));
        }
    }
    let slot = sh.writer.reserve().await?;
    {
        let mut core = sh.lock_core();
        let mut batch = build_sitemap_batch(
            &mut core,
            &sh.cfg,
            &sh.scope,
            &sh.rules,
            &fetched,
            sh.clock.now(),
        );
        batch.events.extend(events);
        // unter der Sperre senden (siehe Entdeckung in `process`, K-78 b)
        slot.send(batch);
    }
    sh.ctl.notify.notify_one();
    Ok(())
}

/// Bildet das Schreibmodell der abgerufenen Sitemaps: URL-Zeile der Datei, `SitemapWrite` und die URLs der
/// Einträge als Warteschlangeneinträge ohne Tiefe (Priorität 1, TECH-210/CRAWL-005).
///
/// K-41d (Sitemap-Protokoll): Ein `<loc>` gilt nur, wenn Schema, Host und Port mit der Sitemap-Datei
/// übereinstimmen. Andere Einträge zählen wie ungültige (`invalid_entries`), bekommen keine URL-Zeile und
/// keinen Warteschlangeneintrag; je Datei meldet ein `CrawlEvent` (warn) ihre Anzahl. Cross-Submission per
/// robots.txt ist M2.
pub(crate) fn build_sitemap_batch(
    core: &mut Core,
    cfg: &CrawlConfig,
    scope: &impl Classify,
    rules: &os_url::RuleSet,
    fetched: &[FetchedSitemap],
    now: Timestamp,
) -> WriteBatch {
    let mut batch = WriteBatch::default();
    let base = core.next_sitemap_id;
    core.next_sitemap_id += fetched.len() as i64;
    for (i, fs) in fetched.iter().enumerate() {
        let sid = SitemapId(base + i as i64);
        let parent = fs.parent.map(|p| SitemapId(base + p as i64));
        let mut url_id = None;
        let mut status_code = None;
        if let Ok(parsed) = Url::parse(&fs.url) {
            let norm = os_url::normalize(&parsed);
            let internal = scope.classify(&norm) == ScopeClass::Internal;
            match core.lookup(&norm.encoded) {
                Some(seen) => url_id = Some(seen.id),
                None => {
                    let id = core.alloc_id();
                    core.register(&norm.encoded, id, None, F_QUEUED | F_RULE_OK);
                    // Die Start-URL einer Sitemap-Datei ist Tiefe 0; alle anderen Dateien haben keine Tiefe.
                    let is_start = fs.found_via == "start" && fs.parent.is_none();
                    batch.new_urls.push(os_model::NewUrl {
                        id,
                        url: norm.encoded.clone(),
                        address: norm.display.clone(),
                        origin: norm.origin.clone(),
                        is_internal: internal,
                        in_tables: false,
                        discovered_via: if is_start {
                            DiscoveryKind::Start
                        } else {
                            DiscoveryKind::Sitemap
                        },
                        crawl_depth: is_start.then_some(0),
                        folder_depth: os_url::folder_depth(&norm.url),
                        url_length: os_url::url_length(&norm.display),
                        state: UrlState::Done,
                        not_crawled_reason: None,
                    });
                    let mut page = plain_page(id, &fs.fetch, cfg);
                    page.headers = header_rows(&fs.fetch, cfg);
                    batch.pages.push(page);
                    url_id = Some(id);
                }
            }
        }
        if let FetchOutcome::Response(r) = &fs.fetch.outcome {
            status_code = Some(r.status);
        }
        let sitemap_origins: Vec<Url> = [Url::parse(&fs.url).ok(), Some(fs.fetch.url.clone())]
            .into_iter()
            .flatten()
            .collect();
        let (kind, entries, invalid, size) = match &fs.doc {
            Some(d) => (d.kind, d.locs.len() as u32, d.invalid_entries, d.size_bytes),
            None => (SitemapKind::Invalid, 0, 0, 0),
        };
        let mut entry_urls = Vec::new();
        let mut hreflang_rows: Vec<(UrlId, HreflangRow)> = Vec::new();
        // Spider-Modus: Einträge ohne Tiefe (Priorität 1, nur über die Sitemap erreicht). Listenmodus: die
        // Einträge sind Listen-URLs der Tiefe 0 (CRAWL-027).
        let (entry_kind, entry_depth) = if core.list_mode {
            (DiscoveryKind::ListInput, Some(0))
        } else {
            (DiscoveryKind::Sitemap, None)
        };
        let mut foreign = 0u32;
        if let (Some(doc), SitemapKind::Urlset) = (&fs.doc, kind) {
            for loc in &doc.locs {
                let Some(u) = loc.url.as_deref() else {
                    continue;
                };
                let rewritten = os_url::rewrite::apply_discovered(&cfg.rewrite, u);
                let Ok(parsed) = Url::parse(&rewritten.url) else {
                    continue;
                };
                if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
                    continue;
                }
                if !same_origin(&parsed, &sitemap_origins) {
                    foreign += 1;
                    continue;
                }
                let norm = os_url::normalize(&parsed);
                if scope.classify(&norm) != ScopeClass::Internal {
                    continue;
                }
                let cand = Cand {
                    norm: &norm,
                    internal: true,
                    kind: entry_kind,
                    store: true,
                    crawl: true,
                    reason: None,
                    resource_like: false,
                    apply_rules: true,
                    purpose: Purpose::Page,
                    source: None,
                };
                let id = core.discover(&cand, entry_depth, 0, 0, rules, &mut batch);
                entry_urls.push((id, loc.loc_raw.clone(), loc.lastmod.clone()));
                if loc.hreflang_truncated > 0 {
                    batch.ext.push(os_model::ExtWrite {
                        url: id,
                        key: os_model::ExtKey::from_static("hreflang.truncated_sitemap"),
                        value: os_model::Value::Int(i64::from(loc.hreflang_truncated)),
                    });
                }
                // Auch bei `store = false, crawl = true` durchlaufen: die Ziele laufen über `decide` und
                // `discover`, nur die hreflang-Zeilen entfallen (CRAWL-038).
                if cfg.spider.hreflang.store || cfg.spider.hreflang.crawl {
                    sitemap_hreflang(
                        core,
                        cfg,
                        scope,
                        rules,
                        &mut batch,
                        sid,
                        id,
                        &loc.hreflang,
                        &mut hreflang_rows,
                    );
                }
            }
        }
        if foreign > 0 {
            batch.events.push(os_model::CrawlEvent {
                at: now,
                kind: "warn".to_string(),
                code: "sitemap_foreign_entries".to_string(),
                message: format!(
                    "{foreign} Einträge in {} verworfen: anderes Schema, anderer Host oder anderer Port als die Sitemap-Datei",
                    fs.url
                ),
            });
        }
        batch.sitemaps.push(SitemapWrite {
            id: sid,
            url: fs.url.clone(),
            url_id,
            parent,
            kind,
            entries,
            invalid_entries: invalid + foreign,
            size_bytes: size,
            status_code,
            depth: fs.depth,
            found_via: fs.found_via.clone(),
            entry_urls,
            hreflang: hreflang_rows,
            error: None,
            entry_meta: Vec::new(),
        });
    }
    batch
}

/// Gleiches Schema, gleicher Host und gleicher Port wie eine der Adressen der Sitemap-Datei (K-41d).
fn same_origin(loc: &Url, files: &[Url]) -> bool {
    files.iter().any(|f| {
        f.scheme() == loc.scheme()
            && f.host_str() == loc.host_str()
            && f.port_or_known_default() == loc.port_or_known_default()
    })
}

/// hreflang-Angaben eines `<url>`-Blocks (TECH-210, K-41e): Ziele laufen über `decide` wie HTML-hreflang
/// (`LinkType::Hreflang`, `spider.hreflang`); `order` zählt je `loc`-URL fortlaufend über alle Sitemaps, damit
/// mehrfach gelistete URLs nichts überschreiben. `target_id` gilt, sobald eine Zielzeile existiert. Bei
/// `spider.hreflang.store = false` entstehen keine Zeilen, die Ziele werden trotzdem eingeplant.
#[allow(clippy::too_many_arguments)]
fn sitemap_hreflang(
    core: &mut Core,
    cfg: &CrawlConfig,
    scope: &impl Classify,
    rules: &os_url::RuleSet,
    batch: &mut WriteBatch,
    sitemap: SitemapId,
    loc_id: UrlId,
    refs: &[HreflangRef],
    out: &mut Vec<(UrlId, HreflangRow)>,
) {
    for h in refs {
        let ext = synth_link(LinkType::Hreflang, &h.href_raw, h.resolved.as_deref());
        let target = h
            .resolved
            .as_deref()
            .and_then(|r| Url::parse(&os_url::rewrite::apply_discovered(&cfg.rewrite, r).url).ok())
            .filter(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
            .map(|u| os_url::normalize(&u));
        let target_id = target.and_then(|norm| {
            let class = scope.classify(&norm);
            let internal = class == ScopeClass::Internal;
            let d = decide::decide(&ext, class, cfg, true);
            if !d.store && !d.crawl {
                return core.lookup(&norm.encoded).map(|s| s.id);
            }
            let cand = Cand {
                norm: &norm,
                internal,
                kind: DiscoveryKind::Sitemap,
                // K-62: nur per hreflang bekannt = keine Tabellen-Sichtbarkeit
                store: d.store && !decide::is_hreflang_store_only(LinkType::Hreflang, &d),
                crawl: d.crawl,
                reason: d.reason,
                resource_like: false,
                apply_rules: true,
                purpose: if internal {
                    Purpose::Page
                } else {
                    Purpose::External
                },
                source: Some((loc_id, LinkType::Hreflang, None)),
            };
            // Listenmodus: Alternativen sind Links der Listenseite (Tiefe 1) und fallen unter das Tiefenlimit.
            let depth = core.list_mode.then_some(1);
            Some(core.discover(&cand, depth, 0, 0, rules, batch))
        });
        if !cfg.spider.hreflang.store {
            continue;
        }
        let next = core.sitemap_hreflang_next.entry(loc_id).or_insert(0);
        let mut r = h.clone();
        r.order = *next;
        r.source = HintSource::Sitemap;
        *next += 1;
        out.push((
            loc_id,
            HreflangRow {
                target_id,
                sitemap_id: Some(sitemap),
                r,
            },
        ));
    }
}

//! Auftragsgebundene Bündel beachten die App-Freigabe und liefern Ergebnisse je URL.
use super::*;
use std::collections::BTreeSet;

pub fn batch_source(source: Arc<dyn ApiSource>, mut urls: Vec<UrlRef>) -> Arc<dyn ApiSource> {
    // Dieselbe Reihenfolge wie Queue::run_with: bezahlte Bündel werden zuerst ausgeliefert.
    urls.sort_by(|a, b| a.url.cmp(&b.url));
    Arc::new(BatchSource {
        source,
        urls,
        state: AsyncMutex::new(BatchState::default()),
    })
}
#[derive(Default)]
struct BatchState {
    results: Grouped,
    targets: BTreeMap<Vec<Target>, Outcome>,
}
struct BatchSource {
    source: Arc<dyn ApiSource>,
    urls: Vec<UrlRef>,
    state: AsyncMutex<BatchState>,
}
impl ApiSource for BatchSource {
    fn kind(&self) -> ApiKind {
        self.source.kind()
    }
    fn status(&self) -> ApiStatus {
        self.source.status()
    }
    fn quotas(&self) -> Vec<ProviderQuota> {
        // Die Auslieferung bereits bezahlter Ergebnisse verbraucht keine Einheiten.
        // Live-Kontingente prüft die Originalquelle vor jedem tatsächlichen Request;
        // sie bleibt auch die Quelle der Kontingentanzeige in der App.
        vec![]
    }
    fn fetch<'a>(&'a self, urls: &'a [UrlRef], ctx: &'a ApiCtx) -> BoxFuture<'a, Outcome> {
        Box::pin(async move {
            let mut state = self.state.lock().await;
            let cfg = config(ctx, self.kind());
            validate(self.kind(), cfg)?;
            let batch_size = match self.kind() {
                ApiKind::Moz if !cfg.options.moz_paid => 1,
                ApiKind::Moz => 50,
                _ => 100,
            };
            let mut output = vec![];
            for requested in urls {
                let key = (requested.url_id, requested.url.clone());
                let signature = |url: &UrlRef| {
                    cfg.levels
                        .iter()
                        .map(|level| target(&url.url, *level))
                        .collect::<Result<Vec<_>, _>>()
                };
                if !state.results.contains_key(&key) {
                    if let Some(cached) = state.targets.get(&signature(requested)?).cloned() {
                        let projected = cached.map(|values| {
                            values
                                .into_iter()
                                .map(|mut value| {
                                    value.url_id = requested.url_id;
                                    value.url = requested.url.clone();
                                    value
                                })
                                .collect()
                        });
                        state.results.insert(key.clone(), projected);
                    }
                }
                if !state.results.contains_key(&key) {
                    let mut targets = BTreeSet::new();
                    let mut admitted = vec![];
                    // Das angeforderte Ziel zuerst; ein Bündel enthält höchstens die native Zielzahl.
                    for url in std::iter::once(requested)
                        .chain(self.urls.iter().filter(|url| *url != requested))
                    {
                        // LimitedSource hat nur die tatsächlich übergebene URL im
                        // persistenten Crawl-Kontingent reserviert. Bei gesetztem
                        // Crawl-Limit dürfen wir daher keine weiteren URLs vorladen.
                        if cfg.limits.quota_limit.is_some() && url != requested {
                            continue;
                        }
                        let key = (url.url_id, url.url.clone());
                        if state.results.contains_key(&key) {
                            continue;
                        }
                        let own: BTreeSet<_> = cfg
                            .levels
                            .iter()
                            .map(|level| target(&url.url, *level))
                            .collect::<Result<_, _>>()?;
                        if targets.union(&own).count() > batch_size && !admitted.is_empty() {
                            break;
                        }
                        targets.extend(own);
                        admitted.push(url.clone());
                    }
                    if !admitted.is_empty() {
                        for (url, outcome) in self.source.fetch_outcomes(&admitted, ctx).await {
                            state.targets.insert(signature(&url)?, outcome.clone());
                            state.results.insert((url.url_id, url.url), outcome);
                        }
                    }
                }
                output.extend(
                    state
                        .results
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| Err(invalid()))?,
                );
            }
            Ok(output)
        })
    }
}

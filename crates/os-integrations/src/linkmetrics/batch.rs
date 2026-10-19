//! Ein Abruf pro explizitem Auftrag, danach verteilt die bestehende URL-Warteschlange die Ergebnisse.
use super::*;
pub fn batch_source(source: Arc<dyn ApiSource>, urls: Vec<UrlRef>) -> Arc<dyn ApiSource> {
    let quotas = source.quotas();
    Arc::new(BatchSource {
        quotas,
        source,
        urls,
        result: AsyncMutex::new(None),
    })
}
type Outcome = Result<Vec<ApiValueWrite>, ApiError>;
type Grouped = Result<BTreeMap<(Option<os_model::UrlId>, String), Vec<ApiValueWrite>>, ApiError>;
struct BatchSource {
    quotas: Vec<ProviderQuota>,
    source: Arc<dyn ApiSource>,
    urls: Vec<UrlRef>,
    result: AsyncMutex<Option<Grouped>>,
}
impl ApiSource for BatchSource {
    fn kind(&self) -> ApiKind {
        self.source.kind()
    }
    fn status(&self) -> ApiStatus {
        self.source.status()
    }
    fn quotas(&self) -> Vec<ProviderQuota> {
        // Der Auftrag wird gegen das Budget vor seinem Bündel geprüft. Bereits bezahlte
        // Ergebnisse müssen auch nach Verbrauch des letzten Kontingents gespeichert werden.
        // Die registrierte Originalquelle liefert der UI weiterhin die aktuellen Quoten.
        self.quotas.clone()
    }
    fn fetch<'a>(&'a self, urls: &'a [UrlRef], ctx: &'a ApiCtx) -> BoxFuture<'a, Outcome> {
        Box::pin(async move {
            let mut result = self.result.lock().await;
            if result.is_none() {
                *result = Some(self.source.fetch(&self.urls, ctx).await.map(|values| {
                    let mut grouped: BTreeMap<_, Vec<_>> = BTreeMap::new();
                    for value in values {
                        grouped
                            .entry((value.url_id, value.url.clone()))
                            .or_default()
                            .push(value);
                    }
                    grouped
                }));
            }
            match result.as_ref().expect("initialized") {
                Ok(values) => Ok(urls
                    .iter()
                    .flat_map(|url| {
                        values
                            .get(&(url.url_id, url.url.clone()))
                            .into_iter()
                            .flatten()
                            .cloned()
                    })
                    .collect()),
                Err(error) => Err(error.clone()),
            }
        })
    }
}

//! `CrawlSink` ist objektsicher (EngineDeps.sink_factory hält `Box<dyn CrawlSink>`).

use os_model::{NewUrl, WriteBatch};
use os_store::{CrawlSink, StoreError};

#[derive(Default)]
struct Recording {
    urls: usize,
    checkpoints: u32,
}

impl CrawlSink for Recording {
    fn write(&mut self, batch: WriteBatch) -> Result<(), StoreError> {
        self.urls += batch.new_urls.len();
        Ok(())
    }

    fn checkpoint(&mut self) -> Result<(), StoreError> {
        self.checkpoints += 1;
        Ok(())
    }
}

fn assert_send<T: Send>() {}

// spec: PERF-003
#[test]
fn sink_can_be_used_as_boxed_trait_object() {
    assert_send::<Box<dyn CrawlSink>>();
    let mut sink: Box<dyn CrawlSink> = Box::new(Recording::default());
    let mut batch = WriteBatch::default();
    batch.new_urls.push(NewUrl {
        id: os_model::UrlId(1),
        url: "https://a.test/".into(),
        address: "https://a.test/".into(),
        origin: "https://a.test".into(),
        is_internal: true,
        in_tables: true,
        discovered_via: os_model::DiscoveryKind::Start,
        crawl_depth: Some(0),
        folder_depth: 0,
        url_length: 15,
        state: os_model::UrlState::Queued,
        not_crawled_reason: None,
    });
    sink.write(batch).unwrap();
    sink.checkpoint().unwrap();
}

#[test]
fn sink_factory_type_is_expressible() {
    use std::path::Path;
    use std::sync::Arc;
    type SinkFactory = Arc<
        dyn Fn(&Path, os_store::StorageProfile) -> Result<Box<dyn CrawlSink>, StoreError>
            + Send
            + Sync,
    >;
    let f: SinkFactory = Arc::new(|_, _| Ok(Box::new(Recording::default())));
    assert!(f(Path::new("x"), os_store::StorageProfile::Balanced).is_ok());
}

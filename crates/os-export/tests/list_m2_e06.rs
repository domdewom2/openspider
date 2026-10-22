// M2-E06 acceptance tests live with their owned fixtures. The export crate already
// has all dependencies required to exercise the engine-to-export round trip.
#[path = "../../../tests/fixtures/crawl-m2-list/export_tests.rs"]
mod acceptance;

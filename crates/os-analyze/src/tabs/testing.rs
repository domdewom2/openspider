//! Threadlokale Testquelle für Registry-Verträge über Crate-Grenzen.
use super::{ColumnDef, ColumnSource, FilterDef, FilterKind};
use std::cell::Cell;
thread_local! { static ENABLED: Cell<bool> = const { Cell::new(false) }; }

/// Installiert eine Quelle für die Dauer einer synchronen Prüfung, auch bei Panic begrenzt.
pub fn with_fixture_source<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENABLED.set(self.0);
        }
    }
    let _restore = Restore(ENABLED.replace(true));
    run()
}

pub(super) fn source() -> Option<Box<dyn ColumnSource>> {
    ENABLED
        .get()
        .then(|| Box::new(FixtureSource) as Box<dyn ColumnSource>)
}
struct FixtureSource;
impl ColumnSource for FixtureSource {
    fn columns(&self, _: &os_model::CrawlConfig) -> Vec<ColumnDef> {
        Vec::new()
    }
    fn filters(&self, _: &os_model::CrawlConfig) -> Vec<FilterDef> {
        let mut f = FilterDef::placeholder("Fixture Filter", FilterKind::Live, &[]);
        f.predicate = "u.id = 1";
        vec![f]
    }
}

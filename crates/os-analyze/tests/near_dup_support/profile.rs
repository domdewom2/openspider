//! Minimal benchmark-only tracing sink; the product uses the application's logger.
struct Profile;

impl tracing::Subscriber for Profile {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.target().contains("near_duplicates")
    }

    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        struct Fields;
        impl tracing::field::Visit for Fields {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                eprintln!("{}={value:?}", field.name());
            }
        }
        event.record(&mut Fields);
    }
}

pub fn capture<T>(run: impl FnOnce() -> T) -> T {
    tracing::subscriber::with_default(Profile, run)
}

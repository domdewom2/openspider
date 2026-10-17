//! Feste Start- und Änderungsreihenfolge der App-Dienste.
use super::{lifecycle_bus::CrawlEvent, App};
pub fn start(app: &App) {
    crate::commands::settings_network::on_startup(app);
    crate::logging::init_for_app(app);
    crate::commands::api::on_startup(app);
    crate::commands::crawls::start_retention(app);
    app.crawl_events().subscribe(Box::new(|app, event| {
        if let CrawlEvent::ConfigChanged { key, old, new } = event {
            crate::commands::api::on_config_changed(app, key, old, new);
            crate::commands::sidebar_ext::on_config_changed(app, key, old, new);
        }
    }));
}
pub fn settings_changed(app: &App) {
    crate::logging::apply_settings(app);
    crate::commands::crawls::on_settings_changed(app);
}

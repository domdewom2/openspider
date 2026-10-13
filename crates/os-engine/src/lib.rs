//! `os-engine` – Frontier, Scheduler, Worker-Pool, Pause/Resume und Crawl-Modi
//! (Spider, Liste, SERP, Vergleich). Orchestriert fetch, parse, store und optional render.

/// Name dieses Crates (Platzhalter bis zur ersten echten Funktion).
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches() {
        assert_eq!(NAME, "os-engine");
    }
}

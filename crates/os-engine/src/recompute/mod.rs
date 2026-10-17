//! Register für Konfigurationsänderungen ohne Recrawl (K2-43).
use crate::EngineError;
use os_model::{CancelToken, CrawlConfig, UrlId};
use os_store::CrawlFile;
pub mod d02;
pub mod d06;
pub mod d16;
pub mod e01;
pub type RecomputeFn =
    fn(&mut CrawlFile, &CrawlConfig, &CrawlConfig, &CancelToken) -> Result<Vec<UrlId>, EngineError>;
pub struct RecomputeEntry {
    pub owner: &'static str,
    pub triggers: &'static [&'static str],
    pub run: RecomputeFn,
}
#[derive(Debug, Default)]
pub struct RecomputeOutcome {
    pub changed: Vec<UrlId>,
    pub ran: Vec<&'static str>,
}
pub const ENTRIES: &[RecomputeEntry] = &[
    RecomputeEntry {
        owner: "M2-E01",
        triggers: &["extraction.hash", "content.area"],
        run: e01::run,
    },
    RecomputeEntry {
        owner: "M2-D02",
        triggers: &["content.area"],
        run: d02::run,
    },
    RecomputeEntry {
        owner: "M2-D06",
        triggers: &[
            "advanced.respect_noindex",
            "advanced.respect_canonical",
            "advanced.respect_self_ref_meta_refresh",
        ],
        run: d06::run,
    },
    RecomputeEntry {
        owner: "M2-D16",
        triggers: &["link_positions.rules"],
        run: d16::run,
    },
];
pub fn run_triggered(
    file: &mut CrawlFile,
    old: &CrawlConfig,
    new: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<RecomputeOutcome, EngineError> {
    let a = serde_json::to_value(old).map_err(|e| EngineError::Invalid(e.to_string()))?;
    let b = serde_json::to_value(new).map_err(|e| EngineError::Invalid(e.to_string()))?;
    let mut out = RecomputeOutcome::default();
    for entry in ENTRIES {
        if entry.triggers.iter().any(|path| {
            let ptr = format!("/{}", path.replace('.', "/"));
            a.pointer(&ptr) != b.pointer(&ptr)
        }) {
            cancel.checkpoint().map_err(|_| EngineError::Cancelled)?;
            out.changed.extend((entry.run)(file, old, new, cancel)?);
            out.ran.push(entry.owner);
        }
    }
    out.changed.sort_unstable();
    out.changed.dedup();
    Ok(out)
}

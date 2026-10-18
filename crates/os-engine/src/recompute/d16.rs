//! Linkpositionen aus gespeicherten Pfaden neu zuordnen, ohne HTML oder Netzwerk (INTEG-082).
use os_model::{CancelToken, CrawlConfig, LinkPosition, UrlId};
use os_store::{queries, writer, CrawlFile};

pub fn run(
    file: &mut CrawlFile,
    old: &CrawlConfig,
    new: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<Vec<UrlId>, crate::EngineError> {
    cancel
        .checkpoint()
        .map_err(|_| crate::EngineError::Cancelled)?;
    if old.link_positions == new.link_positions {
        return Ok(Vec::new());
    }
    let mut updates = writer::FactUpdates::default();
    queries::for_each_link_path(file, |id, path, in_head| {
        if cancel.is_cancelled() {
            return;
        }
        let landmark = if in_head == Some(true) || path.starts_with("//head/") {
            LinkPosition::Head
        } else {
            LinkPosition::Content
        };
        let previous =
            os_parse::link_position::classify(path, landmark.clone(), &old.link_positions);
        let next = os_parse::link_position::classify(path, landmark, &new.link_positions);
        if previous != next {
            updates.link_positions.push((id, Some(next.to_string())));
        }
    })?;
    // Ein Abbruch vor dem Schreibschritt hinterlässt die bisherige Zuordnung vollständig.
    cancel
        .checkpoint()
        .map_err(|_| crate::EngineError::Cancelled)?;
    if updates.link_positions.is_empty() {
        return Ok(Vec::new());
    }
    Ok(writer::apply_fact_updates(file, &updates)?)
}

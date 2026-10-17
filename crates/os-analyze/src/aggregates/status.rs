//! Statusquelle für UI-080, Fachumsetzung M2-K03.
#[derive(Debug, Clone, Default)]
pub struct AnalysisStatusData {
    pub steps: Vec<(os_model::AnalysisStep, os_model::StepState)>,
}
// STUB(M2-K03)
pub fn analysis_status(_: &os_store::CrawlFile, _: &os_model::CrawlConfig) -> AnalysisStatusData {
    AnalysisStatusData::default()
}

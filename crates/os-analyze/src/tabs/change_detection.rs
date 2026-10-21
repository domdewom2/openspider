//! Change Detection (TECH-300–307). Die befüllte Definition liefert `compare::change::tab`.
//! Außerhalb eines Vergleichs hat dieser Registry-Eintrag keine Population.
use crate::defs::TabDef;

pub fn tab() -> TabDef {
    let mut tab = crate::compare::change::tab(&os_model::ChangeDetectionConfig::default(), true);
    tab.population = "u.id < 0";
    for col in &mut tab.columns {
        col.sql = crate::defs::ColumnSql::Expr("NULL");
    }
    for filter in &mut tab.filters {
        filter.predicate = "u.id < 0";
    }
    // Das statische Register bewahrt seine historischen Kennungen; die Vergleichsansicht nutzt TECH-300.
    tab.filters.sort_by_key(|f| {
        super::M2_FILTERS
            .iter()
            .position(|(t, id, _, _, _)| *t == "Change Detection" && *id == f.id)
            .unwrap_or(usize::MAX)
    });
    tab
}

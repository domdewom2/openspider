//! sidebar.overview, issues, issueExplain (UI-061, UI-062, UI-065).
//!
//! STUB(M2-K06): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K06 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K06",
    [
        ("sidebar.overview", "ScopeArgs", "OverviewDto", false),
        ("sidebar.issues", "IssuesArgs", "IssueRow[]", false),
        (
            "sidebar.issueExplain",
            "IssueExplainArgs",
            "IssueText",
            false
        ),
    ]
);

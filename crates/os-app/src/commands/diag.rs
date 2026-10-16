//! diag.* (UI-127, UI-019, OUT-245); diag.usageStats liefert M2-O02 über denselben Eintrag.
//!
//! STUB(M2-O01): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-O01 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-O01",
    [
        ("diag.debugInfo", "void", "DebugInfo", false),
        ("diag.bundle", "BundleArgs", "FileResult", true),
        ("diag.logFrontend", "LogFrontendArgs", "void", false),
        ("diag.openLogs", "void", "void", false),
        ("diag.usageStats", "UsageStatsArgs", "UsageStats", false),
    ]
);

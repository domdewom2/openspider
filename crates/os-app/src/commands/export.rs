//! export.* (OUT-006, OUT-020, OUT-021, OUT-030 ff., OUT-120 ff.).
//!
//! STUB(M2-K06): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K06 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K06",
    [
        ("export.catalog", "void", "ExportCatalog", false),
        ("export.suggestName", "SuggestNameArgs", "string", false),
        ("export.view", "ExportViewArgs", "FileResult", true),
        ("export.detail", "ExportDetailArgs", "FileResult", true),
        ("export.bulk", "ExportBulkArgs", "FileResult", true),
        ("export.report", "ExportBulkArgs", "FileResult", true),
        ("export.sitemap", "ExportSitemapArgs", "FileResult", true),
        ("export.issues", "ExportPanelArgs", "FileResult", true),
        ("export.sidebar", "ExportPanelArgs", "FileResult", true),
    ]
);

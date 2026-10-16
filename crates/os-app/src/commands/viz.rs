//! viz.* (UI-090 … UI-100).
//!
//! STUB(M2-K16): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K16 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K16",
    [
        ("viz.open", "VizOpenArgs", "VizOpenResult", false),
        ("viz.graph", "VizGraphArgs", "GraphDto", false),
        ("viz.words", "VizWordsArgs", "WordFreq[]", false),
        ("viz.exportCsv", "VizExportArgs", "FileResult", true),
        ("viz.focusInMain", "UrlIdArgs", "void", false),
    ]
);

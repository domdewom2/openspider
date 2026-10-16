//! detail.* der Fach-Tabs und crawl.path (K-8 … K-11).
//!
//! STUB(M2-K09): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K09 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K09",
    [
        ("detail.serp", "SerpArgs", "SerpData", false),
        (
            "detail.serpMeasure",
            "SerpMeasureArgs",
            "SerpMeasure",
            false
        ),
        ("detail.serpSaveEdit", "SerpEditArgs", "void", false),
        ("detail.serpReset", "SerpEditArgs", "void", false),
        ("detail.sourceOpen", "SourceOpenArgs", "SourceDoc", false),
        ("detail.sourceLines", "SourceLinesArgs", "string[]", false),
        ("detail.sourceDiff", "SourceDiffArgs", "DiffHunks", true),
        ("detail.screenshot", "UrlIdArgs", "ScreenshotInfo", false),
        ("detail.imagePreview", "UrlIdArgs", "ImagePreview", false),
        ("detail.structuredData", "UrlIdArgs", "SdTree", false),
        (
            "detail.duplicates",
            "DuplicatesArgs",
            "DuplicatesResult",
            false
        ),
        (
            "detail.lighthouse",
            "LighthouseArgs",
            "LighthouseDto",
            false
        ),
        ("crawl.path", "UrlIdArgs", "CrawlPathDto[]", false),
    ]
);

//! crawl.* (UI-003, CRAWL-126/127/131).
//!
//! STUB(M2-K04): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K04 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K04",
    [
        (
            "crawl.validateStartUrl",
            "ValidateStartUrlArgs",
            "NormalizedUrl",
            false
        ),
        ("crawl.parseList", "ListSource", "ListPreview", false),
        ("crawl.start", "StartRequest", "CrawlRef", false),
        ("crawl.pause", "void", "void", false),
        ("crawl.resume", "void", "void", false),
        ("crawl.stop", "void", "void", false),
        ("crawl.clear", "void", "void", false),
        ("crawl.open", "OpenCrawlArgs", "CrawlSummaryDto", false),
        ("crawl.saveAs", "SaveAsArgs", "FileResult", true),
        ("crawl.recrawl", "SelectionArgs", "CountResult", true),
        ("crawl.remove", "SelectionArgs", "CountResult", true),
    ]
);

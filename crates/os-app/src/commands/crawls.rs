//! crawls.* (OUT-228).
//!
//! STUB(M2-O02): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-O02 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-O02",
    [
        ("crawls.checkLocation", "PathArgs", "string[]", false),
        ("crawls.list", "CrawlListQuery", "CrawlListing[]", false),
        ("crawls.rename", "CrawlRenameArgs", "void", false),
        ("crawls.move", "CrawlMoveArgs", "CountResult", true),
        ("crawls.lock", "IdsArgs", "void", false),
        ("crawls.unlock", "IdsArgs", "void", false),
        ("crawls.duplicate", "IdsArgs", "CountResult", true),
        ("crawls.delete", "IdsArgs", "CountResult", true),
        ("crawls.import", "CrawlsImportArgs", "ImportResult", true),
        ("crawls.export", "CrawlsExportArgs", "CountResult", true),
        ("crawls.reveal", "IdsArgs", "void", false),
        ("crawls.copyIds", "IdsArgs", "void", false),
    ]
);

// STUB(M2-O02)
pub fn start_retention(_app: &crate::App) {}

// STUB(M2-O02)
pub fn on_settings_changed(_app: &crate::App) {}

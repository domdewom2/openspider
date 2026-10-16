//! config.* (ONPAGE-002, CRAWL-132, OUT-224).
//!
//! STUB(M2-K07): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K07 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K07",
    [
        ("config.get", "void", "CrawlConfigDto", false),
        (
            "config.validate",
            "CrawlConfigDto",
            "ValidationIssue[]",
            false
        ),
        ("config.set", "CrawlConfigDto", "ConfigSetResult", false),
        ("config.schema", "void", "JsonValue", false),
        ("config.diff", "ConfigDiffArgs", "DiffRow[]", false),
        ("config.testRules", "TestRulesArgs", "RuleTrace", false),
    ]
);

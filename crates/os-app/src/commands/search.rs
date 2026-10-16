//! search.* (UI-011).
//!
//! STUB(M2-K02): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K02 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K02",
    [
        ("search.parse", "SearchParseArgs", "RuleNode", false),
        ("search.validateRegex", "ValidateRegexArgs", "void", false),
    ]
);

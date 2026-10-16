//! compare.* (OUT-140 ff.).
//!
//! STUB(M2-C01): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-C01 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-C01",
    [
        ("compare.open", "CompareOpenArgs", "CompareSession", false),
        ("compare.swap", "void", "CompareSession", false),
        ("compare.testMapping", "TestMappingArgs", "RuleTrace", false),
        ("compare.close", "void", "void", false),
    ]
);

//! view.* (UI-140, UI-141, UI-142); ab Welle 2 M2-K05.
//!
//! STUB(M2-K06): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K06 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K06",
    [
        ("view.open", "ViewOpenArgs", "ViewHandle", false),
        ("view.update", "ViewUpdateArgs", "ViewHandle", false),
        ("view.rows", "RowsArgs", "RowWindow", false),
        ("view.locate", "LocateArgs", "number | null", false),
        ("view.copy", "CopyArgs", "CopyResult", false),
        ("view.close", "ViewCloseArgs", "void", false),
    ]
);

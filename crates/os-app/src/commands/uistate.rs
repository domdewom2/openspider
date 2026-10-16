//! uistate.load/save (UI-125).
//!
//! STUB(M2-F03): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-F03 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-F03",
    [
        ("uistate.load", "void", "UiStateJson", false),
        ("uistate.save", "UiStateJson", "UiStateJson", false),
    ]
);

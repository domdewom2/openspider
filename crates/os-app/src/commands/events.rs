//! events.* (UI-018, UI-143).
//!
//! STUB(M2-F03): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-F03 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-F03",
    [
        ("events.subscribe", "SubscribeArgs", "SubscriptionId", false),
        ("events.setVisibility", "SetVisibilityArgs", "void", false),
        ("events.setInterval", "SetIntervalArgs", "void", false),
    ]
);

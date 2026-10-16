//! auth.openLogin, auth.finishLogin (CRAWL-120).
//!
//! STUB(M2-U17): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-U17 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-U17",
    [
        ("auth.openLogin", "AuthOpenLoginArgs", "LoginCapture", true),
        (
            "auth.finishLogin",
            "AuthFinishLoginArgs",
            "AuthProfileInfo | null",
            false
        ),
    ]
);

//! auth.listProfiles, saveProfile, deleteProfile (OUT-223, CRAWL-121).
//!
//! STUB(M2-O01): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-O01 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-O01",
    [
        ("auth.listProfiles", "void", "AuthProfileInfo[]", false),
        (
            "auth.saveProfile",
            "AuthSaveProfileArgs",
            "AuthProfileInfo",
            false
        ),
        ("auth.deleteProfile", "NameArgs", "void", false),
    ]
);

//! profile.* (OUT-220, OUT-221).
//!
//! STUB(M2-K07): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K07 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K07",
    [
        ("profile.list", "void", "ConfigProfileInfo[]", false),
        ("profile.load", "NameArgs", "CrawlConfigDto", false),
        (
            "profile.save",
            "ProfileSaveArgs",
            "ConfigProfileInfo",
            false
        ),
        ("profile.delete", "NameArgs", "void", false),
        ("profile.setDefault", "SetDefaultProfileArgs", "void", false),
        ("profile.import", "PathArgs", "ConfigProfileInfo", false),
        ("profile.export", "ProfileExportArgs", "void", false),
    ]
);

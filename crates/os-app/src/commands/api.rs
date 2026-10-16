//! api.*, sidebar.apiStatus (INTEG-001 … 011, UI-070); APIs-Modus über crawl.start.
//!
//! STUB(M2-I01): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-I01 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-I01",
    [
        ("api.list", "void", "ApiSourceInfo[]", false),
        ("api.connect", "ApiConnectArgs", "ApiAccount", true),
        ("api.disconnect", "ApiKindArgs", "void", false),
        ("api.testConnection", "ApiKindArgs", "ApiTestResult", false),
        ("api.getSettings", "ApiKindArgs", "ApiSourceSettings", false),
        (
            "api.setSettings",
            "ApiSetSettingsArgs",
            "ApiSettingsResult",
            false
        ),
        ("api.status", "void", "ApiRow[]", false),
        ("api.fetch", "ApiFetchArgs", "ApiFetchSummary", true),
        ("api.errors", "ApiErrorsArgs", "ApiErrorRow[]", false),
        ("sidebar.apiStatus", "void", "ApiRow[]", false),
    ]
);

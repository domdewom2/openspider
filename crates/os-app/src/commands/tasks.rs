//! tasks.* (OUT-160 … OUT-170).
//!
//! STUB(M2-T01): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-T01 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-T01",
    [
        ("tasks.list", "void", "TaskInfo[]", false),
        ("tasks.get", "IdArgs", "TaskDef", false),
        ("tasks.validate", "TaskArgs", "ValidationIssue[]", false),
        ("tasks.save", "TaskArgs", "TaskInfo", false),
        ("tasks.delete", "IdArgs", "void", false),
        ("tasks.run", "IdArgs", "TaskRunRow", true),
        ("tasks.history", "TasksHistoryArgs", "TaskRunRow[]", false),
        (
            "tasks.register",
            "TaskRegisterArgs",
            "PlannerPreview",
            false
        ),
        ("tasks.unregister", "IdArgs", "void", false),
        ("tasks.export", "TasksExportArgs", "FileResult", true),
        ("tasks.import", "PathArgs", "TaskInfo[]", true),
    ]
);

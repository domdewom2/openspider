//! segment.* (CRAWL-134, UI-068).
//!
//! STUB(M2-K14): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K14 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K14",
    [
        ("segment.list", "void", "SegmentDef[]", false),
        (
            "segment.save",
            "SegmentSaveArgs",
            "SegmentSaveResult",
            false
        ),
        ("segment.delete", "IdArgs", "void", false),
        (
            "segment.validate",
            "SegmentValidateArgs",
            "ValidationIssue[]",
            false
        ),
        (
            "segment.preview",
            "SegmentPreviewArgs",
            "SegmentPreview",
            true
        ),
        ("segment.reassign", "void", "CountResult", true),
    ]
);

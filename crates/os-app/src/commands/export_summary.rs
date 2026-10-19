//! STUB(M2-X03): Command-Verträge für Welle 1.
stub_commands!(
    "M2-X03",
    [
        (
            "export.customSummaryMetrics",
            "void",
            "CustomSummaryMetric[]",
            false
        ),
        (
            "export.customSummary",
            "CustomSummaryArgs",
            "FileResult",
            true
        ),
    ]
);

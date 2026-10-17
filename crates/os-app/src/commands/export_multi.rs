//! STUB(M2-X01): Command-Verträge für Welle 1.
stub_commands!(
    "M2-X01",
    [
        ("export.multi", "ExportMultiArgs", "ExportBatchResult", true),
        ("export.presetList", "void", "ExportPreset[]", false),
        ("export.presetSave", "ExportPresetArgs", "void", false),
        ("export.presetDelete", "ExportPresetArgs", "void", false),
    ]
);

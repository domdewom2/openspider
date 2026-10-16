//! mcp.* (UI-129, OUT-285).
//!
//! STUB(M2-K17): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K17 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K17",
    [
        ("mcp.status", "void", "McpStatus", false),
        ("mcp.start", "McpStartArgs", "McpStatus", false),
        ("mcp.stop", "void", "McpStatus", false),
        ("mcp.tools", "void", "ToolInfo[]", false),
        ("mcp.setToolEnabled", "SetToolEnabledArgs", "void", false),
        ("mcp.setClasses", "SetClassesArgs", "void", false),
        ("mcp.token", "McpTokenArgs", "McpToken", false),
        ("mcp.regenerateToken", "McpTokenArgs", "McpToken", false),
        ("mcp.setAutostart", "BoolArgs", "void", false),
        ("mcp.saveToolsMarkdown", "PathArgs", "void", false),
        (
            "mcp.clientConfigPreview",
            "McpClientArgs",
            "McpClientPreview",
            false
        ),
        (
            "mcp.clientConfigWrite",
            "McpClientArgs",
            "McpClientWritten",
            false
        ),
        ("mcp.callLog", "PageArgs", "CallLogRow[]", false),
    ]
);

use os_mcp::{
    permissions::Permissions, server::McpServer, tools, McpSettings, ToolClass, ToolContext,
};

// spec: OUT-285 [P1]
// spec: AGENT-003 [M2]
#[test]
fn markdown_and_tool_list_use_the_same_enabled_schemas() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = McpSettings {
        classes: vec![ToolClass::Read, ToolClass::Crawl, ToolClass::Write],
        ..McpSettings::default()
    };
    settings.tools.insert("sql_query".into(), false);
    let permissions = Permissions::resolve(&[], false, &settings).unwrap();
    let server = McpServer::new(
        tools::all(),
        permissions,
        ToolContext::new(dir.path().to_path_buf()),
    );
    let listed = server.list();
    let markdown = server.markdown();
    assert_eq!(
        markdown.lines().filter(|s| s.starts_with("## ")).count(),
        listed.len()
    );
    assert!(!server.exposed_tool_names().contains(&"sql_query"));
    for tool in listed {
        assert!(markdown.contains(&format!("## {}\n", tool.name)));
        if let Some(props) = tool
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
        {
            for (name, schema) in props {
                assert!(markdown.contains(&format!(
                    "| {name} | `{}` |",
                    schema.to_string().replace('|', "\\|")
                )));
            }
        }
    }
}

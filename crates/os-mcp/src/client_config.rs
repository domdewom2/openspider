//! Merge our entry without deleting other client configuration. Local UI only.
use crate::{
    print_config::{self, Client},
    McpError,
};

pub fn merged(
    client: Client,
    before: &str,
    executable: &str,
    args: &[String],
) -> Result<String, McpError> {
    let generated = print_config::render(client, executable, args)?;
    let error = |e: String| McpError::Settings(e);
    if client == Client::Codex {
        let mut doc: toml::Table = if before.trim().is_empty() {
            toml::Table::new()
        } else {
            toml::from_str(before).map_err(|e: toml::de::Error| error(e.to_string()))?
        };
        let own: toml::Table =
            toml::from_str(&generated).map_err(|e: toml::de::Error| error(e.to_string()))?;
        let servers = doc
            .entry("mcp_servers")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| error("mcp_servers must be a table".into()))?;
        servers.insert(
            "openspider".into(),
            own["mcp_servers"]["openspider"].clone(),
        );
        toml::to_string_pretty(&doc).map_err(|e| error(e.to_string()))
    } else {
        let mut doc: serde_json::Value = if before.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(before).map_err(|e| error(e.to_string()))?
        };
        let own: serde_json::Value =
            serde_json::from_str(&generated).map_err(|e| error(e.to_string()))?;
        let root = doc
            .as_object_mut()
            .ok_or_else(|| error("Client configuration must be an object".into()))?;
        let servers = root
            .entry("mcpServers")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .ok_or_else(|| error("mcpServers must be an object".into()))?;
        servers.insert("openspider".into(), own["mcpServers"]["openspider"].clone());
        serde_json::to_string_pretty(&doc)
            .map(|s| s + "\n")
            .map_err(|e| error(e.to_string()))
    }
}

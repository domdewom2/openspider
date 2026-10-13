//! STDIO-Transport und Logging (AGENT-001): Auf stdout steht ausschließlich JSON-RPC, alle Meldungen gehen
//! nach stderr.

use rmcp::service::ServerInitializeError;
use rmcp::ServiceExt;

use crate::error::McpError;
use crate::server::McpServer;

/// Richtet das Logging nach stderr ein (Filter aus `OPENSPIDER_LOG`, sonst `info`). Ein bereits
/// eingerichtetes Logging (z. B. durch `os-cli`) bleibt unberührt.
pub fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("OPENSPIDER_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();
}

/// Bedient einen MCP-Client über stdin/stdout, bis dieser die Verbindung schließt (stdin-Ende).
///
/// # Errors
/// [`McpError::Transport`], wenn der Handshake scheitert oder der Dienst abstürzt. Schließt der Client die
/// Verbindung vor dem Handshake, ist das ein normales Ende (`Ok`).
pub async fn serve_stdio(server: McpServer) -> Result<(), McpError> {
    serve(server, rmcp::transport::stdio()).await
}

/// Bedient einen MCP-Client über ein beliebiges Lese-/Schreibpaar (Tests: `tokio::io::duplex`).
///
/// # Errors
/// [`McpError::Transport`], wenn der Handshake scheitert oder der Dienst abstürzt.
pub async fn serve<R, W>(server: McpServer, io: (R, W)) -> Result<(), McpError>
where
    R: tokio::io::AsyncRead + Send + Unpin + 'static,
    W: tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    let running = match server.serve(io).await {
        Ok(running) => running,
        // Der Client hat die Verbindung geschlossen, bevor er `initialize` geschickt hat: normales Ende.
        Err(ServerInitializeError::ConnectionClosed(_)) => {
            tracing::info!("Client hat die Verbindung vor dem Handshake geschlossen");
            return Ok(());
        }
        Err(e) => return Err(McpError::Transport(e.to_string())),
    };
    running
        .waiting()
        .await
        .map_err(|e| McpError::Transport(e.to_string()))?;
    Ok(())
}

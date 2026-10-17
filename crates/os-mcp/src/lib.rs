//! `os-mcp` – MCP-Server (STDIO und geschütztes Streamable HTTP auf Loopback), Werkzeugklassen read/crawl/write/cost.
//!
//! Modulgerüst nach M1-Plan 3.6 (P00b). Server, Freigaben, Paging und `--print-config`: P18; die lesenden
//! Werkzeuge (`tools`): P27; Einstellungen (`settings`, Nachtrag P00c): Umsetzung P18. Der Server liest Crawl-Dateien über `os-store`/`os-analyze`/`os-export`.

pub mod cli;
pub mod error;
pub mod paging;
pub mod permissions;
pub mod print_config;
pub mod server;
pub mod session;
pub mod settings;
pub mod tool;
pub mod tools;
pub mod transport;

pub use error::McpError;
pub use session::{
    SessionAccess, SessionCrawl, SessionError, SessionSource, SessionStart, SessionStatus,
};
pub use settings::{HttpSettings, McpSettings};
pub use tool::{Tool, ToolClass, ToolContext, ToolError, ToolOutput};

mod crawl_service;
pub mod http;
pub mod prompts;
mod target;

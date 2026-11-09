//! SHARED by Windows, Linux and macOS: the CV editing tools an AI may use, and an MCP server that
//! offers them. The in-app chat calls the same tools, so an AI in CVBuilder's chat and an AI app
//! connected over MCP (Claude Code, Claude Desktop and others) change the CV the same way.
pub mod server;
pub mod tools;

pub use server::{Host, handle, tool_list};
pub use tools::{INSTRUCTIONS, Outcome, Tool, call, is_editing, tools};

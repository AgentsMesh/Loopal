//! All IPC protocol method definitions, partitioned by domain.

mod agent;
mod desktop;
mod desktop_mcp;
mod desktop_skills;
mod hub;
mod meta;
mod workspace;

pub use agent::*;
pub use desktop::*;
pub use desktop_mcp::*;
pub use desktop_skills::*;
pub use hub::*;
pub use meta::*;
pub use workspace::*;

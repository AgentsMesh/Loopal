//! Shared agent IO loop — handles hub/* requests, forwards events,
//! and relays permission/question requests to UI clients.

mod dispatch_loop;
mod spawn;

pub use dispatch_loop::agent_io_loop;
pub use spawn::{spawn_io_loop, start_agent_io};

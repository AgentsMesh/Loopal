//! Shared agent IO loop — handles hub/* requests, forwards events,
//! and relays permission/question requests to UI clients.

mod dispatch_loop;
#[cfg(test)]
mod dispatch_loop_tests;
mod event_admission;
#[cfg(test)]
mod event_admission_tests;
mod event_forward;
mod registration;
mod request_dispatch;
mod root_session;
mod spawn;
#[cfg(test)]
mod spawn_tests;

pub use dispatch_loop::agent_io_loop;
pub(crate) use dispatch_loop::agent_io_loop_exact;
pub use registration::start_agent_io;
pub(crate) use registration::start_reserved_agent_io;
pub use root_session::bind_managed_root_session_id;
pub use spawn::spawn_io_loop;
pub(crate) use spawn::spawn_io_loop_exact;

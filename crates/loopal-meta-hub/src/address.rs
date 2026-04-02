//! Re-export `QualifiedAddress` from `loopal-protocol`.
//!
//! The canonical definition lives in `loopal_protocol::address` so that both
//! `loopal-agent-hub` and `loopal-meta-hub` can use it without circular deps.

pub use loopal_protocol::QualifiedAddress;

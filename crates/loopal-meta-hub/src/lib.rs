//! MetaHub — coordination layer above multiple Hub instances (Sub-Hubs).
//!
//! Manages a cluster of Hubs, enabling cross-hub agent communication,
//! unified event observation, and distributed agent spawning.
//!
//! ## Architecture
//! - `HubRegistry` — Sub-Hub connection lifecycle management
//! - `GlobalRouter` — cross-hub address resolution and message routing
//! - `EventAggregator` — multi-hub event stream aggregation
//! - `MetaHub` — thin composition layer tying subsystems together
//! - `server` — TCP listener accepting Sub-Hub connections
//! - `dispatch` — `meta/*` request routing
//! - `io_loop` — per-Sub-Hub IO processing

mod address;
pub mod aggregator;
pub mod dispatch;
mod hub_info;
mod hub_registry;
pub mod io_loop;
mod managed_hub;
mod meta_hub;
mod router;
pub mod server;

pub use address::QualifiedAddress;
pub use aggregator::EventAggregator;
pub use hub_info::{HubInfo, HubStatus};
pub use hub_registry::HubRegistry;
pub use managed_hub::ManagedHub;
pub use meta_hub::MetaHub;
pub use router::GlobalRouter;

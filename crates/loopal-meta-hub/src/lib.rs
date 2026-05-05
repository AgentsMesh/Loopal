//! MetaHub — coordination layer above multiple Hub instances (Sub-Hubs).
//!
//! Cluster coordinator. No UI clients connect here directly — UI lives on
//! individual Sub-Hubs and reaches cross-hub agents via `meta/route`.
//! Cross-hub agents (spawned via `meta/spawn`) run without interactive
//! permission support; they must use `BypassPermissions` or `Plan` mode.
//!
//! ## Architecture
//! - `HubRegistry` — Sub-Hub connection lifecycle management
//! - `GlobalRouter` — cross-hub address resolution and message routing
//! - `MetaHub` — thin composition layer tying subsystems together
//! - `server` — TCP listener accepting Sub-Hub connections
//! - `dispatch` — `meta/*` request routing
//! - `io_loop` — per-Sub-Hub IO processing

mod address;
pub mod dispatch;
mod hub_info;
mod hub_registry;
pub mod io_loop;
mod managed_hub;
mod meta_hub;
mod router;
pub mod server;

pub use address::QualifiedAddress;
pub use hub_info::{HubInfo, HubStatus};
pub use hub_registry::HubRegistry;
pub use managed_hub::ManagedHub;
pub use meta_hub::MetaHub;
pub use router::GlobalRouter;

//! Shared integration test infrastructure for Loopal.
//!
//! Provides mock providers, fixture management, event collectors, assertion
//! helpers, and a configurable `HarnessBuilder` for wiring agent_loop tests.

pub mod agent_ctx;
pub mod assertions;
pub mod captured_provider;
pub mod chunks;
pub mod events;
pub mod fixture;
pub mod git_fixture;
pub mod harness;
pub mod hook_fixture;
pub mod ipc_harness;
pub mod mcp_mock;
pub mod mock_provider;
pub mod scenarios;
mod wiring;

/// In-memory duplex transport pair — re-export of `loopal_ipc::duplex_pair`.
/// Keeps the test toolbox single-source: there is exactly one in-memory
/// transport impl in the project, this is its discoverable alias under
/// `loopal_test_support`.
pub use loopal_ipc::duplex_pair as make_duplex_pair;

pub use fixture::TestFixture;
pub use git_fixture::GitFixture;
pub use harness::{HarnessBuilder, IntegrationHarness, SpawnedHarness};
pub use hook_fixture::HookFixture;
pub use mcp_mock::MockMcpServer;

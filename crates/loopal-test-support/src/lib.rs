//! Shared integration test infrastructure for Loopal.
//!
//! Provides mock providers, fixture management, event collectors, assertion
//! helpers, and a configurable `HarnessBuilder` for wiring agent_loop tests.

pub mod assertions;
pub mod captured_provider;
pub mod chunks;
pub mod events;
pub mod fixture;
pub mod git_fixture;
pub mod harness;
pub mod hook_fixture;
pub mod mcp_mock;
pub mod mock_provider;
pub mod scenarios;
mod wiring;

pub use fixture::TestFixture;
pub use git_fixture::GitFixture;
pub use harness::{HarnessBuilder, IntegrationHarness, SpawnedHarness};
pub use hook_fixture::HookFixture;
pub use mcp_mock::MockMcpServer;

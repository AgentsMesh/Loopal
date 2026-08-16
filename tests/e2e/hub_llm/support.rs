#![allow(dead_code)]

#[path = "support_hub.rs"]
mod hub;
#[path = "support_mcp_control.rs"]
mod mcp_control;
#[path = "support_mcp_server.rs"]
mod mcp_server;
#[path = "support_permission_ui.rs"]
mod permission_ui;
#[path = "support_ui.rs"]
mod ui;
#[path = "support_ui_snapshot.rs"]
mod ui_snapshot;
#[path = "support_workflow.rs"]
mod workflow;
#[path = "support_workflow_lifecycle.rs"]
mod workflow_lifecycle;
#[path = "support_workflow_terminal.rs"]
mod workflow_terminal;

pub use hub::{API_KEY, HubEnv, HubHarness};
pub use mcp_server::GatedMcpServer;
pub use permission_ui::{PermissionApproval, PermissionClient};
pub use ui::{ObserverClient, TurnOutcome};
pub use workflow::{WorkflowTurnOutcome, replay_workflow, workflow_events};

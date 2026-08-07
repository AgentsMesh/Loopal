// Single test binary — includes all test modules
use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::Hub;
use tokio::sync::Mutex;

async fn permission_interaction_id(hub: &Arc<Mutex<Hub>>, agent: &str, logical_id: &str) -> String {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(id) = hub
                .lock()
                .await
                .pending_permissions
                .get(&(agent.to_string(), logical_id.to_string()))
                .map(|info| info.interaction_id.clone())
            {
                return id;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("permission interaction should become pending")
}

async fn plan_interaction_id(hub: &Arc<Mutex<Hub>>, agent: &str, logical_id: &str) -> String {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(id) = hub
                .lock()
                .await
                .pending_plan_approvals
                .get(&(agent.to_string(), logical_id.to_string()))
                .map(|info| info.interaction_id.clone())
            {
                return id;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("plan interaction should become pending")
}

#[path = "suite/advanced_scenarios_test.rs"]
mod advanced_scenarios_test;
#[path = "suite/agent_completed_result_test.rs"]
mod agent_completed_result_test;
#[path = "suite/collaboration_test.rs"]
mod collaboration_test;
#[path = "suite/completed_agent_tombstone_test.rs"]
mod completed_agent_tombstone_test;
#[path = "suite/completion_injection_test.rs"]
mod completion_injection_test;
#[path = "suite/completion_mock_llm_e2e_test.rs"]
mod completion_mock_llm_e2e_test;
#[path = "suite/completion_notification_policy_test.rs"]
mod completion_notification_policy_test;
#[path = "suite/completion_output_test.rs"]
mod completion_output_test;
#[path = "suite/cross_hub_completion_shadow_test.rs"]
mod cross_hub_completion_shadow_test;
#[path = "suite/desktop_mcp_layer_precedence_test.rs"]
mod desktop_mcp_layer_precedence_test;
#[path = "suite/desktop_mcp_settings_rpc_test.rs"]
mod desktop_mcp_settings_rpc_test;
#[path = "suite/desktop_provider_settings_rpc_test.rs"]
mod desktop_provider_settings_rpc_test;
#[path = "suite/desktop_sessions_rpc_test.rs"]
mod desktop_sessions_rpc_test;
#[path = "suite/desktop_settings_rpc_test.rs"]
mod desktop_settings_rpc_test;
#[path = "suite/desktop_skills_rpc_test.rs"]
mod desktop_skills_rpc_test;
#[path = "suite/dispatch_test.rs"]
mod dispatch_test;
#[path = "suite/e2e_bootstrap_test.rs"]
mod e2e_bootstrap_test;
#[path = "suite/e2e_real_vault_test.rs"]
mod e2e_real_vault_test;
#[path = "suite/e2e_secret_access_boundary_test.rs"]
mod e2e_secret_access_boundary_test;
#[path = "suite/e2e_secret_ipc_test.rs"]
mod e2e_secret_ipc_test;
#[path = "suite/event_router_test.rs"]
mod event_router_test;
#[path = "suite/hub_integration_test.rs"]
mod hub_integration_test;
#[path = "suite/hub_lifecycle_test.rs"]
mod hub_lifecycle_test;
#[path = "suite/hub_secret_client_test.rs"]
mod hub_secret_client_test;
#[path = "suite/hub_shutdown_test.rs"]
mod hub_shutdown_test;
#[path = "suite/interaction_cardinality_test.rs"]
mod interaction_cardinality_test;
#[path = "suite/interaction_cleanup_test.rs"]
mod interaction_cleanup_test;
#[path = "suite/interaction_generation_test.rs"]
mod interaction_generation_test;
#[path = "suite/interaction_terminal_delivery_test.rs"]
mod interaction_terminal_delivery_test;
#[path = "suite/multi_agent_test.rs"]
mod multi_agent_test;
#[path = "suite/multi_ui_attach_test.rs"]
mod multi_ui_attach_test;
#[path = "suite/multi_ui_consistency_test.rs"]
mod multi_ui_consistency_test;
#[path = "suite/parallel_spawn_test.rs"]
mod parallel_spawn_test;
#[path = "suite/permission_lifecycle_test.rs"]
mod permission_lifecycle_test;
#[path = "suite/permission_race_test.rs"]
mod permission_race_test;
#[path = "suite/permission_session_grant_test.rs"]
mod permission_session_grant_test;
#[path = "suite/plan_approval_relay_test.rs"]
mod plan_approval_relay_test;
#[path = "suite/race_condition_test.rs"]
mod race_condition_test;
#[path = "suite/relay_test.rs"]
mod relay_test;
#[path = "suite/secret_test_helpers.rs"]
mod secret_test_helpers;
#[path = "suite/spawn_lifecycle_test.rs"]
mod spawn_lifecycle_test;
#[path = "suite/spawn_prepare_test.rs"]
mod spawn_prepare_test;
#[path = "suite/spawn_registry_test.rs"]
mod spawn_registry_test;
#[path = "suite/spawn_remote_test.rs"]
mod spawn_remote_test;
#[path = "suite/tcp_principal_test.rs"]
mod tcp_principal_test;
#[path = "suite/tcp_ui_cleanup_test.rs"]
mod tcp_ui_cleanup_test;
#[path = "suite/tcp_ui_client_test.rs"]
mod tcp_ui_client_test;
#[path = "suite/transport_close_test.rs"]
mod transport_close_test;
#[path = "suite/ui_capability_lifecycle_test.rs"]
mod ui_capability_lifecycle_test;
#[path = "suite/view_protocol_test.rs"]
mod view_protocol_test;
#[path = "suite/view_snapshot_seed_test.rs"]
mod view_snapshot_seed_test;
#[path = "suite/view_state_routing_test.rs"]
mod view_state_routing_test;
#[path = "suite/wait_nonblocking_test.rs"]
mod wait_nonblocking_test;
#[path = "suite/workspace_git_rpc_test.rs"]
mod workspace_git_rpc_test;
#[path = "suite/workspace_rpc_support.rs"]
mod workspace_rpc_support;
#[path = "suite/workspace_rpc_test.rs"]
mod workspace_rpc_test;

// Single test binary — includes all test modules
#[path = "suite/advanced_scenarios_test.rs"]
mod advanced_scenarios_test;
#[path = "suite/agent_completed_result_test.rs"]
mod agent_completed_result_test;
#[path = "suite/collaboration_test.rs"]
mod collaboration_test;
#[path = "suite/completion_injection_test.rs"]
mod completion_injection_test;
#[path = "suite/completion_output_test.rs"]
mod completion_output_test;
#[path = "suite/dispatch_test.rs"]
mod dispatch_test;
#[path = "suite/e2e_bootstrap_test.rs"]
mod e2e_bootstrap_test;
#[path = "suite/event_router_test.rs"]
mod event_router_test;
#[path = "suite/hub_integration_test.rs"]
mod hub_integration_test;
#[path = "suite/hub_lifecycle_test.rs"]
mod hub_lifecycle_test;
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
#[path = "suite/race_condition_test.rs"]
mod race_condition_test;
#[path = "suite/relay_test.rs"]
mod relay_test;
#[path = "suite/spawn_lifecycle_test.rs"]
mod spawn_lifecycle_test;
#[path = "suite/spawn_remote_test.rs"]
mod spawn_remote_test;
#[path = "suite/tcp_ui_cleanup_test.rs"]
mod tcp_ui_cleanup_test;
#[path = "suite/tcp_ui_client_test.rs"]
mod tcp_ui_client_test;
#[path = "suite/transport_close_test.rs"]
mod transport_close_test;
#[path = "suite/view_protocol_test.rs"]
mod view_protocol_test;
#[path = "suite/view_snapshot_seed_test.rs"]
mod view_snapshot_seed_test;
#[path = "suite/view_state_routing_test.rs"]
mod view_state_routing_test;
#[path = "suite/wait_nonblocking_test.rs"]
mod wait_nonblocking_test;

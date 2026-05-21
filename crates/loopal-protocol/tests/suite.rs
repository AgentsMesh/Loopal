// Single test binary — includes all test modules
#[path = "suite/address_test.rs"]
mod address_test;
#[path = "suite/agent_state_test.rs"]
mod agent_state_test;
#[path = "suite/command_test.rs"]
mod command_test;
#[path = "suite/control_test.rs"]
mod control_test;
#[path = "suite/cron_snapshot_test.rs"]
mod cron_snapshot_test;
#[path = "suite/degeneration_event_test.rs"]
mod degeneration_event_test;
#[path = "suite/envelope_test.rs"]
mod envelope_test;
#[path = "suite/event_edge_test.rs"]
mod event_edge_test;
#[path = "suite/event_id_test.rs"]
mod event_id_test;
#[path = "suite/event_inbox_edge_test.rs"]
mod event_inbox_edge_test;
#[path = "suite/event_lifecycle_test.rs"]
mod event_lifecycle_test;
#[path = "suite/event_metadata_test.rs"]
mod event_metadata_test;
#[path = "suite/event_session_edge_test.rs"]
mod event_session_edge_test;
#[path = "suite/event_test.rs"]
mod event_test;
#[path = "suite/file_size_cap_test.rs"]
mod file_size_cap_test;
#[path = "suite/mcp_snapshot_test.rs"]
mod mcp_snapshot_test;
#[path = "suite/projection_edge_test.rs"]
mod projection_edge_test;
#[path = "suite/projection_metadata_test.rs"]
mod projection_metadata_test;
#[path = "suite/projection_test.rs"]
mod projection_test;
#[path = "suite/task_boundary_test.rs"]
mod task_boundary_test;
#[path = "suite/task_snapshot_test.rs"]
mod task_snapshot_test;
#[path = "suite/thread_goal_test.rs"]
mod thread_goal_test;
#[path = "suite/user_content_test.rs"]
mod user_content_test;

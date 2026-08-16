// Single test binary — includes all test modules
#[path = "suite/address_test.rs"]
mod address_test;
#[path = "suite/agent_completion_test.rs"]
mod agent_completion_test;
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
#[path = "suite/permission_action_test.rs"]
mod permission_action_test;
#[path = "suite/permission_decision_audit_test.rs"]
mod permission_decision_audit_test;
#[path = "suite/permission_intent_test.rs"]
mod permission_intent_test;
#[path = "suite/permission_receipt_error_test.rs"]
mod permission_receipt_error_test;
#[path = "suite/permission_receipt_test.rs"]
mod permission_receipt_test;
#[path = "suite/permission_request_test.rs"]
mod permission_request_test;
#[path = "suite/protected_effect_audit_test.rs"]
mod protected_effect_audit_test;
#[path = "suite/protocol_branch_close_test.rs"]
mod protocol_branch_close_test;
#[path = "suite/task_snapshot_test.rs"]
mod task_snapshot_test;
#[path = "suite/thread_goal_test.rs"]
mod thread_goal_test;
#[path = "suite/ui_capabilities_test.rs"]
mod ui_capabilities_test;
#[path = "suite/user_content_test.rs"]
mod user_content_test;
#[path = "suite/workflow_cancel_edge_test.rs"]
mod workflow_cancel_edge_test;
#[path = "suite/workflow_capability_test.rs"]
mod workflow_capability_test;
#[path = "suite/workflow_coverage_test.rs"]
mod workflow_coverage_test;
#[path = "suite/workflow_dependency_budget_test.rs"]
mod workflow_dependency_budget_test;
#[path = "suite/workflow_planner_schema_test.rs"]
mod workflow_planner_schema_test;
#[path = "suite/workflow_planner_test.rs"]
mod workflow_planner_test;
#[path = "suite/workflow_reducer_branch_matrix_test.rs"]
mod workflow_reducer_branch_matrix_test;
#[path = "suite/workflow_reducer_test.rs"]
mod workflow_reducer_test;
#[path = "suite/workflow_regression_test.rs"]
mod workflow_regression_test;
#[path = "suite/workflow_request_ledger_test.rs"]
mod workflow_request_ledger_test;
#[path = "suite/workflow_retry_capacity_test.rs"]
mod workflow_retry_capacity_test;
#[path = "suite/workflow_retry_output_test.rs"]
mod workflow_retry_output_test;
#[path = "suite/workflow_strict_wire_test.rs"]
mod workflow_strict_wire_test;
#[path = "suite/workflow_support.rs"]
mod workflow_support;
#[path = "suite/workflow_terminal_test.rs"]
mod workflow_terminal_test;
#[path = "suite/workflow_validation_branch_test.rs"]
mod workflow_validation_branch_test;
#[path = "suite/workflow_validation_test.rs"]
mod workflow_validation_test;
#[path = "suite/workflow_wire_test.rs"]
mod workflow_wire_test;

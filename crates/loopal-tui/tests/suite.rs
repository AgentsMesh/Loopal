// Single test binary — includes all test modules
#[path = "suite/app_event_edge_test.rs"]
mod app_event_edge_test;
#[path = "suite/app_event_test.rs"]
mod app_event_test;
#[path = "suite/app_test.rs"]
mod app_test;
#[path = "suite/app_tool_edge_test.rs"]
mod app_tool_edge_test;
#[path = "suite/app_tool_test.rs"]
mod app_tool_test;
#[path = "suite/event_forwarding_test.rs"]
mod event_forwarding_test;
#[path = "suite/event_test.rs"]
mod event_test;
#[path = "suite/input_edge_test.rs"]
mod input_edge_test;
#[path = "suite/input_test.rs"]
mod input_test;
#[path = "suite/line_cache_test.rs"]
mod line_cache_test;
#[path = "suite/markdown_code_test.rs"]
mod markdown_code_test;
#[path = "suite/markdown_edge_test.rs"]
mod markdown_edge_test;
#[path = "suite/markdown_table_test.rs"]
mod markdown_table_test;
#[path = "suite/markdown_test.rs"]
mod markdown_test;
#[path = "suite/message_lines_edge_test.rs"]
mod message_lines_edge_test;
#[path = "suite/message_lines_test.rs"]
mod message_lines_test;
#[path = "suite/styled_wrap_test.rs"]
mod styled_wrap_test;

// E2E tests
#[path = "suite/e2e_compact_edge_test.rs"]
mod e2e_compact_edge_test;
#[path = "suite/e2e_compact_test.rs"]
mod e2e_compact_test;
#[path = "suite/e2e_completion_test.rs"]
mod e2e_completion_test;
#[path = "suite/e2e_control_test.rs"]
mod e2e_control_test;
#[path = "suite/e2e_edge_test.rs"]
mod e2e_edge_test;
#[path = "suite/e2e_error_test.rs"]
mod e2e_error_test;
#[path = "suite/e2e_fetch_test.rs"]
mod e2e_fetch_test;
#[path = "suite/e2e_git_test.rs"]
mod e2e_git_test;
#[path = "suite/e2e_harness.rs"]
mod e2e_harness;
#[path = "suite/e2e_hooks_test.rs"]
mod e2e_hooks_test;
#[path = "suite/e2e_loop_test.rs"]
mod e2e_loop_test;
#[path = "suite/e2e_mcp_test.rs"]
mod e2e_mcp_test;
#[path = "suite/e2e_multi_turn_test.rs"]
mod e2e_multi_turn_test;
#[path = "suite/e2e_permission_test.rs"]
mod e2e_permission_test;
#[path = "suite/e2e_session_test.rs"]
mod e2e_session_test;
#[path = "suite/e2e_system_test.rs"]
mod e2e_system_test;
#[path = "suite/e2e_task_test.rs"]
mod e2e_task_test;
#[path = "suite/e2e_test.rs"]
mod e2e_test;
#[path = "suite/e2e_tools_extended_test.rs"]
mod e2e_tools_extended_test;
#[path = "suite/e2e_tools_test.rs"]
mod e2e_tools_test;
#[path = "suite/e2e_worktree_test.rs"]
mod e2e_worktree_test;

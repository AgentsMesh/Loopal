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
#[path = "suite/bg_task_focus_test.rs"]
mod bg_task_focus_test;
#[path = "suite/command_dispatch_test.rs"]
mod command_dispatch_test;
#[path = "suite/command_edge_test.rs"]
mod command_edge_test;
#[path = "suite/command_test.rs"]
mod command_test;
#[path = "suite/cycle_focus_test.rs"]
mod cycle_focus_test;
#[path = "suite/enter_panel_test.rs"]
mod enter_panel_test;
#[path = "suite/event_forwarding_test.rs"]
mod event_forwarding_test;
#[path = "suite/event_test.rs"]
mod event_test;
#[path = "suite/focus_mode_test.rs"]
mod focus_mode_test;
#[path = "suite/focus_panel_keys_test.rs"]
mod focus_panel_keys_test;
#[path = "suite/init_cmd_test.rs"]
mod init_cmd_test;
#[path = "suite/input_edge_test.rs"]
mod input_edge_test;
#[path = "suite/input_scroll_edge_test.rs"]
mod input_scroll_edge_test;
#[path = "suite/input_scroll_test.rs"]
mod input_scroll_test;
#[path = "suite/input_test.rs"]
mod input_test;
#[path = "suite/line_cache_test.rs"]
mod line_cache_test;
#[path = "suite/markdown_code_test.rs"]
mod markdown_code_test;
#[path = "suite/markdown_edge_test.rs"]
mod markdown_edge_test;
#[path = "suite/markdown_elements_test.rs"]
mod markdown_elements_test;
#[path = "suite/markdown_table_test.rs"]
mod markdown_table_test;
#[path = "suite/markdown_test.rs"]
mod markdown_test;
#[path = "suite/mcp_page_keys_test.rs"]
mod mcp_page_keys_test;
#[path = "suite/mcp_page_test.rs"]
mod mcp_page_test;
#[path = "suite/mcp_refresh_test.rs"]
mod mcp_refresh_test;
#[path = "suite/message_lines_edge_test.rs"]
mod message_lines_edge_test;
#[path = "suite/message_lines_test.rs"]
mod message_lines_test;
#[path = "suite/panel_tab_test.rs"]
mod panel_tab_test;
#[path = "suite/render_guard_test.rs"]
mod render_guard_test;
#[path = "suite/scroll_compensation_test.rs"]
mod scroll_compensation_test;
#[path = "suite/scroll_test.rs"]
mod scroll_test;
#[path = "suite/skill_render_test.rs"]
mod skill_render_test;
#[path = "suite/skills_cmd_test.rs"]
mod skills_cmd_test;
#[path = "suite/skills_page_keys_test.rs"]
mod skills_page_keys_test;
#[path = "suite/styled_wrap_test.rs"]
mod styled_wrap_test;
#[path = "suite/tasks_panel_test.rs"]
mod tasks_panel_test;
#[path = "suite/view_switch_panel_lifecycle_test.rs"]
mod view_switch_panel_lifecycle_test;
#[path = "suite/view_switch_test.rs"]
mod view_switch_test;

// E2E tests
#[path = "suite/e2e_compact_edge_test.rs"]
mod e2e_compact_edge_test;
#[path = "suite/e2e_compact_test.rs"]
mod e2e_compact_test;
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
#[path = "suite/e2e_scroll_test.rs"]
mod e2e_scroll_test;
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

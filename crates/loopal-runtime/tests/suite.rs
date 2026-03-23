// Single test binary — includes all test modules
#[path = "agent_loop/mod.rs"]
mod agent_loop;
#[path = "suite/diff_tracker_test.rs"]
mod diff_tracker_test;
#[path = "suite/env_context_test.rs"]
mod env_context_test;
#[path = "suite/frontend_unified_edge_test.rs"]
mod frontend_unified_edge_test;
#[path = "suite/frontend_unified_test.rs"]
mod frontend_unified_test;
#[path = "suite/loop_detector_test.rs"]
mod loop_detector_test;
#[path = "suite/mode_test.rs"]
mod mode_test;
#[path = "suite/permission_test.rs"]
mod permission_test;
#[path = "suite/projection_edge_test.rs"]
mod projection_edge_test;
#[path = "suite/projection_test.rs"]
mod projection_test;
#[path = "suite/rewind_test.rs"]
mod rewind_test;
#[path = "suite/session_manager_test.rs"]
mod session_manager_test;
#[path = "suite/session_test.rs"]
mod session_test;
#[path = "suite/tool_pipeline_hooks_test.rs"]
mod tool_pipeline_hooks_test;
#[path = "suite/tool_pipeline_test.rs"]
mod tool_pipeline_test;

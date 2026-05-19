// Single test binary — includes all test modules
#[path = "agent_loop/mod.rs"]
mod agent_loop;
#[path = "suite/bash_real_subprocess_test.rs"]
mod bash_real_subprocess_test;
#[path = "suite/classifier_permission_handler_support.rs"]
mod classifier_permission_handler_support;
#[path = "suite/classifier_permission_handler_test.rs"]
mod classifier_permission_handler_test;
#[path = "suite/classifier_question_handler_edge_test.rs"]
mod classifier_question_handler_edge_test;
#[path = "suite/classifier_question_handler_support.rs"]
mod classifier_question_handler_support;
#[path = "suite/classifier_question_handler_test.rs"]
mod classifier_question_handler_test;
#[path = "suite/compensation_test.rs"]
mod compensation_test;
#[path = "suite/data_plane_bridge_test.rs"]
mod data_plane_bridge_test;
#[path = "suite/diff_tracker_test.rs"]
mod diff_tracker_test;
#[path = "suite/dispatch_test.rs"]
mod dispatch_test;
#[path = "suite/drain_controls_test.rs"]
mod drain_controls_test;
#[path = "suite/e2e_abstain_full_chain_test.rs"]
mod e2e_abstain_full_chain_test;
#[path = "suite/env_context_test.rs"]
mod env_context_test;
#[path = "suite/frontend_unified_edge_test.rs"]
mod frontend_unified_edge_test;
#[path = "suite/frontend_unified_emit_test.rs"]
mod frontend_unified_emit_test;
#[path = "suite/frontend_unified_permission_test.rs"]
mod frontend_unified_permission_test;
#[path = "suite/frontend_unified_test.rs"]
mod frontend_unified_test;
#[path = "suite/goal_continuation_test.rs"]
mod goal_continuation_test;
#[path = "suite/goal_session_lifecycle_test.rs"]
mod goal_session_lifecycle_test;
#[path = "suite/goal_session_reopen_test.rs"]
mod goal_session_reopen_test;
#[path = "suite/goal_session_support.rs"]
mod goal_session_support;
#[path = "suite/goal_session_test.rs"]
mod goal_session_test;
#[path = "suite/loop_detector_edge_test.rs"]
mod loop_detector_edge_test;
#[path = "suite/loop_detector_test.rs"]
mod loop_detector_test;
#[path = "suite/mode_test.rs"]
mod mode_test;
#[path = "suite/outraced_telemetry_test.rs"]
mod outraced_telemetry_test;
#[path = "suite/permission_test.rs"]
mod permission_test;
#[path = "suite/plan_file_test.rs"]
mod plan_file_test;
#[path = "suite/question_parse_edge_test.rs"]
mod question_parse_edge_test;
#[path = "suite/question_parse_test.rs"]
mod question_parse_test;
#[path = "suite/rewind_test.rs"]
mod rewind_test;
#[path = "suite/sandbox_precheck_test.rs"]
mod sandbox_precheck_test;
#[path = "suite/secrets_pipeline_test.rs"]
mod secrets_pipeline_test;
#[path = "suite/session_manager_test.rs"]
mod session_manager_test;
#[path = "suite/session_test.rs"]
mod session_test;
#[path = "suite/store_closure_invariant_test.rs"]
mod store_closure_invariant_test;
#[path = "suite/tool_pipeline_hooks_test.rs"]
mod tool_pipeline_hooks_test;
#[path = "suite/tool_pipeline_test.rs"]
mod tool_pipeline_test;
#[path = "suite/tracing_sentinel_test.rs"]
mod tracing_sentinel_test;
#[path = "suite/verdict_aggregator_test.rs"]
mod verdict_aggregator_test;

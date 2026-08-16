// Single test binary — includes all test modules
#[path = "agent_loop/mod.rs"]
mod agent_loop;
#[path = "suite/audit_secret_order_test.rs"]
mod audit_secret_order_test;
#[path = "suite/bash_background_secret_test.rs"]
mod bash_background_secret_test;
#[path = "suite/bash_foreground_secret_test.rs"]
mod bash_foreground_secret_test;
#[path = "suite/bash_real_subprocess_test.rs"]
mod bash_real_subprocess_test;
#[path = "suite/bash_secret_support.rs"]
mod bash_secret_support;
#[path = "suite/classifier_permission_handler_happy_test.rs"]
mod classifier_permission_handler_happy_test;
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
#[path = "suite/governance_bridge_test.rs"]
mod governance_bridge_test;
#[path = "suite/hydrate_security_test.rs"]
mod hydrate_security_test;
#[path = "suite/hydrate_support.rs"]
mod hydrate_support;
#[path = "suite/hydrate_test.rs"]
mod hydrate_test;
#[path = "suite/loop_detector_digest_test.rs"]
mod loop_detector_digest_test;
#[path = "suite/loop_detector_edge_test.rs"]
mod loop_detector_edge_test;
#[path = "suite/loop_detector_test.rs"]
mod loop_detector_test;
#[path = "suite/manual_permission_handler_test.rs"]
mod manual_permission_handler_test;
#[path = "suite/mode_test.rs"]
mod mode_test;
#[path = "suite/outraced_telemetry_test.rs"]
mod outraced_telemetry_test;
#[path = "suite/permission_request_support.rs"]
mod permission_request_support;
#[path = "suite/permission_test.rs"]
mod permission_test;
#[path = "suite/plan_file_test.rs"]
mod plan_file_test;
#[path = "suite/post_hook_secret_input_test.rs"]
mod post_hook_secret_input_test;
#[path = "suite/protected_effect_audit_test.rs"]
mod protected_effect_audit_test;
#[path = "suite/protected_effect_read_only_test.rs"]
mod protected_effect_read_only_test;
#[path = "suite/question_parse_edge_test.rs"]
mod question_parse_edge_test;
#[path = "suite/question_parse_test.rs"]
mod question_parse_test;
#[path = "suite/read_image_e2e_test.rs"]
mod read_image_e2e_test;
#[path = "suite/read_image_resource_e2e_test.rs"]
mod read_image_resource_e2e_test;
#[path = "suite/read_image_support.rs"]
mod read_image_support;
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
#[path = "suite/tool_action_integrity_test.rs"]
mod tool_action_integrity_test;
#[path = "suite/tool_effect_boundary_test.rs"]
mod tool_effect_boundary_test;
#[path = "suite/tool_pipeline_hooks_test.rs"]
mod tool_pipeline_hooks_test;
#[path = "suite/tool_pipeline_test.rs"]
mod tool_pipeline_test;
#[path = "suite/tool_result_sink_test.rs"]
mod tool_result_sink_test;
#[path = "suite/tracing_sentinel_test.rs"]
mod tracing_sentinel_test;
#[path = "suite/unresolved_secret_effect_test.rs"]
mod unresolved_secret_effect_test;
#[path = "suite/verdict_aggregator_test.rs"]
mod verdict_aggregator_test;

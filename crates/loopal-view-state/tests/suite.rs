// Single test binary — includes all test modules
#[path = "suite/classifier_status_mutator_test.rs"]
mod classifier_status_mutator_test;
#[path = "suite/compact_banner_mutator_test.rs"]
mod compact_banner_mutator_test;
#[path = "suite/compact_idle_e2e_test.rs"]
mod compact_idle_e2e_test;
#[path = "suite/conversation_serde_test.rs"]
mod conversation_serde_test;
#[path = "suite/decided_mutators_test.rs"]
mod decided_mutators_test;
#[path = "suite/e2e_hub_health_chain_test.rs"]
mod e2e_hub_health_chain_test;
#[path = "suite/e2e_resolve_source_propagation_test.rs"]
mod e2e_resolve_source_propagation_test;
#[path = "suite/free_text_test.rs"]
mod free_text_test;
#[path = "suite/pending_question_nav_test.rs"]
mod pending_question_nav_test;
#[path = "suite/pending_question_test.rs"]
mod pending_question_test;
#[path = "suite/reducer_aggregate_test.rs"]
mod reducer_aggregate_test;
#[path = "suite/reducer_bg_test.rs"]
mod reducer_bg_test;
#[path = "suite/reducer_edge_test.rs"]
mod reducer_edge_test;
#[path = "suite/reducer_lifecycle_test.rs"]
mod reducer_lifecycle_test;
#[path = "suite/reducer_status_test.rs"]
mod reducer_status_test;
#[path = "suite/reducer_tool_test.rs"]
mod reducer_tool_test;
#[path = "suite/server_tool_format_test.rs"]
mod server_tool_format_test;
#[path = "suite/tool_handler_batch_test.rs"]
mod tool_handler_batch_test;
#[path = "suite/tool_handler_edge_test.rs"]
mod tool_handler_edge_test;
#[path = "suite/tool_handler_stale_test.rs"]
mod tool_handler_stale_test;
#[path = "suite/turn_end_reconcile_test.rs"]
mod turn_end_reconcile_test;

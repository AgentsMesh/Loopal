// Single test binary — includes all test modules
#[path = "suite/conversation_serde_test.rs"]
mod conversation_serde_test;
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

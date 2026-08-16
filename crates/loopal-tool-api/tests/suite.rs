// Single test binary — includes all test modules
#[path = "suite/file_read_tracker_test.rs"]
mod file_read_tracker_test;
#[path = "suite/head_tail_test.rs"]
mod head_tail_test;
#[path = "suite/input_normalize_test.rs"]
mod input_normalize_test;
#[path = "suite/output_tail_test.rs"]
mod output_tail_test;
#[path = "suite/overflow_test.rs"]
mod overflow_test;
#[path = "suite/path_test.rs"]
mod path_test;
#[path = "suite/permission_test.rs"]
mod permission_test;
#[path = "suite/provider_resolver_test.rs"]
mod provider_resolver_test;
#[path = "suite/schema_normalize_test.rs"]
mod schema_normalize_test;
#[path = "suite/tool_context_support.rs"]
mod tool_context_support;
#[path = "suite/tool_context_test.rs"]
mod tool_context_test;
#[path = "suite/tool_test.rs"]
mod tool_test;
#[path = "suite/truncate_test.rs"]
mod truncate_test;
#[path = "suite/typed_bridge_integration_test.rs"]
mod typed_bridge_integration_test;
#[path = "suite/typed_bridge_nested_test.rs"]
mod typed_bridge_nested_test;

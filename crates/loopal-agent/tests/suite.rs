// Single test binary — includes all test modules
#[path = "suite/bridge_chain_test.rs"]
mod bridge_chain_test;
#[path = "suite/bridge_child_test.rs"]
mod bridge_child_test;
#[path = "suite/config_test.rs"]
mod config_test;
#[path = "suite/cron_tool_test.rs"]
mod cron_tool_test;
#[path = "suite/task_store_test.rs"]
mod task_store_test;
#[path = "suite/task_tool_test.rs"]
mod task_tool_test;

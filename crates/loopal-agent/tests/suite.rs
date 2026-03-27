// Single test binary — includes all test modules
#[path = "suite/bridge_chain_test.rs"]
mod bridge_chain_test;
#[path = "suite/bridge_child_test.rs"]
mod bridge_child_test;
#[path = "suite/config_test.rs"]
mod config_test;
#[path = "suite/registry_test.rs"]
mod registry_test;
#[path = "suite/router_channel_test.rs"]
mod router_channel_test;
#[path = "suite/router_observe_test.rs"]
mod router_observe_test;
#[path = "suite/router_test.rs"]
mod router_test;
#[path = "suite/task_store_test.rs"]
mod task_store_test;
#[path = "suite/tool_channel_test.rs"]
mod tool_channel_test;
#[path = "suite/tool_send_message_test.rs"]
mod tool_send_message_test;

// Single test binary — includes all test modules
#[path = "suite/agent_isolation_test.rs"]
mod agent_isolation_test;
#[path = "suite/agent_shared_test.rs"]
mod agent_shared_test;
#[path = "suite/agent_snapshot_test.rs"]
mod agent_snapshot_test;
#[path = "suite/bridge_chain_test.rs"]
mod bridge_chain_test;
#[path = "suite/bridge_child_test.rs"]
mod bridge_child_test;
#[path = "suite/config_test.rs"]
mod config_test;
#[path = "suite/cron_tool_test.rs"]
mod cron_tool_test;
#[path = "suite/in_memory_task_storage_test.rs"]
mod in_memory_task_storage_test;
#[path = "suite/send_message_tool_test.rs"]
mod send_message_tool_test;
#[path = "suite/session_resume_adapters_test.rs"]
mod session_resume_adapters_test;
#[path = "suite/spawn_params_test.rs"]
mod spawn_params_test;
#[path = "suite/task_file_storage_test.rs"]
mod task_file_storage_test;
#[path = "suite/task_store_concurrency_test.rs"]
mod task_store_concurrency_test;
#[path = "suite/task_store_switch_session_test.rs"]
mod task_store_switch_session_test;
#[path = "suite/task_store_test.rs"]
mod task_store_test;
#[path = "suite/task_tool_test.rs"]
mod task_tool_test;

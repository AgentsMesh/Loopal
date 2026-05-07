// Single test binary — includes all test modules
#[path = "suite/entry_test.rs"]
mod entry_test;
#[path = "suite/goal_store_test.rs"]
mod goal_store_test;
#[path = "suite/messages_test.rs"]
mod messages_test;
#[path = "suite/replay_test.rs"]
mod replay_test;
#[path = "suite/sessions_cwd_test.rs"]
mod sessions_cwd_test;
#[path = "suite/sessions_test.rs"]
mod sessions_test;
#[path = "suite/sessions_update_test.rs"]
mod sessions_update_test;
#[path = "suite/sub_agent_ref_test.rs"]
mod sub_agent_ref_test;

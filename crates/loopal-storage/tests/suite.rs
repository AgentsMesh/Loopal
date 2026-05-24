// Single test binary — includes all test modules
#[path = "suite/goal_store_test.rs"]
mod goal_store_test;
#[path = "suite/resources_test.rs"]
mod resources_test;
#[path = "suite/sessions_cwd_test.rs"]
mod sessions_cwd_test;
#[path = "suite/sessions_test.rs"]
mod sessions_test;
#[path = "suite/sessions_update_test.rs"]
mod sessions_update_test;
#[path = "suite/sub_agent_ref_test.rs"]
mod sub_agent_ref_test;
#[path = "suite/turn_event_store_test.rs"]
mod turn_event_store_test;

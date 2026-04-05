// Single test binary — includes all test modules
#[path = "suite/agent_handler_edge_test.rs"]
mod agent_handler_edge_test;
#[path = "suite/agent_handler_test.rs"]
mod agent_handler_test;
#[path = "suite/agent_lifecycle_test.rs"]
mod agent_lifecycle_test;
#[path = "suite/agent_routing_test.rs"]
mod agent_routing_test;
#[path = "suite/controller_async_test.rs"]
mod controller_async_test;
#[path = "suite/controller_test.rs"]
mod controller_test;
#[path = "suite/event_handler_edge_test.rs"]
mod event_handler_edge_test;
#[path = "suite/event_handler_test.rs"]
mod event_handler_test;
#[path = "suite/is_idle_test.rs"]
mod is_idle_test;
#[path = "suite/message_log_test.rs"]
mod message_log_test;
#[path = "suite/projection_convert_test.rs"]
mod projection_convert_test;
#[path = "suite/resume_display_test.rs"]
mod resume_display_test;
#[path = "suite/resume_test.rs"]
mod resume_test;
#[path = "suite/retry_banner_test.rs"]
mod retry_banner_test;
#[path = "suite/rewind_test.rs"]
mod rewind_test;
#[path = "suite/topology_test.rs"]
mod topology_test;
#[path = "suite/user_display_test.rs"]
mod user_display_test;

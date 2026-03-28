// Single test binary — includes all test modules
#[path = "suite/event_router_test.rs"]
mod event_router_test;
#[path = "suite/dispatch_test.rs"]
mod dispatch_test;
#[path = "suite/relay_test.rs"]
mod relay_test;
#[path = "suite/hub_integration_test.rs"]
mod hub_integration_test;
#[path = "suite/hub_lifecycle_test.rs"]
mod hub_lifecycle_test;
#[path = "suite/spawn_lifecycle_test.rs"]
mod spawn_lifecycle_test;
#[path = "suite/e2e_bootstrap_test.rs"]
mod e2e_bootstrap_test;
#[path = "suite/multi_agent_test.rs"]
mod multi_agent_test;
#[path = "suite/advanced_scenarios_test.rs"]
mod advanced_scenarios_test;
#[path = "suite/wait_nonblocking_test.rs"]
mod wait_nonblocking_test;
#[path = "suite/completion_output_test.rs"]
mod completion_output_test;
#[path = "suite/race_condition_test.rs"]
mod race_condition_test;
#[path = "suite/collaboration_test.rs"]
mod collaboration_test;

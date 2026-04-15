// Single test binary — includes all test modules

#[path = "suite/background_task_edge_test.rs"]
mod background_task_edge_test;
#[path = "suite/background_task_test.rs"]
mod background_task_test;
#[path = "suite/snapshot_test.rs"]
mod snapshot_test;
#[path = "suite/spawn_notification_test.rs"]
mod spawn_notification_test;

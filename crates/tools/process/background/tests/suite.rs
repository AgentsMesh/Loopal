// Single test binary — includes all test modules

/// Global mutex for tests that clear/populate the shared background store.
pub(crate) static BG_STORE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[path = "suite/background_task_edge_test.rs"]
mod background_task_edge_test;
#[path = "suite/background_task_test.rs"]
mod background_task_test;
#[path = "suite/snapshot_test.rs"]
mod snapshot_test;

// Single test binary — includes all test modules

#[path = "suite/test_support.rs"]
mod test_support;

#[path = "suite/background_task_edge_test.rs"]
mod background_task_edge_test;
#[path = "suite/background_task_test.rs"]
mod background_task_test;
#[path = "suite/gc_evict_test.rs"]
mod gc_evict_test;
#[path = "suite/monitor_test.rs"]
mod monitor_test;
#[path = "suite/ops_defensive_test.rs"]
mod ops_defensive_test;
#[path = "suite/ops_status_test.rs"]
mod ops_status_test;
#[path = "suite/process_render_test.rs"]
mod process_render_test;
#[path = "suite/render_preview_test.rs"]
mod render_preview_test;
#[path = "suite/snapshot_test.rs"]
mod snapshot_test;
#[path = "suite/spawn_notification_test.rs"]
mod spawn_notification_test;
#[path = "suite/timeout_streaming_test.rs"]
mod timeout_streaming_test;

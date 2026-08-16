// Single test binary for loopal-backend
#[path = "suite/approved_paths_test.rs"]
mod approved_paths_test;
#[path = "suite/atomic_write_test.rs"]
mod atomic_write_test;
#[path = "suite/batch_test.rs"]
mod batch_test;
#[path = "suite/command_log_test.rs"]
mod command_log_test;
#[path = "suite/command_timeout_test.rs"]
mod command_timeout_test;
#[path = "suite/fetch_headers_test.rs"]
mod fetch_headers_test;
#[path = "suite/glob_parallel_test.rs"]
mod glob_parallel_test;
#[path = "suite/image_test.rs"]
mod image_test;
#[path = "suite/local_process_coverage_test.rs"]
mod local_process_coverage_test;
#[path = "suite/log_file_test.rs"]
mod log_file_test;
#[path = "suite/log_file_test_support.rs"]
mod log_file_test_support;
#[path = "suite/log_guard_support.rs"]
mod log_guard_support;
#[path = "suite/log_guard_test.rs"]
mod log_guard_test;
#[path = "suite/log_permissions_test.rs"]
mod log_permissions_test;
#[path = "suite/path_approval_test.rs"]
mod path_approval_test;
#[path = "suite/process_group_test.rs"]
mod process_group_test;
#[path = "suite/process_group_windows_test.rs"]
mod process_group_windows_test;
#[path = "suite/process_test_support.rs"]
mod process_test_support;
#[path = "suite/resolve_checked_test.rs"]
mod resolve_checked_test;
#[path = "suite/search_coverage_test.rs"]
mod search_coverage_test;
#[path = "suite/search_timeout_test.rs"]
mod search_timeout_test;
#[path = "suite/secret_env_test.rs"]
mod secret_env_test;
#[path = "suite/tmp_cleanup_test.rs"]
mod tmp_cleanup_test;

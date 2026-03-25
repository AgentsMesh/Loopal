// Single test binary — includes all ACP test modules
#[path = "suite/e2e_cancel_test.rs"]
mod e2e_cancel_test;
#[path = "suite/e2e_error_test.rs"]
mod e2e_error_test;
#[path = "suite/e2e_harness.rs"]
mod e2e_harness;
#[path = "suite/e2e_lifecycle_test.rs"]
mod e2e_lifecycle_test;
#[path = "suite/e2e_multi_test.rs"]
mod e2e_multi_test;
#[path = "suite/e2e_protocol_test.rs"]
mod e2e_protocol_test;
#[path = "suite/e2e_session_test.rs"]
mod e2e_session_test;

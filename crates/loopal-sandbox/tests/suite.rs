// Single test binary — includes all test modules
#[path = "suite/command_checker_test.rs"]
mod command_checker_test;
#[path = "suite/command_wrapper_test.rs"]
mod command_wrapper_test;
#[path = "suite/env_sanitizer_test.rs"]
mod env_sanitizer_test;
#[path = "suite/network_test.rs"]
mod network_test;
#[path = "suite/path_checker_edge_test.rs"]
mod path_checker_edge_test;
#[path = "suite/path_checker_test.rs"]
mod path_checker_test;
#[path = "suite/platform_linux_test.rs"]
mod platform_linux_test;
#[path = "suite/platform_macos_test.rs"]
mod platform_macos_test;
#[path = "suite/policy_test.rs"]
mod policy_test;
#[path = "suite/scanner_test.rs"]
mod scanner_test;
#[path = "suite/security_inspector_test.rs"]
mod security_inspector_test;

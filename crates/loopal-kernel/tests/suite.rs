// Single test binary — includes all test modules
#[path = "suite/kernel_anthropic_test.rs"]
mod kernel_anthropic_test;
#[path = "suite/kernel_google_test.rs"]
mod kernel_google_test;
#[path = "suite/kernel_init_test.rs"]
mod kernel_init_test;
#[path = "suite/kernel_mcp_test.rs"]
mod kernel_mcp_test;
#[path = "suite/kernel_openai_test.rs"]
mod kernel_openai_test;
#[path = "suite/kernel_provider_registry_test.rs"]
mod kernel_provider_registry_test;
#[path = "suite/kernel_sandbox_test.rs"]
mod kernel_sandbox_test;

pub mod command_checker;
pub mod command_wrapper;
pub mod decorator;
pub mod env_sanitizer;
pub mod network;
pub mod path_checker;
pub mod platform;
pub mod policy;
pub mod sensitive_patterns;

// Internal modules — used by decorator/bash_executor only.
pub(crate) mod bash_executor;

// scanner is not yet integrated into the decorator pipeline.
#[doc(hidden)]
pub mod scanner;

// Public API — only types needed by kernel to bootstrap sandboxing.
pub use decorator::SandboxedTool;
pub use policy::resolve_policy;

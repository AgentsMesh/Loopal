// Single test binary — includes all test modules
#[path = "suite/build_secret_store_test.rs"]
mod build_secret_store_test;
#[path = "suite/classifier_prompt_test.rs"]
mod classifier_prompt_test;
#[path = "suite/classifier_timeout_test.rs"]
mod classifier_timeout_test;
#[path = "suite/config_test.rs"]
mod config_test;
#[path = "suite/hook_test.rs"]
mod hook_test;
#[path = "suite/loader_instructions_test.rs"]
mod loader_instructions_test;
#[path = "suite/loader_settings_merge_test.rs"]
mod loader_settings_merge_test;
#[path = "suite/loader_settings_test.rs"]
mod loader_settings_test;
#[path = "suite/loader_unit_test.rs"]
mod loader_unit_test;
#[path = "suite/local_writer_test.rs"]
mod local_writer_test;
#[path = "suite/locations_test.rs"]
mod locations_test;
#[path = "suite/mcp_json_test.rs"]
mod mcp_json_test;
#[path = "suite/plugin_test.rs"]
mod plugin_test;
#[path = "suite/resolver_edge_test.rs"]
mod resolver_edge_test;
#[path = "suite/resolver_hooks_test.rs"]
mod resolver_hooks_test;
#[path = "suite/resolver_test.rs"]
mod resolver_test;
#[path = "suite/settings_routing_test.rs"]
mod settings_routing_test;
#[path = "suite/skills_loader_test.rs"]
mod skills_loader_test;
#[path = "suite/skills_parser_test.rs"]
mod skills_parser_test;
#[path = "suite/telemetry_edge_test.rs"]
mod telemetry_edge_test;
#[path = "suite/telemetry_test.rs"]
mod telemetry_test;
#[path = "suite/validate_test.rs"]
mod validate_test;

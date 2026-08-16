#[path = "suite/ssh_fixtures.rs"]
mod ssh_fixtures;
#[path = "suite/store_fixtures.rs"]
mod store_fixtures;
#[path = "suite/vault_fixtures.rs"]
mod vault_fixtures;

#[path = "suite/audit_failure_test.rs"]
mod audit_failure_test;
#[path = "suite/concurrent_lock_test.rs"]
mod concurrent_lock_test;
#[path = "suite/discovery_test.rs"]
mod discovery_test;
#[path = "suite/editor_test.rs"]
mod editor_test;
#[path = "suite/identity_test.rs"]
mod identity_test;
#[path = "suite/multi_vault_e2e_test.rs"]
mod multi_vault_e2e_test;
#[path = "suite/recipients_test.rs"]
mod recipients_test;
#[path = "suite/ssh_agent_test.rs"]
mod ssh_agent_test;
#[path = "suite/store_edge_test.rs"]
mod store_edge_test;
#[path = "suite/store_test.rs"]
mod store_test;
#[path = "suite/vault_name_test.rs"]
mod vault_name_test;

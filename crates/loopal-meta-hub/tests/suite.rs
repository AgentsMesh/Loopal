#[path = "suite/test_helpers.rs"]
mod test_helpers;

#[path = "suite/address_test.rs"]
mod address_test;
#[path = "suite/e2e_tcp_test.rs"]
mod e2e_tcp_test;
#[path = "suite/forward_timeout_test.rs"]
mod forward_timeout_test;
#[path = "suite/hub_lifecycle_test.rs"]
mod hub_lifecycle_test;
#[path = "suite/io_lifecycle_test.rs"]
mod io_lifecycle_test;
#[path = "suite/nat_roundtrip_test.rs"]
mod nat_roundtrip_test;
#[path = "suite/nat_routing_test.rs"]
mod nat_routing_test;
#[path = "suite/nat_spawn_completion_test.rs"]
mod nat_spawn_completion_test;
#[path = "suite/relay_event_test.rs"]
mod relay_event_test;
#[path = "suite/remote_interrupt_test.rs"]
mod remote_interrupt_test;
#[path = "suite/remote_relay_test.rs"]
mod remote_relay_test;
#[path = "suite/routing_test.rs"]
mod routing_test;
#[path = "suite/server_lifecycle_test.rs"]
mod server_lifecycle_test;
#[path = "suite/shadow_lifecycle_test.rs"]
mod shadow_lifecycle_test;
#[path = "suite/shadow_routing_test.rs"]
mod shadow_routing_test;
#[path = "suite/spawn_completion_test.rs"]
mod spawn_completion_test;
#[path = "suite/spawn_edge_test.rs"]
mod spawn_edge_test;
#[path = "suite/spawn_schema_test.rs"]
mod spawn_schema_test;
#[path = "suite/status_resolve_test.rs"]
mod status_resolve_test;
#[path = "suite/topology_aggregate_test.rs"]
mod topology_aggregate_test;

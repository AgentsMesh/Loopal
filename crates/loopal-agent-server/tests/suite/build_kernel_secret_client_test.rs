use loopal_agent_server::params::build_kernel_from_config;
use loopal_config::ConfigResolver;
use loopal_ipc::connection::Connection;

#[tokio::test]
async fn hub_connection_installs_secret_client_without_placeholders() {
    let (client_transport, _hub_transport) = loopal_ipc::duplex_pair();
    let (connection, _incoming) = Connection::new(client_transport).into_listening();
    let config = ConfigResolver::new().resolve().unwrap();
    let kernel = build_kernel_from_config(
        &config,
        false,
        0,
        None,
        Some(connection),
        std::path::PathBuf::from("."),
        "test".to_string(),
        "secret-session".to_string(),
    )
    .await
    .unwrap();

    assert!(kernel.secret_client().is_some());
}

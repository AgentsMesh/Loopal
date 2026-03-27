//! Tests for server_info: write/read/remove lifecycle.
//! All in one test to avoid concurrent PID-file conflicts.

use loopal_agent_server::server_info;

#[test]
fn server_info_lifecycle() {
    // Clean slate
    server_info::remove_server_info();

    // Read non-existent → error
    assert!(server_info::read_server_info(std::process::id()).is_err());

    // Write → read roundtrip
    server_info::write_server_info(9527, "test-token-abc").unwrap();
    let info = server_info::read_server_info(std::process::id()).unwrap();
    assert_eq!(info.port, 9527);
    assert_eq!(info.token, "test-token-abc");
    assert_eq!(info.pid, std::process::id());

    // Remove → gone
    server_info::remove_server_info();
    assert!(server_info::read_server_info(std::process::id()).is_err());

    // Read truly non-existent PID
    assert!(server_info::read_server_info(1).is_err());
}

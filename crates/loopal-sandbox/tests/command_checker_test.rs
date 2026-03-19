use loopal_sandbox::command_checker::check_command;
use loopal_types::sandbox::CommandDecision;

#[test]
fn empty_command_allowed() {
    assert_eq!(check_command(""), CommandDecision::Allow);
    assert_eq!(check_command("   "), CommandDecision::Allow);
}

#[test]
fn normal_commands_allowed() {
    assert_eq!(check_command("ls -la"), CommandDecision::Allow);
    assert_eq!(check_command("cargo build"), CommandDecision::Allow);
    assert_eq!(check_command("git status"), CommandDecision::Allow);
    assert_eq!(check_command("echo hello"), CommandDecision::Allow);
}

#[test]
fn rm_rf_root_blocked() {
    assert!(matches!(
        check_command("rm -rf /"),
        CommandDecision::Deny(_)
    ));
    assert!(matches!(
        check_command("rm -rf /*"),
        CommandDecision::Deny(_)
    ));
}

#[test]
fn rm_rf_home_blocked() {
    assert!(matches!(
        check_command("rm -rf ~"),
        CommandDecision::Deny(_)
    ));
}

#[test]
fn fork_bomb_blocked() {
    assert!(matches!(
        check_command(":(){ :|:& };:"),
        CommandDecision::Deny(_)
    ));
}

#[test]
fn device_write_blocked() {
    assert!(matches!(
        check_command("dd if=/dev/zero of=/dev/sda"),
        CommandDecision::Deny(_)
    ));
}

#[test]
fn mkfs_blocked() {
    assert!(matches!(
        check_command("mkfs.ext4 /dev/sda1"),
        CommandDecision::Deny(_)
    ));
}

#[test]
fn safe_rm_allowed() {
    // Regular rm on specific files is allowed
    assert_eq!(
        check_command("rm target/debug/test.o"),
        CommandDecision::Allow
    );
    assert_eq!(
        check_command("rm -f ./build/output.log"),
        CommandDecision::Allow
    );
}

#[test]
fn sudo_rm_rf_system_dirs_blocked() {
    assert!(matches!(
        check_command("sudo rm -rf /usr"),
        CommandDecision::Deny(_)
    ));
    assert!(matches!(
        check_command("sudo rm -rf /etc"),
        CommandDecision::Deny(_)
    ));
}

#[test]
fn shutdown_blocked() {
    assert!(matches!(
        check_command("shutdown -h now"),
        CommandDecision::Deny(_)
    ));
}

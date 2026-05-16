use loopal_vault_age::{RecipientEntry, Recipients};
use loopal_vault_api::VaultError;
use tempfile::tempdir;

const ED25519_PK_ALICE: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN alice@rust";
const ED25519_PK_NO_COMMENT: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN";

#[test]
fn parse_entry_extracts_comment_as_label() {
    let entry = RecipientEntry::parse(ED25519_PK_ALICE).unwrap();
    assert_eq!(entry.label, "alice@rust");
}

#[test]
fn parse_entry_falls_back_to_pubkey_prefix_when_no_comment() {
    let entry = RecipientEntry::parse(ED25519_PK_NO_COMMENT).unwrap();
    assert!(entry.label.starts_with("pk:"));
    assert_eq!(entry.label.len(), "pk:".len() + 8);
}

#[test]
fn parse_entry_rejects_malformed() {
    match RecipientEntry::parse("not a key") {
        Err(VaultError::InvalidRecipient(_)) => {}
        other => panic!("expected InvalidRecipient, got {other:?}"),
    }
}

#[test]
fn parse_file_preserves_comments_and_blank_lines() {
    let content = format!(
        "# team recipients\n{ED25519_PK_ALICE}\n\n# bob is offline\n{ED25519_PK_NO_COMMENT}\n",
    );
    let rec = Recipients::parse(&content).unwrap();
    let entries = rec.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].label, "alice@rust");
}

#[test]
fn round_trip_preserves_layout() {
    let content = format!("# header\n\n{ED25519_PK_ALICE}\n# another\n{ED25519_PK_NO_COMMENT}\n",);
    let dir = tempdir().unwrap();
    let path = dir.path().join(".age-recipients");

    let rec = Recipients::parse(&content).unwrap();
    rec.write(&path).unwrap();
    let actual = std::fs::read_to_string(&path).unwrap();
    assert!(actual.contains("# header"));
    assert!(actual.contains("# another"));
    assert!(actual.contains("alice@rust"));
}

#[test]
fn load_returns_empty_when_file_missing() {
    let dir = tempdir().unwrap();
    let rec = Recipients::load(&dir.path().join("nope")).unwrap();
    assert!(rec.is_empty());
}

#[test]
fn add_line_appends() {
    let mut rec = Recipients::new();
    rec.add_line(ED25519_PK_ALICE).unwrap();
    assert_eq!(rec.entries().len(), 1);
}

#[test]
fn add_line_rejects_malformed() {
    let mut rec = Recipients::new();
    match rec.add_line("garbage line") {
        Err(VaultError::InvalidRecipient(_)) => {}
        other => panic!("expected InvalidRecipient, got {other:?}"),
    }
    assert!(rec.is_empty());
}

#[test]
fn remove_by_label_removes_matching_entry() {
    let mut rec = Recipients::new();
    rec.add_line(ED25519_PK_ALICE).unwrap();
    rec.add_line(ED25519_PK_NO_COMMENT).unwrap();
    rec.remove_by_label("alice@rust").unwrap();
    let entries = rec.entries();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].label.starts_with("pk:"));
}

#[test]
fn remove_by_label_returns_not_found_when_unknown() {
    let mut rec = Recipients::new();
    rec.add_line(ED25519_PK_ALICE).unwrap();
    match rec.remove_by_label("ghost") {
        Err(VaultError::RecipientNotFound(l)) => assert_eq!(l, "ghost"),
        other => panic!("expected RecipientNotFound, got {other:?}"),
    }
}

#[test]
fn remove_by_pubkey_prefix_matches() {
    let mut rec = Recipients::new();
    rec.add_line(ED25519_PK_ALICE).unwrap();
    rec.remove_by_label("AAAAC3NzaC1lZDI1NTE5").unwrap();
    assert!(rec.is_empty());
}

#[cfg(unix)]
#[test]
fn write_sets_mode_0644() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let path = dir.path().join(".age-recipients");
    let mut rec = Recipients::new();
    rec.add_line(ED25519_PK_ALICE).unwrap();
    rec.write(&path).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o644);
}

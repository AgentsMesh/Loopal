//! `validate_vault_name` covers the path-traversal defense for `loopal vault --name <name>`
//! (and the legacy `vault@<name>` form normalized to it).
//! Anything not matching `^[a-z][a-z0-9_-]*$` must be rejected.

use loopal_vault_age::cli::validate_vault_name;
use loopal_vault_api::VaultError;

fn ok(name: &str) {
    assert!(validate_vault_name(name).is_ok(), "should accept {name:?}");
}

fn bad(name: &str) {
    let err = validate_vault_name(name).unwrap_err();
    assert!(
        matches!(err, VaultError::InvalidVaultName(_)),
        "{name:?} should be rejected as InvalidVaultName, got {err:?}"
    );
}

#[test]
fn accepts_simple_lowercase_names() {
    ok("default");
    ok("production");
    ok("staging");
    ok("a");
}

#[test]
fn accepts_names_with_digits_and_underscores_after_letter() {
    ok("env1");
    ok("project_x");
    ok("a1b2c3");
    ok("personal_dev_2024");
}

#[test]
fn accepts_names_with_dashes() {
    ok("staging-eu");
    ok("us-east-1");
    ok("p-r-o-d");
}

#[test]
fn rejects_uppercase() {
    bad("Default");
    bad("Production");
    bad("ABC");
}

#[test]
fn rejects_digit_start() {
    bad("1prod");
    bad("2024-prod");
    bad("9");
}

#[test]
fn rejects_dash_or_underscore_start() {
    bad("-prod");
    bad("_internal");
}

#[test]
fn rejects_path_traversal_attempts() {
    // These are the actual path-traversal vectors the input layer must block.
    bad("..");
    bad("../foo");
    bad("../../etc/passwd");
    bad("foo/bar");
    bad("foo\\bar");
    bad("./bar");
    bad("/abs/path");
}

#[test]
fn rejects_special_chars_and_whitespace() {
    bad("name with space");
    bad("name.with.dot");
    bad("name@vault");
    bad("name:foo");
    bad("name;rm");
    bad("name\nfoo");
    bad("name\tfoo");
}

#[test]
fn rejects_empty() {
    bad("");
}

#[test]
fn rejects_unicode_letters() {
    bad("默认");
    bad("café");
    bad("\u{200b}default"); // zero-width space prefix
}

use super::truncate_cmd_for_desc;

#[test]
fn command_description_normalizes_whitespace_and_preserves_short_input() {
    assert_eq!(
        truncate_cmd_for_desc("  printf   safe  ", 60),
        "printf safe"
    );
}

#[test]
fn command_description_truncates_ascii_at_the_requested_boundary() {
    assert_eq!(truncate_cmd_for_desc("abcdefgh", 5), "abcd…");
    assert_eq!(truncate_cmd_for_desc("abcdefgh", 0), "…");
}

#[test]
fn command_description_never_splits_a_utf8_code_point() {
    assert_eq!(truncate_cmd_for_desc("abcéfg", 5), "abc…");
}

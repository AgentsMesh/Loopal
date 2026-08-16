use super::*;

#[test]
fn missing_absolute_executable_falls_through_without_path_rewrite() {
    let missing = support::missing_path();
    assert!(missing.is_absolute());
    assert_eq!(
        AgentProcess::select_executable(missing.to_str().unwrap(), None, None),
        missing
    );
}

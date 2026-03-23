use loopal_runtime::agent_loop::env_context::build_env_section;
use std::path::Path;

#[test]
fn contains_date_line() {
    let section = build_env_section(Path::new("/tmp"), 0, 50);
    assert!(section.contains("- Date:"), "should contain date");
}

#[test]
fn contains_working_directory() {
    let section = build_env_section(Path::new("/tmp/my-project"), 0, 50);
    assert!(
        section.contains("/tmp/my-project"),
        "should contain cwd path"
    );
}

#[test]
fn contains_turn_progress() {
    let section = build_env_section(Path::new("/tmp"), 3, 50);
    assert!(
        section.contains("- Turn: 3/50"),
        "should contain turn progress"
    );
}

#[test]
fn starts_with_environment_header() {
    let section = build_env_section(Path::new("/tmp"), 0, 50);
    assert!(
        section.contains("# Environment"),
        "should have Environment header"
    );
}

#[test]
fn zero_turn_max_turns() {
    let section = build_env_section(Path::new("/tmp"), 0, 0);
    assert!(section.contains("- Turn: 0/0"));
}

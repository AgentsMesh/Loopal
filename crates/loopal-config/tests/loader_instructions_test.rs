use loopal_config::load_instructions;
use tempfile::TempDir;

#[test]
fn test_load_instructions() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("LOOPAL.md"), "# Project Instructions").unwrap();

    let instructions = load_instructions(tmp.path()).unwrap();
    assert!(instructions.contains("Project Instructions"));
}

#[test]
fn test_load_instructions_empty() {
    let tmp = TempDir::new().unwrap();
    let instructions = load_instructions(tmp.path()).unwrap();
    assert!(instructions.is_empty());
}

#[test]
fn test_load_instructions_project_only() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("LOOPAL.md"), "# Project\nDo stuff.").unwrap();

    let instructions = load_instructions(tmp.path()).unwrap();
    assert!(instructions.contains("Project"));
    assert!(instructions.contains("Do stuff."));
}

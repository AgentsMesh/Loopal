use loopal_config::load_config;
use tempfile::TempDir;

#[test]
fn test_load_instructions() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("LOOPAL.md"), "# Project Instructions").unwrap();

    let instructions = load_config(tmp.path()).unwrap().instructions;
    assert!(instructions.contains("Project Instructions"));
}

#[test]
fn test_load_instructions_empty() {
    let tmp = TempDir::new().unwrap();
    let instructions = load_config(tmp.path()).unwrap().instructions;
    assert!(instructions.is_empty());
}

#[test]
fn test_load_instructions_project_only() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("LOOPAL.md"), "# Project\nDo stuff.").unwrap();

    let instructions = load_config(tmp.path()).unwrap().instructions;
    assert!(instructions.contains("Project"));
    assert!(instructions.contains("Do stuff."));
}

#[test]
fn test_load_instructions_local_md_appended() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".loopal");
    std::fs::create_dir_all(&config_dir).unwrap();

    // Project instructions
    std::fs::write(tmp.path().join("LOOPAL.md"), "# Project").unwrap();
    // Project local instructions
    std::fs::write(config_dir.join("LOOPAL.local.md"), "# Local Override").unwrap();

    let instructions = load_config(tmp.path()).unwrap().instructions;
    assert!(
        instructions.contains("# Project"),
        "project instructions should be present"
    );
    assert!(
        instructions.contains("# Local Override"),
        "local override should be appended"
    );
}

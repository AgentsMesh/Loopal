use std::path::Path;

use loopal_config::{
    global_config_dir, global_instructions_path, global_settings_path, project_config_dir,
    project_instructions_path, project_local_settings_path, project_settings_path,
};

#[test]
fn test_project_config_dir() {
    let cwd = Path::new("/home/user/project");
    let result = project_config_dir(cwd);
    assert_eq!(result, Path::new("/home/user/project/.loopal"));
}

#[test]
fn test_project_settings_path() {
    let cwd = Path::new("/home/user/project");
    let result = project_settings_path(cwd);
    assert_eq!(
        result,
        Path::new("/home/user/project/.loopal/settings.json")
    );
}

#[test]
fn test_project_local_settings_path() {
    let cwd = Path::new("/home/user/project");
    let result = project_local_settings_path(cwd);
    assert_eq!(
        result,
        Path::new("/home/user/project/.loopal/settings.local.json")
    );
}

#[test]
fn test_project_instructions_path() {
    let cwd = Path::new("/home/user/project");
    let result = project_instructions_path(cwd);
    assert_eq!(result, Path::new("/home/user/project/LOOPAL.md"));
}

#[test]
fn test_global_config_dir_returns_home_based_path() {
    // This test depends on the home directory being available in the test environment.
    // It should pass in any standard environment.
    let result = global_config_dir();
    assert!(result.is_ok());
    let path = result.unwrap();
    assert!(path.ends_with(".loopal"));
}

#[test]
fn test_global_settings_path_returns_json_file() {
    let result = global_settings_path();
    assert!(result.is_ok());
    let path = result.unwrap();
    assert!(path.ends_with("settings.json"));
    assert!(path.to_string_lossy().contains(".loopal"));
}

#[test]
fn test_global_instructions_path_returns_md_file() {
    let result = global_instructions_path();
    assert!(result.is_ok());
    let path = result.unwrap();
    assert!(path.ends_with("LOOPAL.md"));
    assert!(path.to_string_lossy().contains(".loopal"));
}

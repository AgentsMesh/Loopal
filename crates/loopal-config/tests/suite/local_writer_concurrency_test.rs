use loopal_config::{
    project_local_settings_path, update_local_settings_field, update_local_settings_fields,
};
use tempfile::TempDir;

#[test]
fn concurrent_updates_do_not_lose_independent_fields() {
    let tmp = TempDir::new().unwrap();
    let root = std::sync::Arc::new(tmp.path().to_path_buf());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(12));
    let threads: Vec<_> = (0..12)
        .map(|index| {
            let root = root.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                update_local_settings_fields(
                    &root,
                    [(
                        format!("desktop_test.field_{index}"),
                        serde_json::json!(index),
                    )],
                )
                .unwrap();
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }

    let text = std::fs::read_to_string(project_local_settings_path(&root)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    for index in 0..12 {
        assert_eq!(value["desktop_test"][format!("field_{index}")], index);
    }
    let entries = std::fs::read_dir(tmp.path().join(".loopal")).unwrap();
    assert!(
        entries
            .filter_map(Result::ok)
            .all(|entry| !entry.file_name().to_string_lossy().contains(".tmp."))
    );
}

#[cfg(unix)]
#[test]
fn settings_and_writer_lock_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    let path = project_local_settings_path(tmp.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{\"model\":\"before\"}").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    update_local_settings_field(tmp.path(), "model", serde_json::json!("private")).unwrap();
    assert_eq!(mode(&path), 0o600);
    assert_eq!(
        mode(&path.parent().unwrap().join(".settings.local.json.lock")),
        0o600
    );

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
    update_local_settings_field(tmp.path(), "model", serde_json::json!("stricter")).unwrap();
    assert_eq!(mode(&path), 0o400);
}

#[cfg(unix)]
fn mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

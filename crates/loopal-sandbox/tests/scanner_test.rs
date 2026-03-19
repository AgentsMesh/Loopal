use std::fs;

use loopal_sandbox::scanner::scan_sensitive_files;

#[test]
fn finds_env_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    fs::write(tmp.path().join(".env"), "SECRET=value").unwrap();
    fs::write(tmp.path().join("safe.txt"), "hello").unwrap();

    let results = scan_sensitive_files(tmp.path(), 3, 100);
    assert!(results.contains(&".env".to_string()));
    assert!(!results.contains(&"safe.txt".to_string()));
}

#[test]
fn finds_key_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let ssh_dir = tmp.path().join(".ssh");
    fs::create_dir(&ssh_dir).unwrap();
    fs::write(ssh_dir.join("id_rsa"), "private key").unwrap();
    fs::write(tmp.path().join("cert.pem"), "cert data").unwrap();

    let results = scan_sensitive_files(tmp.path(), 3, 100);
    assert!(results.iter().any(|r| r.contains("id_rsa")));
    assert!(results.contains(&"cert.pem".to_string()));
}

#[test]
fn respects_max_depth() {
    let tmp = tempfile::TempDir::new().unwrap();
    let deep = tmp.path().join("a/b/c/d");
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join(".env"), "deep").unwrap();
    fs::write(tmp.path().join(".env"), "shallow").unwrap();

    // max_depth=1 should only find the root .env
    let results = scan_sensitive_files(tmp.path(), 1, 100);
    assert!(results.contains(&".env".to_string()));
    assert_eq!(results.len(), 1);
}

#[test]
fn respects_max_results() {
    let tmp = tempfile::TempDir::new().unwrap();
    for i in 0..10 {
        fs::write(tmp.path().join(format!(".env.{i}")), "secret").unwrap();
    }

    let results = scan_sensitive_files(tmp.path(), 3, 3);
    assert_eq!(results.len(), 3);
}

#[test]
fn empty_directory_returns_empty() {
    let tmp = tempfile::TempDir::new().unwrap();
    let results = scan_sensitive_files(tmp.path(), 3, 100);
    assert!(results.is_empty());
}

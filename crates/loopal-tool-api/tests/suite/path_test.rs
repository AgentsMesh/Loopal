use loopal_tool_api::ResolvedPath;

#[test]
fn from_backend_resolved_round_trips_path() {
    let p = std::path::PathBuf::from("/tmp/foo");
    let rp = ResolvedPath::from_backend_resolved(p.clone());
    assert_eq!(rp.as_path(), p.as_path());
}

#[test]
fn as_str_returns_path_string() {
    let rp = ResolvedPath::from_backend_resolved(std::path::PathBuf::from("/tmp/bar"));
    assert_eq!(rp.as_str().as_ref(), "/tmp/bar");
}

#[test]
fn into_path_buf_consumes_self() {
    let p = std::path::PathBuf::from("/x/y");
    let rp = ResolvedPath::from_backend_resolved(p.clone());
    assert_eq!(rp.into_path_buf(), p);
}

#[test]
fn display_renders_inner_path() {
    let rp = ResolvedPath::from_backend_resolved(std::path::PathBuf::from("/abc"));
    assert_eq!(format!("{rp}"), "/abc");
}

#[test]
fn equality_and_hash_use_inner_path() {
    use std::collections::HashSet;
    let rp1 = ResolvedPath::from_backend_resolved(std::path::PathBuf::from("/p"));
    let rp2 = ResolvedPath::from_backend_resolved(std::path::PathBuf::from("/p"));
    let rp3 = ResolvedPath::from_backend_resolved(std::path::PathBuf::from("/q"));
    assert_eq!(rp1, rp2);
    assert_ne!(rp1, rp3);

    let mut set = HashSet::new();
    set.insert(rp1.clone());
    assert!(set.contains(&rp2));
    assert!(!set.contains(&rp3));
}

#[test]
fn as_ref_path_works() {
    let rp = ResolvedPath::from_backend_resolved(std::path::PathBuf::from("/r"));
    let p: &std::path::Path = rp.as_ref();
    assert_eq!(p, std::path::Path::new("/r"));
}

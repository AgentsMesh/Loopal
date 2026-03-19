use loopal_tools::edit::search_replace::{search_replace, SearchReplaceResult};

#[test]
fn test_single_replace() {
    match search_replace("hello world", "world", "rust", false) {
        SearchReplaceResult::Ok(s) => assert_eq!(s, "hello rust"),
        _ => panic!("expected Ok"),
    }
}

#[test]
fn test_not_found() {
    assert!(matches!(
        search_replace("hello", "xyz", "abc", false),
        SearchReplaceResult::NotFound
    ));
}

#[test]
fn test_multiple_no_replace_all() {
    assert!(matches!(
        search_replace("aa", "a", "b", false),
        SearchReplaceResult::MultipleMatches(2)
    ));
}

#[test]
fn test_replace_all() {
    match search_replace("aaa", "a", "b", true) {
        SearchReplaceResult::Ok(s) => assert_eq!(s, "bbb"),
        _ => panic!("expected Ok"),
    }
}

#[test]
fn test_replace_multiline() {
    let content = "line1\nline2\nline3";
    match search_replace(content, "line2", "replaced", false) {
        SearchReplaceResult::Ok(s) => assert_eq!(s, "line1\nreplaced\nline3"),
        _ => panic!("expected Ok"),
    }
}

#[test]
fn test_replace_with_empty_string() {
    match search_replace("hello world", "world", "", false) {
        SearchReplaceResult::Ok(s) => assert_eq!(s, "hello "),
        _ => panic!("expected Ok"),
    }
}

#[test]
fn test_replace_all_not_found() {
    assert!(matches!(
        search_replace("hello", "xyz", "abc", true),
        SearchReplaceResult::NotFound
    ));
}

#[test]
fn test_single_replace_exact_match() {
    // L27: !replace_all && count > 1 is false because count == 1
    // L31: replace_all is false, so uses replacen
    match search_replace("abcd", "bc", "XX", false) {
        SearchReplaceResult::Ok(s) => assert_eq!(s, "aXXd"),
        _ => panic!("expected Ok"),
    }
}

#[test]
fn test_replace_all_multiple_occurrences() {
    // L31: replace_all is true, uses replace
    match search_replace("ababab", "ab", "x", true) {
        SearchReplaceResult::Ok(s) => assert_eq!(s, "xxx"),
        _ => panic!("expected Ok"),
    }
}

#[test]
fn test_replace_all_single_occurrence() {
    // replace_all with only one occurrence
    match search_replace("hello world", "world", "earth", true) {
        SearchReplaceResult::Ok(s) => assert_eq!(s, "hello earth"),
        _ => panic!("expected Ok"),
    }
}

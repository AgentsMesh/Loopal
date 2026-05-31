use loopal_memory::extract::wikilink::{normalize_to_slug, scan};

#[test]
fn scan_finds_single_wikilink() {
    let body = "see [[foo-bar]] for details";
    let links = scan(body);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].slug, "foo-bar");
    assert_eq!(links[0].line, 1);
}

#[test]
fn scan_records_line_numbers() {
    let body = "line one\nline two [[alpha]]\n\nline four [[beta]]";
    let links = scan(body);
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].slug, "alpha");
    assert_eq!(links[0].line, 2);
    assert_eq!(links[1].slug, "beta");
    assert_eq!(links[1].line, 4);
}

#[test]
fn scan_ignores_uppercase_or_non_slug_chars() {
    let body = "[[Foo]] [[bar baz]] [[123]] [[ok-id]] [[a_b]]";
    let links: Vec<_> = scan(body).into_iter().map(|l| l.slug).collect();
    assert_eq!(links, vec!["ok-id", "a_b"]);
}

#[test]
fn scan_accepts_alias_pipe_syntax() {
    let body = "see [[my-slug|My Display Title]]";
    let links = scan(body);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].slug, "my-slug");
}

#[test]
fn scan_finds_multiple_on_same_line() {
    let body = "[[a]] [[b]] [[c]]";
    let links: Vec<_> = scan(body).into_iter().map(|l| l.slug).collect();
    assert_eq!(links, vec!["a", "b", "c"]);
}

#[test]
fn normalize_to_slug_strips_md_suffix() {
    assert_eq!(normalize_to_slug("foo.md").as_deref(), Some("foo"));
    assert_eq!(normalize_to_slug("a-b-c.md").as_deref(), Some("a-b-c"));
}

#[test]
fn normalize_to_slug_strips_brackets() {
    assert_eq!(normalize_to_slug("[[foo]]").as_deref(), Some("foo"));
    assert_eq!(normalize_to_slug("[[foo|Title]]").as_deref(), Some("foo"));
}

#[test]
fn normalize_to_slug_rejects_invalid_chars() {
    assert!(normalize_to_slug("Foo").is_none());
    assert!(normalize_to_slug("123-x").is_none());
    assert!(normalize_to_slug("a b").is_none());
    assert!(normalize_to_slug("").is_none());
    assert!(normalize_to_slug("[[]]").is_none());
}

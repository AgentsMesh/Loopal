use loopal_memory::extract::frontmatter::parse;

#[test]
fn parses_full_frontmatter() {
    let text = "---\nname: Twitter Auto\ndescription: rate limits\ntype: project\nttl_days: 90\nrelated:\n  - twitter-long-tweets\n  - chrome-for-testing-cdp\n---\n\nBody starts here.\n";
    let p = parse(text);
    assert_eq!(p.frontmatter.name.as_deref(), Some("Twitter Auto"));
    assert_eq!(p.frontmatter.description.as_deref(), Some("rate limits"));
    assert_eq!(p.frontmatter.kind.as_deref(), Some("project"));
    assert_eq!(p.frontmatter.ttl_days, Some(90));
    assert_eq!(p.frontmatter.related.len(), 2);
    assert_eq!(p.body.trim(), "Body starts here.");
    assert!(p.errors.is_empty());
}

#[test]
fn missing_frontmatter_returns_default_with_no_errors() {
    let text = "Just a body, no frontmatter.\nLine two.\n";
    let p = parse(text);
    assert!(p.frontmatter.name.is_none());
    assert!(p.frontmatter.kind.is_none());
    assert_eq!(p.body, text);
    assert!(p.errors.is_empty());
}

#[test]
fn malformed_yaml_recovers_to_default() {
    let text = "---\nname: ok\ntype: [unclosed\n---\nbody\n";
    let p = parse(text);
    assert!(!p.errors.is_empty());
    assert!(p.frontmatter.name.is_none());
}

#[test]
fn partial_frontmatter_fills_missing_with_none() {
    let text = "---\nname: only-name\n---\nbody\n";
    let p = parse(text);
    assert_eq!(p.frontmatter.name.as_deref(), Some("only-name"));
    assert!(p.frontmatter.kind.is_none());
    assert!(p.frontmatter.ttl_days.is_none());
    assert!(p.frontmatter.related.is_empty());
}

#[test]
fn detects_git_merge_conflict_marker() {
    let text = "---\nname: x\n---\n<<<<<<< HEAD\nmine\n=======\ntheirs\n>>>>>>> branch\n";
    let p = parse(text);
    let has_marker = p.errors.iter().any(|e| {
        matches!(
            e,
            loopal_memory::extract::errors::ExtractionError::MergeConflictMarker
        )
    });
    assert!(has_marker);
}

#[test]
fn handles_bom_at_start() {
    let text = "\u{feff}---\nname: bom\n---\nbody\n";
    let p = parse(text);
    assert_eq!(p.frontmatter.name.as_deref(), Some("bom"));
}

#[test]
fn related_accepts_both_md_suffix_and_wikilink_form() {
    let text = "---\nrelated:\n  - foo.md\n  - \"[[bar]]\"\n  - baz\n---\n";
    let p = parse(text);
    assert_eq!(p.frontmatter.related.len(), 3);
}

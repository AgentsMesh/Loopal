use loopal_memory::extract::{extract_file, slug_from_path};
use loopal_memory::{EdgeKind, MemoryKind, Provenance};

#[test]
fn extracts_full_project_memory() {
    let md = "---\nname: Twitter Auto\ndescription: rate limits\ntype: project\nttl_days: 90\nrelated:\n  - twitter-long-tweets\n---\n\nWe enforce strict cooldowns. See [[chrome-for-testing-cdp]] for browser tricks.\n";
    let result = extract_file("twitter-automation.md", md);
    assert_eq!(result.nodes.len(), 1);

    let n = &result.nodes[0];
    assert_eq!(n.id, "twitter-automation");
    assert_eq!(n.kind, MemoryKind::Project);
    assert_eq!(n.name, "Twitter Auto");
    assert_eq!(n.description.as_deref(), Some("rate limits"));
    assert_eq!(n.ttl_days, Some(90));
    assert!(n.body.contains("strict cooldowns"));
    assert_eq!(n.content_hash.len(), 64);
}

#[test]
fn generates_one_frontmatter_edge_per_related() {
    let md = "---\ntype: project\nrelated:\n  - alpha\n  - beta\n---\n";
    let r = extract_file("foo.md", md);
    let frontmatter_edges: Vec<_> = r
        .edges
        .iter()
        .filter(|e| e.provenance == Provenance::Frontmatter)
        .collect();
    assert_eq!(frontmatter_edges.len(), 2);
    assert!(
        frontmatter_edges
            .iter()
            .all(|e| e.kind == EdgeKind::References)
    );
}

#[test]
fn generates_inline_edges_for_each_wikilink_with_line() {
    let md = "---\ntype: project\n---\nfirst body line\n[[alpha]]\nplain\n[[beta]]\n";
    let r = extract_file("foo.md", md);
    let inline: Vec<_> = r
        .edges
        .iter()
        .filter(|e| e.provenance == Provenance::InlineLink)
        .collect();
    assert_eq!(inline.len(), 2);
    let targets: Vec<&str> = inline.iter().map(|e| e.dst_id.as_str()).collect();
    assert!(targets.contains(&"alpha"));
    assert!(targets.contains(&"beta"));
    let alpha = inline.iter().find(|e| e.dst_id == "alpha").unwrap();
    assert_eq!(alpha.line, Some(2));
    let beta = inline.iter().find(|e| e.dst_id == "beta").unwrap();
    assert_eq!(beta.line, Some(4));
}

#[test]
fn missing_type_defaults_to_reference() {
    let r = extract_file("foo.md", "no frontmatter, just body");
    assert_eq!(r.nodes[0].kind, MemoryKind::Reference);
}

#[test]
fn memory_index_file_classified_as_index() {
    let r = extract_file("MEMORY.md", "# Index");
    assert_eq!(r.nodes[0].kind, MemoryKind::Index);
}

#[test]
fn body_stores_full_content_for_indexing() {
    let mut src = String::from("---\ntype: reference\n---\n\n");
    src.push_str(&"a".repeat(500));
    let r = extract_file("foo.md", &src);
    // The whole body is stored (indexed by FTS), not truncated to a preview.
    assert!(r.nodes[0].body.len() >= 500);
}

#[test]
fn unresolved_links_record_inline_targets() {
    let md = "---\ntype: project\n---\n[[never-defined]]\n";
    let r = extract_file("foo.md", md);
    assert_eq!(r.unresolved.len(), 1);
    assert_eq!(r.unresolved[0].target_name, "never-defined");
    assert_eq!(r.unresolved[0].line, 1);
}

#[test]
fn merge_conflict_file_yields_error_but_still_extracts() {
    let md = "---\ntype: project\n---\n<<<<<<< HEAD\na\n=======\nb\n>>>>>>> branch\n";
    let r = extract_file("foo.md", md);
    assert!(!r.errors.is_empty());
    assert_eq!(r.nodes.len(), 1);
}

#[test]
fn slug_from_path_handles_various_paths() {
    assert_eq!(slug_from_path("foo.md"), "foo");
    assert_eq!(slug_from_path("teams/onboard.md"), "teams__onboard");
    assert_eq!(slug_from_path("./baz.md"), "baz");
}

#[test]
fn slug_from_path_distinguishes_same_filename_different_dirs() {
    let a = slug_from_path("notes/foo.md");
    let b = slug_from_path("archive/foo.md");
    assert_ne!(
        a, b,
        "same file_stem in different dirs must yield different slugs"
    );
    assert_eq!(a, "notes__foo");
    assert_eq!(b, "archive__foo");
}

#[test]
fn slug_from_path_avoids_dir_vs_hyphen_collision() {
    let nested = slug_from_path("a/b.md");
    let flat = slug_from_path("a-b.md");
    assert_ne!(
        nested, flat,
        "nested 'a/b.md' must not collide with flat 'a-b.md'"
    );
    assert_eq!(nested, "a__b");
    assert_eq!(flat, "a-b");
}

#[test]
fn content_hash_is_deterministic_and_unique_for_changes() {
    let r1 = extract_file("a.md", "hello");
    let r2 = extract_file("a.md", "hello");
    let r3 = extract_file("a.md", "world");
    assert_eq!(r1.nodes[0].content_hash, r2.nodes[0].content_hash);
    assert_ne!(r1.nodes[0].content_hash, r3.nodes[0].content_hash);
}

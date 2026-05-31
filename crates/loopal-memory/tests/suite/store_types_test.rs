use loopal_memory::{EdgeKind, MemoryKind, Provenance};

#[test]
fn memory_kind_round_trip() {
    for k in [
        MemoryKind::User,
        MemoryKind::Feedback,
        MemoryKind::Project,
        MemoryKind::Reference,
        MemoryKind::Index,
    ] {
        let s = k.as_str();
        assert_eq!(MemoryKind::parse(s).unwrap(), k);
    }
}

#[test]
fn memory_kind_parse_rejects_unknown() {
    assert!(MemoryKind::parse("nope").is_err());
    assert!(MemoryKind::parse("").is_err());
    assert!(MemoryKind::parse("USER").is_err());
}

#[test]
fn memory_kind_serde_uses_snake_case() {
    let s = serde_json::to_string(&MemoryKind::Feedback).unwrap();
    assert_eq!(s, "\"feedback\"");
    let k: MemoryKind = serde_json::from_str("\"reference\"").unwrap();
    assert_eq!(k, MemoryKind::Reference);
}

#[test]
fn edge_kind_round_trip() {
    for k in [
        EdgeKind::References,
        EdgeKind::ContainedIn,
        EdgeKind::SupersededBy,
        EdgeKind::DerivedFrom,
        EdgeKind::CoOccursSlug,
        EdgeKind::Contradicts,
    ] {
        let s = k.as_str();
        assert_eq!(EdgeKind::parse(s).unwrap(), k);
    }
}

#[test]
fn edge_kind_parse_rejects_unknown() {
    assert!(EdgeKind::parse("invalid").is_err());
    assert!(EdgeKind::parse("references ").is_err());
}

#[test]
fn edge_kind_serde_uses_snake_case() {
    let s = serde_json::to_string(&EdgeKind::ContainedIn).unwrap();
    assert_eq!(s, "\"contained_in\"");
}

#[test]
fn provenance_round_trip() {
    for p in [
        Provenance::Frontmatter,
        Provenance::InlineLink,
        Provenance::Index,
        Provenance::Synthesized,
        Provenance::UserStated,
    ] {
        let s = p.as_str();
        assert_eq!(Provenance::parse(s).unwrap(), p);
    }
}

#[test]
fn provenance_uses_kebab_case() {
    assert_eq!(Provenance::InlineLink.as_str(), "inline-link");
    assert_eq!(Provenance::UserStated.as_str(), "user-stated");
    let s = serde_json::to_string(&Provenance::InlineLink).unwrap();
    assert_eq!(s, "\"inline-link\"");
}

#[test]
fn provenance_parse_rejects_snake_case_form() {
    assert!(Provenance::parse("inline_link").is_err());
    assert!(Provenance::parse("user_stated").is_err());
}

use loopal_config::skills::parse_skill;

#[test]
fn test_parse_with_frontmatter() {
    let content = "---\ndescription: Generate a git commit\n---\nReview staged changes.\n$ARGUMENTS\n";
    let skill = parse_skill("/commit", content);
    assert_eq!(skill.name, "/commit");
    assert_eq!(skill.description, "Generate a git commit");
    assert!(skill.has_arg);
    assert!(skill.body.contains("Review staged changes."));
    assert!(skill.body.contains("$ARGUMENTS"));
}

#[test]
fn test_parse_without_frontmatter() {
    let content = "Review the code and suggest improvements.\nBe thorough.\n";
    let skill = parse_skill("/review", content);
    assert_eq!(skill.name, "/review");
    assert_eq!(skill.description, "Review the code and suggest improvements.");
    assert!(!skill.has_arg);
    assert!(skill.body.contains("Be thorough."));
}

#[test]
fn test_parse_empty_frontmatter() {
    let content = "---\n---\nJust a body.\n";
    let skill = parse_skill("/test", content);
    assert_eq!(skill.name, "/test");
    // Falls back to first line of body
    assert_eq!(skill.description, "Just a body.");
    assert!(!skill.has_arg);
}

#[test]
fn test_parse_description_truncation() {
    let long_line = "A".repeat(80);
    let content = format!("{long_line}\nSecond line.");
    let skill = parse_skill("/long", &content);
    assert!(skill.description.chars().count() <= 60);
    assert!(skill.description.ends_with('…'));
}

#[test]
fn test_parse_arguments_detection() {
    let with_arg = "Do something with $ARGUMENTS here.";
    let without_arg = "Do something here.";

    assert!(parse_skill("/a", with_arg).has_arg);
    assert!(!parse_skill("/b", without_arg).has_arg);
}

#[test]
fn test_parse_frontmatter_ignores_unknown_keys() {
    let content = "---\nauthor: alice\ndescription: My skill\nversion: 1\n---\nBody here.\n";
    let skill = parse_skill("/custom", content);
    assert_eq!(skill.description, "My skill");
    assert_eq!(skill.body, "Body here.\n");
}

#[test]
fn test_parse_no_closing_frontmatter() {
    let content = "---\ndescription: Unclosed\nBody content here.\n";
    let skill = parse_skill("/broken", content);
    // No closing --- means entire content is treated as body, description from first line
    assert!(skill.description.starts_with("---"));
    assert_eq!(skill.body, content);
}

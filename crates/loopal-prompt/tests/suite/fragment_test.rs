use loopal_prompt::{parse_fragment, Category, Condition};

#[test]
fn parse_basic_fragment() {
    let raw = "\
---
name: Test Fragment
category: core
condition: always
priority: 100
---
# Hello
This is a test.
";
    let frag = parse_fragment("core/test", raw).unwrap();
    assert_eq!(frag.id, "core/test");
    assert_eq!(frag.name, "Test Fragment");
    assert_eq!(frag.category, Category::Core);
    assert_eq!(frag.condition, Condition::Always);
    assert_eq!(frag.priority, 100);
    assert!(frag.content.contains("# Hello"));
}

#[test]
fn parse_mode_condition() {
    let raw = "\
---
name: Plan Mode
condition: mode
condition_value: plan
priority: 900
---
Plan instructions here.
";
    let frag = parse_fragment("modes/plan", raw).unwrap();
    assert_eq!(frag.condition, Condition::Mode("plan".to_string()));
}

#[test]
fn parse_defaults() {
    let raw = "\
---
name: Minimal
---
Content only.
";
    let frag = parse_fragment("custom/minimal", raw).unwrap();
    assert_eq!(frag.priority, 500); // default
    assert_eq!(frag.condition, Condition::Always);
    assert_eq!(frag.category, Category::Custom);
}

#[test]
fn infer_category_from_id() {
    let raw = "\
---
name: Tool Policy
---
Use the right tool.
";
    let frag = parse_fragment("tools/usage-policy", raw).unwrap();
    assert_eq!(frag.category, Category::Tools);
}

#[test]
fn returns_none_without_frontmatter() {
    let raw = "Just plain text, no frontmatter.";
    assert!(parse_fragment("bad", raw).is_none());
}

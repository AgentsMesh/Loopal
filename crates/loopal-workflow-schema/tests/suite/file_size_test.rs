const SOURCES: &[(&str, &str)] = &[
    ("lib.rs", include_str!("../../src/lib.rs")),
    ("policy.rs", include_str!("../../src/policy.rs")),
    ("schema.rs", include_str!("../../src/schema.rs")),
];

#[test]
fn handwritten_sources_stay_under_line_cap() {
    for (name, source) in SOURCES {
        let lines = source.lines().count();
        assert!(lines <= 200, "{name} is {lines} lines");
    }
}

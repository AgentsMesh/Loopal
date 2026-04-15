/// Command module tests: registry, filter_entries.
use loopal_config::Skill;
use loopal_tui::command::{CommandEntry, CommandRegistry, filter_entries};

// ---------------------------------------------------------------------------
// CommandRegistry
// ---------------------------------------------------------------------------

#[test]
fn test_registry_new_has_all_builtins() {
    let registry = CommandRegistry::new();
    let entries = registry.entries();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    for expected in &[
        "/plan", "/act", "/clear", "/compact", "/model", "/rewind", "/status", "/resume", "/init",
        "/help", "/exit", "/agents", "/topology", "/skills",
    ] {
        assert!(names.contains(expected), "missing builtin: {expected}");
    }
}

#[test]
fn test_registry_find_returns_handler() {
    let registry = CommandRegistry::new();
    assert!(registry.find("/clear").is_some());
    assert!(registry.find("/model").is_some());
    assert!(registry.find("/resume").is_some());
}

#[test]
fn test_registry_sessions_command_removed() {
    let registry = CommandRegistry::new();
    assert!(
        registry.find("/sessions").is_none(),
        "/sessions was replaced by /resume"
    );
}

#[test]
fn test_registry_find_unknown_returns_none() {
    let registry = CommandRegistry::new();
    assert!(registry.find("/nonexistent").is_none());
    assert!(registry.find("clear").is_none());
}

#[test]
fn test_registry_reload_skills_adds_skills() {
    let mut registry = CommandRegistry::new();
    let skills = vec![Skill {
        name: "/commit".into(),
        description: "Generate commit".into(),
        has_arg: true,
        body: "Review changes. $ARGUMENTS".into(),
    }];
    registry.reload_skills(&skills);
    let handler = registry.find("/commit");
    assert!(handler.is_some());
    assert!(handler.unwrap().is_skill());
}

#[test]
fn test_registry_reload_skills_builtin_priority() {
    let mut registry = CommandRegistry::new();
    let skills = vec![Skill {
        name: "/help".into(),
        description: "Custom help".into(),
        has_arg: false,
        body: "custom body".into(),
    }];
    registry.reload_skills(&skills);
    let handler = registry.find("/help").unwrap();
    assert!(!handler.is_skill());
}

#[test]
fn test_registry_reload_replaces_old_skills() {
    let mut registry = CommandRegistry::new();
    registry.reload_skills(&[Skill {
        name: "/commit".into(),
        description: "v1".into(),
        has_arg: false,
        body: "v1".into(),
    }]);
    assert!(registry.find("/commit").is_some());
    registry.reload_skills(&[]);
    assert!(registry.find("/commit").is_none());
}

#[test]
fn test_registry_entries_includes_skills() {
    let mut registry = CommandRegistry::new();
    registry.reload_skills(&[Skill {
        name: "/deploy".into(),
        description: "Deploy".into(),
        has_arg: false,
        body: "deploy now".into(),
    }]);
    let entries = registry.entries();
    let deploy = entries.iter().find(|e| e.name == "/deploy");
    assert!(deploy.is_some());
    assert!(deploy.unwrap().is_skill);
}

#[test]
fn test_registry_entries_no_duplicate_builtins() {
    let registry = CommandRegistry::new();
    let entries = registry.entries();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    let unique: std::collections::HashSet<&str> = names.iter().copied().collect();
    assert_eq!(names.len(), unique.len());
}

#[test]
fn test_registry_builtin_not_marked_as_skill() {
    let registry = CommandRegistry::new();
    for entry in registry.entries() {
        assert!(!entry.is_skill, "{} should not be a skill", entry.name);
    }
}

// ---------------------------------------------------------------------------
// filter_entries (returns Vec<CommandEntry> snapshots)
// ---------------------------------------------------------------------------

fn sample_entries() -> Vec<CommandEntry> {
    CommandRegistry::new().entries()
}

#[test]
fn test_filter_slash_matches_all() {
    let entries = sample_entries();
    let result = filter_entries(&entries, "/");
    assert_eq!(result.len(), entries.len());
}

#[test]
fn test_filter_prefix_c_matches_clear_compact() {
    let result = filter_entries(&sample_entries(), "/c");
    let names: Vec<&str> = result.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"/clear"));
    assert!(names.contains(&"/compact"));
    assert_eq!(names.len(), 2);
}

#[test]
fn test_filter_unknown_prefix_matches_none() {
    let result = filter_entries(&sample_entries(), "/zzz");
    assert!(result.is_empty());
}

#[test]
fn test_filter_case_insensitive() {
    let result = filter_entries(&sample_entries(), "/HELP");
    let names: Vec<&str> = result.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"/help"));
}

#[test]
fn test_filter_with_skills() {
    let mut registry = CommandRegistry::new();
    registry.reload_skills(&[Skill {
        name: "/commit".into(),
        description: "Commit".into(),
        has_arg: true,
        body: "...".into(),
    }]);
    let result = filter_entries(&registry.entries(), "/co");
    let names: Vec<&str> = result.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"/compact"));
    assert!(names.contains(&"/commit"));
}

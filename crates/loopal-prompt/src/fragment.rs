use serde::Deserialize;

/// A prompt fragment parsed from a .md file with YAML frontmatter.
#[derive(Debug, Clone)]
pub struct Fragment {
    /// Derived from file path, e.g. "core/output-efficiency".
    pub id: String,
    /// Human-readable name from frontmatter.
    pub name: String,
    /// Organizational category.
    pub category: Category,
    /// When this fragment should be included.
    pub condition: Condition,
    /// Assembly order (lower = earlier). Default 500.
    pub priority: u16,
    /// Minijinja template body (content after frontmatter).
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Category {
    Core,
    Tasks,
    Tools,
    Modes,
    Agents,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// Always included.
    Always,
    /// Only when the agent is in the specified mode ("plan", "auto").
    Mode(String),
    /// Only when a feature flag is set ("team_mode", "memory_enabled").
    Feature(String),
    /// Only when the specified tool is available.
    Tool(String),
}

// -- Frontmatter deserialization --

#[derive(Deserialize)]
struct Frontmatter {
    name: Option<String>,
    category: Option<String>,
    condition: Option<String>,
    #[serde(default)]
    condition_value: Option<String>,
    #[serde(default = "default_priority")]
    priority: u16,
}

fn default_priority() -> u16 {
    500
}

/// Parse a single fragment from raw markdown with YAML frontmatter.
///
/// `id` is typically derived from the file path (e.g. "core/output-efficiency").
pub fn parse_fragment(id: &str, raw: &str) -> Option<Fragment> {
    let (fm, content) = split_frontmatter(raw)?;
    let meta: Frontmatter = serde_json::from_value(serde_yaml_value(&fm)?).ok()?;

    let category = match meta.category.as_deref() {
        Some("core") => Category::Core,
        Some("tasks") => Category::Tasks,
        Some("tools") => Category::Tools,
        Some("modes") => Category::Modes,
        Some("agents") => Category::Agents,
        _ => infer_category(id),
    };

    let condition = parse_condition(meta.condition.as_deref(), meta.condition_value.as_deref());

    Some(Fragment {
        id: id.to_string(),
        name: meta.name.unwrap_or_else(|| id.to_string()),
        category,
        condition,
        priority: meta.priority,
        content: content.to_string(),
    })
}

/// Parse all fragments from an `include_dir::Dir`.
pub fn parse_fragments_from_dir(dir: &include_dir::Dir<'_>) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    collect_dir(dir, &mut fragments);
    fragments
}

fn collect_dir(dir: &include_dir::Dir<'_>, out: &mut Vec<Fragment>) {
    for file in dir.files() {
        let path = file.path().to_string_lossy();
        if !path.ends_with(".md") {
            continue;
        }
        let id = path.trim_end_matches(".md").to_string();
        if let Some(raw) = file.contents_utf8() {
            if let Some(frag) = parse_fragment(&id, raw) {
                out.push(frag);
            } else {
                tracing::warn!(id = %id, "failed to parse prompt fragment");
            }
        }
    }
    for sub in dir.dirs() {
        collect_dir(sub, out);
    }
}

fn infer_category(id: &str) -> Category {
    match id.split('/').next() {
        Some("core") => Category::Core,
        Some("tasks") => Category::Tasks,
        Some("tools") => Category::Tools,
        Some("modes") => Category::Modes,
        Some("agents") => Category::Agents,
        _ => Category::Custom,
    }
}

fn parse_condition(kind: Option<&str>, value: Option<&str>) -> Condition {
    match kind {
        Some("always") | None => Condition::Always,
        Some("mode") => Condition::Mode(value.unwrap_or("plan").to_string()),
        Some("feature") => Condition::Feature(value.unwrap_or("").to_string()),
        Some("tool") => Condition::Tool(value.unwrap_or("").to_string()),
        Some(other) => {
            tracing::warn!(
                condition = other,
                "unknown condition kind, defaulting to Always"
            );
            Condition::Always
        }
    }
}

// -- Minimal YAML frontmatter parsing --
// We avoid pulling in a full YAML crate by parsing the simple key: value format
// into a serde_json::Value.

fn split_frontmatter(raw: &str) -> Option<(String, &str)> {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let after_start = &trimmed[3..];
    let end = after_start.find("\n---")?;
    let fm = &after_start[..end];
    let content_start = 3 + end + 4; // skip "---\n---\n"
    let content = trimmed[content_start..].trim_start_matches('\n');
    Some((fm.to_string(), content))
}

fn serde_yaml_value(yaml: &str) -> Option<serde_json::Value> {
    let mut map = serde_json::Map::new();
    for line in yaml.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, val) = line.split_once(':')?;
        let key = key.trim();
        let val = val.trim();
        // Try to parse as number, bool, or keep as string
        if let Ok(n) = val.parse::<u64>() {
            map.insert(key.to_string(), serde_json::Value::from(n));
        } else if val == "true" {
            map.insert(key.to_string(), serde_json::Value::from(true));
        } else if val == "false" {
            map.insert(key.to_string(), serde_json::Value::from(false));
        } else {
            map.insert(key.to_string(), serde_json::Value::from(val));
        }
    }
    Some(serde_json::Value::Object(map))
}

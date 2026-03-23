use include_dir::{Dir, include_dir};
use loopal_prompt::Fragment;

static PROMPTS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/prompts");

/// Return all built-in system prompt fragments.
pub fn system_fragments() -> Vec<Fragment> {
    loopal_prompt::parse_fragments_from_dir(&PROMPTS_DIR)
}

/// Return the prompt fragment for a specific sub-agent type (e.g. "explore").
pub fn agent_fragment(agent_type: &str) -> Option<Fragment> {
    let id = format!("agents/{agent_type}");
    system_fragments().into_iter().find(|f| f.id == id)
}

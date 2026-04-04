use std::path::Path;

use crate::context::PromptContext;
use crate::fragment::{Category, Condition, Fragment, parse_fragment, parse_fragments_from_dir};
use crate::render::PromptRenderer;

/// Manages a collection of prompt fragments with selection, rendering,
/// and user-override support.
pub struct FragmentRegistry {
    fragments: Vec<Fragment>,
    renderer: PromptRenderer,
}

impl FragmentRegistry {
    /// Create a registry from a pre-loaded list of fragments.
    pub fn new(fragments: Vec<Fragment>) -> Self {
        Self {
            fragments,
            renderer: PromptRenderer::new(),
        }
    }

    /// Load fragments from a compiled-in `include_dir::Dir`.
    pub fn from_included_dir(dir: &include_dir::Dir<'_>) -> Self {
        Self::new(parse_fragments_from_dir(dir))
    }

    /// Merge user-override fragments from a filesystem directory.
    ///
    /// Fragments with the same `id` as existing ones replace them.
    /// New ids are appended.
    pub fn add_overrides_from_path(&mut self, dir: &Path) {
        let overrides = load_fragments_from_fs(dir);
        for ov in overrides {
            if let Some(existing) = self.fragments.iter_mut().find(|f| f.id == ov.id) {
                *existing = ov;
            } else {
                self.fragments.push(ov);
            }
        }
    }

    /// Select fragments that match the given context, sorted by priority.
    ///
    /// Agents-category fragments are excluded for the root agent (non-subagent).
    /// When a specific Agent condition matches (e.g. "explore"), the default
    /// fallback agent fragment (Always + Agents) is excluded to avoid overlap.
    pub fn select<'a>(&'a self, ctx: &PromptContext) -> Vec<&'a Fragment> {
        let is_subagent = ctx.is_subagent();

        // Check if any Agent(type) condition matches for this context.
        let has_specific_agent = is_subagent
            && self.fragments.iter().any(|f| {
                f.category == Category::Agents
                    && matches!(&f.condition, Condition::Agent(t) if ctx.agent_type.as_deref() == Some(t.as_str()))
            });

        let mut matched: Vec<&Fragment> = self
            .fragments
            .iter()
            .filter(|f| {
                // Agents category only relevant for sub-agents
                if f.category == Category::Agents && !is_subagent {
                    return false;
                }
                // If a specific agent fragment matches, skip the Always fallback
                if has_specific_agent
                    && f.category == Category::Agents
                    && f.condition == Condition::Always
                {
                    return false;
                }
                condition_matches(&f.condition, ctx)
            })
            .collect();
        matched.sort_by_key(|f| f.priority);
        matched
    }

    /// Render a single fragment with the given context.
    pub fn render(&self, fragment: &Fragment, ctx: &PromptContext) -> String {
        self.renderer.render(&fragment.content, ctx)
    }

    /// Access all loaded fragments (for testing/introspection).
    pub fn fragments(&self) -> &[Fragment] {
        &self.fragments
    }
}

fn condition_matches(cond: &Condition, ctx: &PromptContext) -> bool {
    match cond {
        Condition::Always => true,
        Condition::Mode(m) => ctx.mode == *m,
        Condition::Feature(f) => ctx.features.contains(f),
        Condition::Tool(t) => ctx.tool_names.contains(t),
        Condition::Agent(t) => ctx.agent_type.as_deref() == Some(t.as_str()),
    }
}

fn load_fragments_from_fs(dir: &Path) -> Vec<Fragment> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return out;
    }
    collect_fs_dir(dir, dir, &mut out);
    out
}

fn collect_fs_dir(base: &Path, current: &Path, out: &mut Vec<Fragment>) {
    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_fs_dir(base, &path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            let rel = path.strip_prefix(base).unwrap_or(&path);
            let id = rel.with_extension("").to_string_lossy().replace('\\', "/");
            if let Ok(raw) = std::fs::read_to_string(&path)
                && let Some(frag) = parse_fragment(&id, &raw)
            {
                out.push(frag);
            }
        }
    }
}

//! Map Loopal tool names to ACP `ToolKind`.

use agent_client_protocol_schema::ToolKind;

/// Map a Loopal tool name to an ACP `ToolKind`.
pub fn map_tool_kind(name: &str) -> ToolKind {
    match name {
        "Read" | "Glob" | "Grep" | "Ls" => ToolKind::Read,
        "Write" | "Edit" | "MultiEdit" | "ApplyPatch" | "Diff" => ToolKind::Edit,
        "Bash" | "Background" => ToolKind::Execute,
        "WebFetch" | "WebSearch" => ToolKind::Fetch,
        "FileOps" => ToolKind::Move,
        "PlanMode" => ToolKind::SwitchMode,
        _ => ToolKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_tools() {
        assert!(matches!(map_tool_kind("Read"), ToolKind::Read));
        assert!(matches!(map_tool_kind("Glob"), ToolKind::Read));
        assert!(matches!(map_tool_kind("Grep"), ToolKind::Read));
        assert!(matches!(map_tool_kind("Ls"), ToolKind::Read));
    }

    #[test]
    fn edit_tools() {
        assert!(matches!(map_tool_kind("Write"), ToolKind::Edit));
        assert!(matches!(map_tool_kind("Edit"), ToolKind::Edit));
        assert!(matches!(map_tool_kind("MultiEdit"), ToolKind::Edit));
        assert!(matches!(map_tool_kind("ApplyPatch"), ToolKind::Edit));
        assert!(matches!(map_tool_kind("Diff"), ToolKind::Edit));
    }

    #[test]
    fn execute_and_fetch() {
        assert!(matches!(map_tool_kind("Bash"), ToolKind::Execute));
        assert!(matches!(map_tool_kind("Background"), ToolKind::Execute));
        assert!(matches!(map_tool_kind("WebFetch"), ToolKind::Fetch));
        assert!(matches!(map_tool_kind("WebSearch"), ToolKind::Fetch));
    }

    #[test]
    fn new_kinds() {
        assert!(matches!(map_tool_kind("FileOps"), ToolKind::Move));
        assert!(matches!(map_tool_kind("PlanMode"), ToolKind::SwitchMode));
    }

    #[test]
    fn unknown_is_other() {
        assert!(matches!(map_tool_kind("CustomTool"), ToolKind::Other));
    }
}

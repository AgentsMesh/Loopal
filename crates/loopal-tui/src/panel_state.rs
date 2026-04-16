//! Unified panel section state — focus and scroll for any panel kind.

use crate::app::PanelKind;

/// Per-section focus + scroll state, shared by all panel kinds.
pub struct PanelSectionState {
    pub kind: PanelKind,
    pub focused: Option<String>,
    pub scroll_offset: usize,
}

impl PanelSectionState {
    pub fn new(kind: PanelKind) -> Self {
        Self {
            kind,
            focused: None,
            scroll_offset: 0,
        }
    }
}

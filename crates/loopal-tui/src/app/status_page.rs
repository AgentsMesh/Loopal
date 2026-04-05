//! Data types for the `/status` full-screen sub-page.

/// Active tab in the status sub-page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTab {
    Status,
    Config,
    Usage,
}

impl StatusTab {
    pub const ALL: [Self; 3] = [Self::Status, Self::Config, Self::Usage];

    pub fn label(self) -> &'static str {
        match self {
            Self::Status => "Status",
            Self::Config => "Config",
            Self::Usage => "Usage",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Status => 0,
            Self::Config => 1,
            Self::Usage => 2,
        }
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// A key-value pair for the Config tab display.
#[derive(Debug, Clone)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
}

/// Snapshot of runtime session state (from SessionController lock).
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub cwd: String,
    pub model_display: String,
    pub mode: String,
    /// Hub TCP endpoint, e.g. "127.0.0.1:12345". Empty if not listening.
    pub hub_endpoint: String,
}

/// Snapshot of resolved configuration (from disk-loaded ResolvedConfig).
#[derive(Debug, Clone)]
pub struct ConfigSnapshot {
    pub auth_env: String,
    pub base_url: String,
    pub mcp_configured: usize,
    pub mcp_enabled: usize,
    pub setting_sources: Vec<String>,
    pub entries: Vec<ConfigEntry>,
}

/// Snapshot of token/usage metrics for the Usage tab.
#[derive(Debug, Clone)]
pub struct UsageSnapshot {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub context_window: u32,
    pub context_used: u32,
    pub turn_count: u32,
    pub tool_count: u32,
}

/// Full state for the status sub-page (snapshot-on-open, no live lock).
pub struct StatusPageState {
    pub active_tab: StatusTab,
    pub session: SessionSnapshot,
    pub config: ConfigSnapshot,
    pub usage: UsageSnapshot,
    /// Per-tab scroll offsets indexed by `StatusTab::index()`.
    pub scroll_offsets: [usize; 3],
    /// Filter text for the Config tab search.
    pub filter: String,
    /// Cursor position within the filter text.
    pub filter_cursor: usize,
}

impl StatusPageState {
    /// Return config entries matching the current filter.
    pub fn filtered_config(&self) -> Vec<&ConfigEntry> {
        if self.filter.is_empty() {
            self.config.entries.iter().collect()
        } else {
            let lower = self.filter.to_ascii_lowercase();
            self.config
                .entries
                .iter()
                .filter(|e| {
                    e.key.to_ascii_lowercase().contains(&lower)
                        || e.value.to_ascii_lowercase().contains(&lower)
                })
                .collect()
        }
    }

    /// Mutable reference to the active tab's scroll offset.
    pub fn active_scroll_mut(&mut self) -> &mut usize {
        &mut self.scroll_offsets[self.active_tab.index()]
    }

    /// Current tab's scroll offset.
    pub fn active_scroll(&self) -> usize {
        self.scroll_offsets[self.active_tab.index()]
    }

    /// Number of content rows in the active tab (for scroll clamping).
    pub fn active_row_count(&self) -> usize {
        match self.active_tab {
            StatusTab::Status => STATUS_TAB_ROWS,
            StatusTab::Config => self.filtered_config().len(),
            StatusTab::Usage => USAGE_TAB_ROWS,
        }
    }
}

/// Fixed row count for the Status tab.
pub const STATUS_TAB_ROWS: usize = 9;
/// Fixed row count for the Usage tab (including separator rows).
pub const USAGE_TAB_ROWS: usize = 7;

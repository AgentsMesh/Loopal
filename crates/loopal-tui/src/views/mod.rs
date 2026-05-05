pub mod agent_panel;
pub mod bg_task_log;
pub mod bg_tasks_panel;
pub mod breadcrumb;
pub mod command_menu;
pub mod cron_duration_format;
pub mod crons_panel;
pub mod input_view;
pub mod mcp_action_menu;
pub mod mcp_page;
pub mod panel_header;
pub mod permission_inline;
pub mod picker;
pub mod progress;
pub mod question_inline;
mod question_layout;
pub mod retry_banner;
pub mod rewind_picker;
pub mod separator;
pub mod skills_page;
pub mod status_page;
pub mod tasks_panel;
pub mod text_width;
pub mod topology_overlay;
pub mod unified_status;

/// Shared dim-grey color used for separators and inactive panel decoration.
pub const DIM_SEPARATOR: ratatui::style::Color = ratatui::style::Color::Rgb(60, 60, 60);

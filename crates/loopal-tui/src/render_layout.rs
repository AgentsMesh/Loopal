//! Frame layout computation — pure function of dimensions → rectangles.

use ratatui::prelude::*;

/// Computed frame layout for one render pass.
pub(crate) struct FrameLayout {
    pub breadcrumb: Rect,
    pub content: Rect,
    pub agents: Rect,
    pub separator: Rect,
    pub retry_banner: Rect,
    pub input: Rect,
    pub status: Rect,
    /// Merged area for sub-page pickers (replaces breadcrumb..input).
    pub picker: Rect,
}

impl FrameLayout {
    pub fn compute(
        size: Rect,
        breadcrumb_h: u16,
        panel_zone_h: u16,
        banner_h: u16,
        input_h: u16,
    ) -> Self {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(breadcrumb_h),
                Constraint::Min(3),
                Constraint::Length(panel_zone_h),
                Constraint::Length(1),
                Constraint::Length(banner_h),
                Constraint::Length(input_h),
                Constraint::Length(1),
            ])
            .split(size);

        let [
            breadcrumb,
            content,
            agents,
            separator,
            retry_banner,
            input,
            status,
        ] = [
            chunks[0], chunks[1], chunks[2], chunks[3], chunks[4], chunks[5], chunks[6],
        ];

        let picker = Rect::new(
            breadcrumb.x,
            breadcrumb.y,
            breadcrumb.width,
            breadcrumb.height
                + content.height
                + agents.height
                + separator.height
                + retry_banner.height
                + input.height,
        );

        Self {
            breadcrumb,
            content,
            agents,
            separator,
            retry_banner,
            input,
            status,
            picker,
        }
    }
}

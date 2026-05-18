//! Sub-page entry + stop side effects (Agent / BgTask / Cron / Task drill-in).
//!
//! Split from `key_dispatch_ops` to keep both files within the 200-line
//! cap while preserving the SRP boundary: this file owns transitions
//! into per-panel sub-pages and the `x` stop action that closes them.

use loopal_protocol::ControlCommand;

use crate::app::{App, PanelKind};

pub(crate) fn enter_agent_view(app: &mut App) {
    let Some(name) = app.section(PanelKind::Agents).focused.clone() else {
        return;
    };
    if !app.is_agent_live(&name) {
        return;
    }
    if !app.session.enter_agent_view(&name) {
        return;
    }
    app.focus_mode = crate::app::FocusMode::Input;
    app.content_scroll.reset();
    app.last_esc_time = None;
}

pub(crate) fn enter_bg_task_view(app: &mut App) {
    let Some(task_id) = app.section(PanelKind::BgTasks).focused.clone() else {
        return;
    };
    app.sub_page = Some(crate::app::SubPage::BgTaskLog(crate::app::BgTaskLogState {
        task_id,
        scroll_offset: 0,
        auto_follow: true,
        prev_line_count: 0,
    }));
    app.focus_mode = crate::app::FocusMode::Input;
}

pub(crate) fn enter_cron_view(app: &mut App) {
    let Some(cron_id) = app.section(PanelKind::Crons).focused.clone() else {
        return;
    };
    app.sub_page = Some(crate::app::SubPage::CronDetail(
        crate::app::CronDetailState { cron_id },
    ));
    app.focus_mode = crate::app::FocusMode::Input;
}

pub(crate) fn enter_task_view(app: &mut App) {
    let Some(task_id) = app.section(PanelKind::Tasks).focused.clone() else {
        return;
    };
    app.sub_page = Some(crate::app::SubPage::TaskDetail(
        crate::app::TaskDetailState {
            task_id,
            scroll_offset: 0,
        },
    ));
    app.focus_mode = crate::app::FocusMode::Input;
}

pub(crate) async fn stop_focused_sub_page_item(app: &mut App) {
    let Some(ref sub_page) = app.sub_page else {
        return;
    };
    let agent = app.session.lock().active_view.clone();
    let cmd = match sub_page {
        crate::app::SubPage::BgTaskLog(s) => Some(ControlCommand::BgTaskKill {
            id: s.task_id.clone(),
        }),
        crate::app::SubPage::CronDetail(s) => Some(ControlCommand::CronDelete {
            id: s.cron_id.clone(),
        }),
        _ => None,
    };
    let Some(cmd) = cmd else { return };
    app.session.send_control(agent, cmd).await;
    app.sub_page = None;
}

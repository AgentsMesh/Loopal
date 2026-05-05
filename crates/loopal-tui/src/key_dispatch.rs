//! Key-action dispatch — maps InputAction → side effects + quit flag.

use loopal_protocol::AgentMode;

use crate::app::App;
use crate::event::EventHandler;
use crate::input::paste;
use crate::input::{InputAction, handle_key};
use crate::key_dispatch_ops::{
    cycle_panel_focus, enter_panel, handle_effect, handle_sub_page_confirm, panel_tab,
    push_to_inbox, terminate_focused_agent,
};

/// Process a single key event and return `true` if the TUI should quit.
pub(crate) async fn handle_key_action(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    events: &EventHandler,
) -> bool {
    let action = handle_key(app, key);
    match action {
        InputAction::Quit => {
            app.exiting = true;
            true
        }
        InputAction::InboxPush(content) => {
            push_to_inbox(app, content).await;
            false
        }
        InputAction::PasteRequested => {
            paste::spawn_paste(events);
            false
        }
        InputAction::ToolApprove => {
            crate::key_dispatch_ops::tool_approve(app).await;
            false
        }
        InputAction::ToolDeny => {
            crate::key_dispatch_ops::tool_deny(app).await;
            false
        }
        InputAction::Interrupt => {
            app.session.interrupt();
            false
        }
        InputAction::ModeSwitch(mode) => {
            let m = if mode == "plan" {
                AgentMode::Plan
            } else {
                AgentMode::Act
            };
            app.session.switch_mode(m).await;
            false
        }
        InputAction::RunCommand(name, arg) => {
            if let Some(handler) = app.command_registry.find(&name) {
                let effect = handler.execute(app, arg.as_deref()).await;
                handle_effect(app, effect).await
            } else {
                false
            }
        }
        InputAction::SubPageConfirm(result) => {
            handle_sub_page_confirm(app, result).await;
            false
        }
        InputAction::EnterPanel => {
            enter_panel(app);
            false
        }
        InputAction::ExitPanel => {
            app.focus_mode = crate::app::FocusMode::Input;
            false
        }
        InputAction::PanelTab => {
            panel_tab(app);
            false
        }
        InputAction::PanelUp => {
            cycle_panel_focus(app, false);
            false
        }
        InputAction::PanelDown => {
            cycle_panel_focus(app, true);
            false
        }
        InputAction::TerminateFocusedAgent => {
            terminate_focused_agent(app).await;
            false
        }
        InputAction::EnterAgentView => {
            if let Some(name) = app.section(crate::app::PanelKind::Agents).focused.clone()
                && app.is_agent_live(&name)
                && app.session.enter_agent_view(&name)
            {
                app.focus_mode = crate::app::FocusMode::Input;
                app.content_scroll.reset();
                app.last_esc_time = None;
            }
            false
        }
        InputAction::EnterBgTaskView => {
            if let Some(ref task_id) = app.section(crate::app::PanelKind::BgTasks).focused {
                app.sub_page = Some(crate::app::SubPage::BgTaskLog(crate::app::BgTaskLogState {
                    task_id: task_id.clone(),
                    scroll_offset: 0,
                    auto_follow: true,
                    prev_line_count: 0,
                }));
                app.focus_mode = crate::app::FocusMode::Input;
            }
            false
        }
        InputAction::ExitAgentView => {
            app.session.exit_agent_view();
            app.content_scroll.reset();
            app.last_esc_time = None;
            false
        }
        InputAction::QuestionUp => {
            crate::question_ops::cursor_up(app);
            false
        }
        InputAction::QuestionDown => {
            crate::question_ops::cursor_down(app);
            false
        }
        InputAction::QuestionToggle => {
            crate::question_ops::toggle(app);
            false
        }
        InputAction::QuestionConfirm => {
            crate::question_ops::confirm(app).await;
            false
        }
        InputAction::QuestionCancel => {
            crate::question_ops::cancel(app).await;
            false
        }
        InputAction::QuestionFreeTextChar(c) => {
            crate::question_ops::free_text_char(app, c);
            false
        }
        InputAction::QuestionFreeTextBackspace => {
            crate::question_ops::free_text_backspace(app);
            false
        }
        InputAction::QuestionFreeTextDelete => {
            crate::question_ops::free_text_delete(app);
            false
        }
        InputAction::QuestionFreeTextCursorLeft => {
            crate::question_ops::free_text_cursor_left(app);
            false
        }
        InputAction::QuestionFreeTextCursorRight => {
            crate::question_ops::free_text_cursor_right(app);
            false
        }
        InputAction::QuestionFreeTextHome => {
            crate::question_ops::free_text_home(app);
            false
        }
        InputAction::QuestionFreeTextEnd => {
            crate::question_ops::free_text_end(app);
            false
        }
        InputAction::McpReconnect(server) => {
            crate::key_dispatch_ops::mcp_reconnect(app, server).await;
            false
        }
        InputAction::McpDisconnect(server) => {
            crate::key_dispatch_ops::mcp_disconnect(app, server).await;
            false
        }
        InputAction::None => false,
    }
}

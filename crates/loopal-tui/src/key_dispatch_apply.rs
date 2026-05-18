//! Single source of truth for `InputAction` → side-effect dispatch.
//!
//! Production `handle_key_action` and test `key_dispatch_for_test::dispatch`
//! both call `apply_action` so the routing table never drifts. The two
//! callers only differ in how they consume the returned `DispatchOutcome`.

use loopal_protocol::AgentMode;

use crate::app::App;
use crate::input::InputAction;
use crate::key_dispatch_ops::{
    cycle_panel_focus, enter_panel, handle_effect, handle_sub_page_confirm, panel_tab,
    push_to_inbox, terminate_focused_agent,
};
use crate::key_dispatch_subpage::{
    enter_agent_view, enter_bg_task_view, enter_cron_view, enter_task_view,
    stop_focused_sub_page_item,
};

/// Signals that `apply_action` cannot resolve itself and must defer to
/// the caller. Production handles all variants; tests only care about
/// `Continue` (paste / quit don't drive any test assertion today).
pub(crate) enum DispatchOutcome {
    Continue,
    Quit,
    PasteRequested,
}

pub(crate) async fn apply_action(app: &mut App, action: InputAction) -> DispatchOutcome {
    match action {
        InputAction::None => DispatchOutcome::Continue,
        InputAction::Quit => DispatchOutcome::Quit,
        InputAction::PasteRequested => DispatchOutcome::PasteRequested,
        InputAction::InboxPush(content) => {
            push_to_inbox(app, content).await;
            DispatchOutcome::Continue
        }
        InputAction::ToolApprove => {
            crate::key_dispatch_ops::tool_approve(app).await;
            DispatchOutcome::Continue
        }
        InputAction::ToolDeny => {
            crate::key_dispatch_ops::tool_deny(app).await;
            DispatchOutcome::Continue
        }
        InputAction::Interrupt => {
            app.session.interrupt();
            DispatchOutcome::Continue
        }
        InputAction::ModeSwitch(mode) => {
            let m = if mode == "plan" {
                AgentMode::Plan
            } else {
                AgentMode::Act
            };
            app.session.switch_mode(m).await;
            DispatchOutcome::Continue
        }
        InputAction::RunCommand(name, arg) => {
            let Some(handler) = app.command_registry.find(&name) else {
                return DispatchOutcome::Continue;
            };
            let effect = handler.execute(app, arg.as_deref()).await;
            // handle_effect mutates app.exiting directly + returns the
            // quit flag; we surface it via Quit so the outcome stays
            // the single signal channel.
            if handle_effect(app, effect).await {
                DispatchOutcome::Quit
            } else {
                DispatchOutcome::Continue
            }
        }
        InputAction::SubPageConfirm(result) => {
            handle_sub_page_confirm(app, result).await;
            DispatchOutcome::Continue
        }
        InputAction::EnterPanel => {
            enter_panel(app);
            DispatchOutcome::Continue
        }
        InputAction::ExitPanel => {
            app.focus_mode = crate::app::FocusMode::Input;
            DispatchOutcome::Continue
        }
        InputAction::PanelTab => {
            panel_tab(app);
            DispatchOutcome::Continue
        }
        InputAction::PanelUp => {
            cycle_panel_focus(app, false);
            DispatchOutcome::Continue
        }
        InputAction::PanelDown => {
            cycle_panel_focus(app, true);
            DispatchOutcome::Continue
        }
        InputAction::TerminateFocusedAgent => {
            terminate_focused_agent(app).await;
            DispatchOutcome::Continue
        }
        InputAction::EnterAgentView => {
            enter_agent_view(app);
            DispatchOutcome::Continue
        }
        InputAction::EnterBgTaskView => {
            enter_bg_task_view(app);
            DispatchOutcome::Continue
        }
        InputAction::EnterCronView => {
            enter_cron_view(app);
            DispatchOutcome::Continue
        }
        InputAction::EnterTaskView => {
            enter_task_view(app);
            DispatchOutcome::Continue
        }
        InputAction::StopFocusedSubPageItem => {
            stop_focused_sub_page_item(app).await;
            DispatchOutcome::Continue
        }
        InputAction::ExitAgentView => {
            app.session.exit_agent_view();
            app.content_scroll.reset();
            app.last_esc_time = None;
            DispatchOutcome::Continue
        }
        InputAction::QuestionUp => {
            crate::question_ops::cursor_up(app);
            DispatchOutcome::Continue
        }
        InputAction::QuestionDown => {
            crate::question_ops::cursor_down(app);
            DispatchOutcome::Continue
        }
        InputAction::QuestionPrev => {
            crate::question_ops::prev_question(app);
            DispatchOutcome::Continue
        }
        InputAction::QuestionNext => {
            crate::question_ops::next_question(app);
            DispatchOutcome::Continue
        }
        InputAction::QuestionToggle => {
            crate::question_ops::toggle(app);
            DispatchOutcome::Continue
        }
        InputAction::QuestionConfirm => {
            crate::question_ops::confirm(app).await;
            DispatchOutcome::Continue
        }
        InputAction::QuestionCancel => {
            crate::question_ops::cancel(app).await;
            DispatchOutcome::Continue
        }
        InputAction::QuestionFreeTextChar(c) => {
            crate::question_ops::free_text_char(app, c);
            DispatchOutcome::Continue
        }
        InputAction::QuestionFreeTextBackspace => {
            crate::question_ops::free_text_backspace(app);
            DispatchOutcome::Continue
        }
        InputAction::QuestionFreeTextDelete => {
            crate::question_ops::free_text_delete(app);
            DispatchOutcome::Continue
        }
        InputAction::QuestionFreeTextCursorLeft => {
            crate::question_ops::free_text_cursor_left(app);
            DispatchOutcome::Continue
        }
        InputAction::QuestionFreeTextCursorRight => {
            crate::question_ops::free_text_cursor_right(app);
            DispatchOutcome::Continue
        }
        InputAction::QuestionFreeTextHome => {
            crate::question_ops::free_text_home(app);
            DispatchOutcome::Continue
        }
        InputAction::QuestionFreeTextEnd => {
            crate::question_ops::free_text_end(app);
            DispatchOutcome::Continue
        }
        InputAction::McpReconnect(server) => {
            crate::key_dispatch_ops::mcp_reconnect(app, server).await;
            DispatchOutcome::Continue
        }
        InputAction::McpDisconnect(server) => {
            crate::key_dispatch_ops::mcp_disconnect(app, server).await;
            DispatchOutcome::Continue
        }
    }
}

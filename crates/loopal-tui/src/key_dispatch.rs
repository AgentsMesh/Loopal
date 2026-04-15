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
            let has = app
                .session
                .lock()
                .active_conversation()
                .pending_permission
                .is_some();
            if has {
                app.session.approve_permission().await;
            }
            false
        }
        InputAction::ToolDeny => {
            let has = app
                .session
                .lock()
                .active_conversation()
                .pending_permission
                .is_some();
            if has {
                app.session.deny_permission().await;
            }
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
            if let Some(name) = app.focused_agent.clone()
                && app.session.enter_agent_view(&name)
            {
                app.focus_mode = crate::app::FocusMode::Input;
                app.content_scroll.reset();
                app.last_esc_time = None;
            }
            false
        }
        InputAction::EnterBgTaskView => {
            if let Some(ref task_id) = app.focused_bg_task {
                app.sub_page = Some(crate::app::SubPage::BgTaskLog(
                    crate::app::BgTaskLogState {
                        task_id: task_id.clone(),
                        scroll_offset: 0,
                        auto_follow: true,
                        prev_line_count: 0,
                    },
                ));
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
            if let Some(ref mut q) = app
                .session
                .lock()
                .active_conversation_mut()
                .pending_question
            {
                q.cursor_up();
            }
            false
        }
        InputAction::QuestionDown => {
            if let Some(ref mut q) = app
                .session
                .lock()
                .active_conversation_mut()
                .pending_question
            {
                q.cursor_down();
            }
            false
        }
        InputAction::QuestionToggle => {
            if let Some(ref mut q) = app
                .session
                .lock()
                .active_conversation_mut()
                .pending_question
            {
                q.toggle();
            }
            false
        }
        InputAction::QuestionConfirm => {
            let answers = app
                .session
                .lock()
                .active_conversation()
                .pending_question
                .as_ref()
                .map(|q| {
                    let ans = q.get_answers();
                    if ans.is_empty() && !q.questions[q.current_question].allow_multiple {
                        vec![
                            q.questions[q.current_question].options[q.cursor]
                                .label
                                .clone(),
                        ]
                    } else {
                        ans
                    }
                });
            if let Some(answers) = answers {
                app.session.answer_question(answers).await;
            }
            false
        }
        InputAction::QuestionCancel => {
            app.session
                .answer_question(vec!["(cancelled)".into()])
                .await;
            false
        }
        InputAction::None => false,
    }
}

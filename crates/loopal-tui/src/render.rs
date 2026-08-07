//! Frame composition — combines all views into the terminal frame.

use ratatui::prelude::*;

use crate::app::{App, SubPage};
use crate::render_layout::FrameLayout;
use crate::views;
use crate::views::input_view;

/// Compose all views into the frame.
pub fn draw(f: &mut Frame, app: &mut App) {
    draw_with_animation_elapsed(f, app, crate::animation::elapsed());
}

pub(crate) fn draw_with_animation_elapsed(
    f: &mut Frame,
    app: &mut App,
    animation_elapsed: std::time::Duration,
) {
    let size = f.area();
    let state = app.session.lock();
    let active = state.active_view.clone();
    let vc = app.view_client_for(&active);
    let vc_guard = vc.state();
    let conv = vc_guard.conversation();

    let pw = input_view::prefix_width(app.pending_image_count());
    let compact_banner_h = views::compact_progress::banner_height(&conv.compact_banner);
    let retry_banner_h = views::retry_banner::banner_height(&conv.retry_banner);
    let breadcrumb_h = u16::from(state.active_view != loopal_session::ROOT_AGENT);
    let turn_elapsed = conv.turn_elapsed();

    // PendingQuestion is cloned here so we can drop `vc_guard` before the
    // input area renders below. Questions are typically 1-4 short strings
    // and TUI redraws are event-driven (not 60fps), so the clone cost is
    // negligible. Permission uses the lighter `prepare` borrow path because
    // its `input` JSON can be much larger.
    let pending_question = conv.pending_question.clone();
    let pending_plan = conv.pending_plan_approval.clone();
    let prepared_perm = conv
        .pending_permission
        .as_ref()
        .map(views::permission_inline::prepare);

    let input_h = if let Some(ref q) = pending_question {
        views::question_inline::height(q, size.width)
    } else if let Some(ref prep) = prepared_perm {
        views::permission_inline::height_of(prep)
    } else if let Some(ref plan) = pending_plan {
        views::plan_approval_inline::height(plan, size.width)
    } else {
        input_view::input_height(&app.input, size.width, pw)
    };

    let panel_zone_h = crate::render_panel::panel_zone_height(app, &state);
    let layout = FrameLayout::compute(
        size,
        breadcrumb_h,
        panel_zone_h,
        compact_banner_h,
        retry_banner_h,
        input_h,
    );

    if let Some(ref mut sub_page) = app.sub_page {
        let cron_snapshots = vc.cron_snapshots();
        let task_snapshots = vc.task_snapshots();
        render_sub_page(
            f,
            sub_page,
            &app.bg_task_details,
            &cron_snapshots,
            &task_snapshots,
            layout.picker,
        );
        views::unified_status::render_unified_status(
            f,
            app,
            &state,
            conv,
            animation_elapsed,
            layout.status,
        );
        return;
    }

    if breadcrumb_h > 0 {
        views::breadcrumb::render_breadcrumb(f, &state.active_view, layout.breadcrumb);
    }
    app.content_scroll
        .render_with_animation_elapsed(f, conv, animation_elapsed, layout.content);
    crate::render_panel::render_panel_zone(f, app, &state, animation_elapsed, layout.agents);
    views::separator::render_separator(f, layout.separator);
    if let Some(ref msg) = conv.compact_banner {
        views::compact_progress::render_compact_banner(f, msg, layout.compact_banner);
    }
    if let Some(ref msg) = conv.retry_banner {
        views::retry_banner::render_retry_banner(f, msg, layout.retry_banner);
    }
    views::unified_status::render_unified_status(
        f,
        app,
        &state,
        conv,
        animation_elapsed,
        layout.status,
    );

    let topology_data = if app.show_topology {
        Some(extract_topology(app, &state, turn_elapsed))
    } else {
        None
    };
    drop(vc_guard);
    drop(state);

    if let Some(ref question) = pending_question {
        let status = app.current_transient_status().map(String::from);
        views::question_inline::render(f, question, layout.input, status.as_deref());
    } else if let Some(ref prep) = prepared_perm {
        let status = app.current_transient_status().map(String::from);
        views::permission_inline::render_prepared(f, prep, layout.input, status.as_deref());
    } else if let Some(ref plan) = pending_plan {
        let viewport_rows = views::plan_approval_inline::content_viewport_rows(layout.input.height);
        app.plan_approval_viewport_rows = viewport_rows;
        app.plan_approval_scroll = app
            .plan_approval_scroll
            .min(views::plan_approval_inline::max_scroll(plan, viewport_rows));
        let status = app.current_transient_status().map(String::from);
        views::plan_approval_inline::render(
            f,
            plan,
            app.plan_approval_scroll,
            layout.input,
            status.as_deref(),
        );
    } else {
        let image_count = app.pending_image_count();
        views::input_view::render_input(
            f,
            &app.input,
            app.input_cursor,
            image_count,
            app.input_scroll,
            layout.input,
        );
        if let Some(ref ac) = app.autocomplete {
            views::command_menu::render_command_menu(f, ac, layout.input);
        }
    }
    if let Some(ref nodes) = topology_data {
        views::topology_overlay::render_topology_overlay(f, nodes, animation_elapsed, size);
    }
}

fn render_sub_page(
    f: &mut Frame,
    sub_page: &mut SubPage,
    bg_details: &[loopal_protocol::BgTaskDetail],
    crons: &[loopal_protocol::CronJobSnapshot],
    tasks: &[loopal_protocol::TaskSnapshot],
    area: Rect,
) {
    match sub_page {
        SubPage::ModelPicker(p) | SubPage::SessionPicker(p) => {
            views::picker::render_picker(f, p, area);
        }
        SubPage::EnumPicker { state, .. } => {
            views::picker::render_picker(f, state, area);
        }
        SubPage::RewindPicker(r) => views::rewind_picker::render_rewind_picker(f, r, area),
        SubPage::StatusPage(s) => views::status_page::render_status_page(f, s, area),
        SubPage::McpPage(s) => views::mcp_page::render_mcp_page(f, s, area),
        SubPage::SkillsPage(s) => views::skills_page::render_skills_page(f, s, area),
        SubPage::BgTaskLog(s) => views::bg_task_log::render_bg_task_log(f, s, bg_details, area),
        SubPage::CronDetail(s) => views::cron_detail::render_cron_detail(f, s, crons, area),
        SubPage::TaskDetail(s) => views::task_detail::render_task_detail(f, s, tasks, area),
    }
}

fn extract_topology(
    app: &App,
    state: &loopal_session::state::SessionState,
    elapsed: std::time::Duration,
) -> Vec<views::topology_overlay::TopologyNode> {
    use indexmap::IndexMap;
    use loopal_protocol::AgentStatus;
    use views::topology_overlay::AgentTopologySnapshot;

    let root_idle = matches!(
        app.observable_for(&state.active_view).status,
        AgentStatus::WaitingForInput | AgentStatus::Finished | AgentStatus::Error
    );
    let root_status = if root_idle {
        AgentStatus::WaitingForInput
    } else {
        AgentStatus::Running
    };

    let agents: IndexMap<String, AgentTopologySnapshot> = app
        .view_clients
        .iter()
        .map(|(name, vc)| {
            let guard = vc.state();
            let view = &guard.state().agent;
            (
                name.clone(),
                AgentTopologySnapshot {
                    status: view.observable.status,
                    model: view.observable.model.clone(),
                    elapsed: view.elapsed(),
                    tools_in_flight: view.tools_in_flight(),
                    parent: view.parent.clone(),
                    children: view.children.clone(),
                },
            )
        })
        .collect();

    let root_model = app.observable_for(&state.active_view).model;
    views::topology_overlay::extract_topology(&agents, &root_model, root_status, elapsed)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use loopal_protocol::{
        AgentEvent, AgentEventPayload, ControlCommand, TaskSnapshot, TaskSnapshotStatus,
        UserQuestionResponse,
    };
    use loopal_session::SessionController;
    use loopal_view_state::{ViewSnapshot, ViewStateReducer};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tokio::sync::{mpsc, watch};

    use super::*;
    use crate::view_client::ViewClient;

    fn app_from_running_wire_snapshot() -> App {
        let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
        let (permission_tx, _) = mpsc::channel::<bool>(16);
        let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
        let session = SessionController::new(
            control_tx,
            permission_tx,
            question_tx,
            Default::default(),
            Arc::new(watch::channel(0u64).0),
        );
        let mut app = App::new(session, std::env::temp_dir());

        let mut hub_reducer = ViewStateReducer::new("main");
        hub_reducer.apply(AgentEventPayload::Running);
        hub_reducer.apply(AgentEventPayload::TasksChanged {
            tasks: vec![TaskSnapshot {
                id: "1".into(),
                subject: "foreground".into(),
                active_form: None,
                status: TaskSnapshotStatus::InProgress,
                blocked_by: Vec::new(),
                description: String::new(),
                blocks: Vec::new(),
            }],
        });
        hub_reducer.apply(AgentEventPayload::BgTaskSpawned {
            id: "bg_1".into(),
            description: "background".into(),
            created_at_unix_ms: 0,
        });
        hub_reducer.apply(AgentEventPayload::ToolCall {
            id: "tool-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/tmp/input"}),
        });
        // A later row makes the active tool message non-final. Tool progress,
        // completion, and animation must still invalidate that earlier row in
        // the incremental line cache.
        hub_reducer.apply(AgentEventPayload::ProviderWarning {
            message: "provider notice after tool start".into(),
        });

        let mut worker_reducer = ViewStateReducer::new("worker");
        worker_reducer.apply(AgentEventPayload::Running);
        worker_reducer.apply(AgentEventPayload::ToolCall {
            id: "worker-tool-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/tmp/worker-input"}),
        });

        // Reproduce the Hub -> TUI snapshot boundary. Process-local Instant
        // anchors are intentionally skipped by serde, while Running and the
        // active task/tool snapshots remain authoritative.
        let wire = serde_json::to_string(&hub_reducer.snapshot()).expect("serialize snapshot");
        let snapshot: ViewSnapshot = serde_json::from_str(&wire).expect("deserialize snapshot");
        let snapshot_rev = snapshot.rev;
        app.view_clients
            .insert("main".into(), ViewClient::from_snapshot("main", snapshot));
        let worker_wire =
            serde_json::to_string(&worker_reducer.snapshot()).expect("serialize worker snapshot");
        let worker_snapshot: ViewSnapshot =
            serde_json::from_str(&worker_wire).expect("deserialize worker snapshot");
        app.view_clients.insert(
            "worker".into(),
            ViewClient::from_snapshot("worker", worker_snapshot),
        );

        // Production can receive the already-snapshotted Running event from
        // the subscription queue. It is correctly deduplicated and therefore
        // cannot recreate the skipped process-local timer.
        let mut queued_running = AgentEvent::root(AgentEventPayload::Running);
        queued_running.rev = Some(snapshot_rev);
        app.view_clients["main"].apply_event(&queued_running);
        assert_eq!(app.view_clients["main"].rev(), snapshot_rev);

        app
    }

    fn render_at(app: &mut App, animation_elapsed: Duration) -> String {
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
        terminal
            .draw(|frame| draw_with_animation_elapsed(frame, app, animation_elapsed))
            .expect("draw frame");
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn snapshot_resubscribe_spinners_advance_on_the_tui_animation_clock() {
        let mut app = app_from_running_wire_snapshot();

        // The serde snapshot dropped the Running Instant and the same-rev
        // live event was deduplicated, reproducing the original frozen-icon
        // failure before exercising the actual frame renderer below.
        assert_eq!(
            app.view_clients["main"]
                .state()
                .conversation()
                .turn_elapsed(),
            Duration::ZERO
        );

        let first = render_at(&mut app, Duration::ZERO);
        let second = render_at(&mut app, Duration::from_millis(100));

        assert!(
            first.contains("⠋ Working"),
            "unified status spinner missing: {first}"
        );
        assert!(
            second.contains("⠙ Working"),
            "unified status spinner did not advance: {second}"
        );
        assert!(
            first.contains("worker  ⠋ Working"),
            "agent panel spinner missing: {first}"
        );
        assert!(
            second.contains("worker  ⠙ Working"),
            "agent panel spinner did not advance: {second}"
        );
        assert!(
            first.contains("⠋ worker"),
            "topology spinner missing: {first}"
        );
        assert!(
            second.contains("⠙ worker"),
            "topology spinner did not advance: {second}"
        );
        assert!(first.contains("⠋ #1"), "task spinner missing: {first}");
        assert!(
            second.contains("⠙ #1"),
            "task spinner did not advance: {second}"
        );
        assert!(first.contains("⠋ bg_1"), "bg spinner missing: {first}");
        assert!(
            second.contains("⠙ bg_1"),
            "bg spinner did not advance: {second}"
        );
        assert!(first.contains("⠋ Read"), "tool spinner missing: {first}");
        assert!(
            second.contains("⠙ Read"),
            "tool spinner did not advance: {second}"
        );
        assert!(
            second.contains("provider notice after tool start"),
            "later system row missing: {second}"
        );
    }

    #[test]
    fn earlier_tool_completion_invalidates_after_a_later_row() {
        let mut app = app_from_running_wire_snapshot();
        let running = render_at(&mut app, Duration::from_millis(100));
        assert!(
            running.contains("⠙ Read"),
            "tool spinner missing: {running}"
        );
        assert!(running.contains("provider notice after tool start"));

        app.view_clients["main"].apply_event(&AgentEvent::root(AgentEventPayload::ToolResult {
            id: "tool-1".into(),
            name: "Read".into(),
            result: "first\nsecond".into(),
            is_error: false,
            duration_ms: Some(10),
            metadata: None,
        }));

        let completed = render_at(&mut app, Duration::from_millis(100));
        assert!(
            completed.contains("● Read"),
            "completed tool row stayed stale: {completed}"
        );
        assert!(
            completed.contains("Read 2 lines"),
            "completed tool body was not rebuilt: {completed}"
        );
        assert!(completed.contains("provider notice after tool start"));
        assert!(
            !completed.contains("⠙ Read"),
            "active icon survived terminal transition: {completed}"
        );
    }
}

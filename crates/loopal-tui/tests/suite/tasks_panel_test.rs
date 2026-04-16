//! Tests for tasks_panel rendering and height computation.

use loopal_protocol::{TaskSnapshot, TaskSnapshotStatus};
use loopal_tui::views::tasks_panel::{render_tasks_panel, task_ids, tasks_panel_height};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::prelude::*;

fn snap(id: &str, status: TaskSnapshotStatus) -> TaskSnapshot {
    TaskSnapshot {
        id: id.into(),
        subject: format!("Task {id}"),
        active_form: None,
        status,
        blocked_by: Vec::new(),
    }
}

#[test]
fn height_zero_when_empty() {
    assert_eq!(tasks_panel_height(&[]), 0);
}

#[test]
fn height_counts_non_completed() {
    let tasks = vec![
        snap("1", TaskSnapshotStatus::Pending),
        snap("2", TaskSnapshotStatus::InProgress),
        snap("3", TaskSnapshotStatus::Completed),
    ];
    assert_eq!(tasks_panel_height(&tasks), 2);
}

#[test]
fn height_capped_at_max() {
    let tasks: Vec<_> = (1..=10)
        .map(|i| snap(&i.to_string(), TaskSnapshotStatus::Pending))
        .collect();
    assert_eq!(tasks_panel_height(&tasks), 5);
}

#[test]
fn height_zero_when_all_completed() {
    let tasks = vec![
        snap("1", TaskSnapshotStatus::Completed),
        snap("2", TaskSnapshotStatus::Completed),
    ];
    assert_eq!(tasks_panel_height(&tasks), 0);
}

#[test]
fn task_ids_excludes_completed() {
    let tasks = vec![
        snap("1", TaskSnapshotStatus::InProgress),
        snap("2", TaskSnapshotStatus::Completed),
        snap("3", TaskSnapshotStatus::Pending),
    ];
    let ids = task_ids(&tasks);
    assert_eq!(ids, vec!["1", "3"]);
}

#[test]
fn render_with_blocked_and_active_form_no_panic() {
    let tasks = vec![
        TaskSnapshot {
            id: "1".into(),
            subject: "Build feature".into(),
            active_form: Some("Building".into()),
            status: TaskSnapshotStatus::InProgress,
            blocked_by: Vec::new(),
        },
        TaskSnapshot {
            id: "2".into(),
            subject: "Deploy".into(),
            active_form: None,
            status: TaskSnapshotStatus::Pending,
            blocked_by: vec!["1".into()],
        },
    ];
    let backend = TestBackend::new(80, 2);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 2);
            render_tasks_panel(f, &tasks, Some("1"), std::time::Duration::ZERO, 0, area);
        })
        .unwrap();
}

#[test]
fn render_empty_area_no_panic() {
    let tasks = vec![snap("1", TaskSnapshotStatus::InProgress)];
    let backend = TestBackend::new(80, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 0);
            render_tasks_panel(f, &tasks, None, std::time::Duration::ZERO, 0, area);
        })
        .unwrap();
}

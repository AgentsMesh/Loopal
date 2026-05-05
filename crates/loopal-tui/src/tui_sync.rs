use crate::app::App;

pub(crate) fn sync_panel_caches(app: &mut App) {
    let bg_snapshots = {
        let vc = app.active_view_client();
        let guard = vc.state();
        guard
            .state()
            .bg_tasks
            .values()
            .map(|v| {
                (
                    v.id.clone(),
                    v.description.clone(),
                    v.status,
                    v.exit_code,
                    v.output.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    crate::session_cleanup::merge_bg_details_from_view(&mut app.bg_task_details, &bg_snapshots);
    crate::session_cleanup::cap_bg_details(&mut app.bg_task_details);
    let active = app.session.lock().active_view.clone();
    crate::session_cleanup::cleanup_view_clients(&mut app.view_clients, &active);
    clamp_scroll_offsets(app);
}

fn clamp_scroll_offsets(app: &mut App) {
    let clamps: Vec<_> = {
        let state = app.session.lock();
        app.panel_registry
            .providers()
            .iter()
            .map(|p| (p.kind(), p.item_ids(app, &state).len(), p.max_visible()))
            .collect()
    };
    for (kind, count, max) in clamps {
        let section = app.section_mut(kind);
        section.scroll_offset = section.scroll_offset.min(count.saturating_sub(max));
    }
}

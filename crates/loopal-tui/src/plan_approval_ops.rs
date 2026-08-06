use crate::app::App;
use crate::views::plan_approval_inline;

pub(crate) fn scroll(app: &mut App, delta: i32) {
    let viewport_rows = app.plan_approval_viewport_rows;
    let max = app.with_active_conversation(|conv| {
        conv.pending_plan_approval
            .as_ref()
            .map(|plan| plan_approval_inline::max_scroll(plan, viewport_rows))
            .unwrap_or(0)
    });
    app.plan_approval_scroll = if delta < 0 {
        app.plan_approval_scroll
            .saturating_sub(delta.unsigned_abs() as usize)
    } else {
        app.plan_approval_scroll
            .saturating_add(delta as usize)
            .min(max)
    };
}

pub(crate) async fn resolve(app: &mut App, approve: bool) {
    app.clear_transient_status();
    // The Hub's resolved event is authoritative. Keep the prompt visible if
    // the response RPC fails so the user can retry instead of losing it.
    let pending = app.with_active_conversation(|conv| conv.pending_plan_approval.clone());
    if let Some(plan) = pending {
        let agent = app.session.lock().active_view.clone();
        app.session
            .respond_plan_approval(&agent, &plan.id, approve)
            .await;
    }
}

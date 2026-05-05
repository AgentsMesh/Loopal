use ratatui::prelude::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn compose(
    cap: usize,
    title_height: usize,
    title_lines: Vec<Line<'static>>,
    options: Vec<Line<'static>>,
    cursor_idx: usize,
    other_line: Line<'static>,
    free_text: Option<Line<'static>>,
    hint: Line<'static>,
) -> (Vec<Line<'static>>, Option<usize>) {
    let mut out: Vec<Line<'static>> = Vec::with_capacity(cap);
    let title_n = title_height.min(cap);
    out.extend(title_lines.into_iter().take(title_n));
    let mut used = title_n;
    if used >= cap {
        return (out, None);
    }
    let want_hint = cap > used;
    let want_free = free_text.is_some() && cap > used + want_hint as usize;
    let want_other = cap > used + want_hint as usize + want_free as usize;
    let opt_budget =
        cap.saturating_sub(used + want_hint as usize + want_free as usize + want_other as usize);

    let opt_count = options.len();
    let (start, end) = window_around(cursor_idx, opt_count, opt_budget);
    for line in options.into_iter().skip(start).take(end - start) {
        out.push(line);
    }
    used += end - start;

    let mut free_text_row = None;
    if want_other {
        out.push(other_line);
        used += 1;
        if want_free && let Some(ft) = free_text {
            free_text_row = Some(used);
            out.push(ft);
        }
    }
    if want_hint {
        out.push(hint);
    }
    (out, free_text_row)
}

fn window_around(cursor: usize, total: usize, budget: usize) -> (usize, usize) {
    if total == 0 || budget == 0 {
        return (0, 0);
    }
    if total <= budget {
        return (0, total);
    }
    let cursor = cursor.min(total - 1);
    let half = budget / 2;
    let start = cursor.saturating_sub(half);
    let end = (start + budget).min(total);
    let start = end.saturating_sub(budget);
    (start, end)
}

/// Table rendering for markdown — collect cells during parse, emit
/// formatted table on `TagEnd::Table`. Long cell content wraps within columns.
use pulldown_cmark::Alignment;
use ratatui::prelude::*;
use unicode_width::UnicodeWidthStr;

use super::writer::MdWriter;

impl MdWriter {
    pub(super) fn start_table(&mut self, alignments: Vec<Alignment>) {
        self.flush_pending();
        self.in_table = true;
        self.table_alignments = alignments;
        self.table_rows.clear();
    }

    pub(super) fn end_table(&mut self) {
        self.in_table = false;
        let rows = std::mem::take(&mut self.table_rows);
        let alignments = std::mem::take(&mut self.table_alignments);
        let border = self.styles.table_border;
        let header = self.styles.table_header;
        self.lines
            .extend(render_table(&rows, &alignments, self.width, border, header));
        self.lines.push(Line::from(""));
    }

    pub(super) fn start_table_head(&mut self) {
        self.in_table_header = true;
        self.current_row.clear();
    }

    pub(super) fn end_table_head(&mut self) {
        self.in_table_header = false;
        self.table_rows.push(std::mem::take(&mut self.current_row));
    }

    pub(super) fn start_table_row(&mut self) {
        self.current_row.clear();
    }

    pub(super) fn end_table_row(&mut self) {
        self.table_rows.push(std::mem::take(&mut self.current_row));
    }

    pub(super) fn start_table_cell(&mut self) {
        self.current_cell.clear();
    }

    pub(super) fn end_table_cell(&mut self) {
        let cell = std::mem::take(&mut self.current_cell);
        self.current_row.push(cell.trim().to_string());
    }
}

// ---------- rendering ----------

fn render_table(
    rows: &[Vec<String>],
    alignments: &[Alignment],
    width: u16,
    border: Style,
    header: Style,
) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return Vec::new();
    }
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return Vec::new();
    }
    let col_widths = compute_col_widths(rows, num_cols, width);
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(border_line(&col_widths, "┌", "┬", "┐", border));
    for (i, row) in rows.iter().enumerate() {
        let cell_style = if i == 0 { header } else { Style::default() };
        lines.extend(format_row(row, &col_widths, alignments, border, cell_style));
        if i == 0 {
            lines.push(border_line(&col_widths, "├", "┼", "┤", border));
        }
    }
    lines.push(border_line(&col_widths, "└", "┴", "┘", border));
    lines
}

/// Compute per-column widths, shrinking proportionally if total exceeds budget.
fn compute_col_widths(rows: &[Vec<String>], num_cols: usize, width: u16) -> Vec<usize> {
    let mut widths: Vec<usize> = vec![3; num_cols];
    for row in rows {
        for (j, cell) in row.iter().enumerate() {
            widths[j] = widths[j].max(UnicodeWidthStr::width(cell.as_str())).max(3);
        }
    }
    let overhead = (num_cols - 1) * 3 + 4; // " │ " between cols + "│ " left + " │" right
    let total: usize = widths.iter().sum::<usize>() + overhead;
    let budget = (width as usize).max(num_cols + overhead);
    if total > budget {
        let content_budget = budget.saturating_sub(overhead).max(num_cols);
        let content_total: usize = widths.iter().sum();
        for w in &mut widths {
            *w = (*w * content_budget / content_total).max(1);
        }
    }
    widths
}

/// Format one table row — cells wrap within their column width,
/// producing multiple visual lines when any cell content overflows.
fn format_row(
    row: &[String],
    col_widths: &[usize],
    alignments: &[Alignment],
    border: Style,
    cell_style: Style,
) -> Vec<Line<'static>> {
    let wrapped: Vec<Vec<String>> = col_widths
        .iter()
        .enumerate()
        .map(|(j, cw)| {
            let text = row.get(j).map(|s| s.as_str()).unwrap_or("");
            wrap_cell(text, *cw)
        })
        .collect();
    let height = wrapped.iter().map(|c| c.len()).max().unwrap_or(1);

    (0..height)
        .map(|li| {
            let mut spans = vec![Span::styled("│ ", border)];
            for (j, cw) in col_widths.iter().enumerate() {
                if j > 0 {
                    spans.push(Span::styled(" │ ", border));
                }
                let text = wrapped[j].get(li).map(|s| s.as_str()).unwrap_or("");
                let align = alignments.get(j).copied().unwrap_or(Alignment::None);
                spans.push(Span::styled(align_cell(text, *cw, align), cell_style));
            }
            spans.push(Span::styled(" │", border));
            Line::from(spans)
        })
        .collect()
}

/// Wrap cell text to fit within `width` display columns.
fn wrap_cell(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() || width == 0 {
        return vec![String::new()];
    }
    let v: Vec<String> = textwrap::wrap(text, width)
        .into_iter()
        .map(|c| c.into_owned())
        .collect();
    if v.is_empty() { vec![String::new()] } else { v }
}

fn border_line(
    col_widths: &[usize],
    left: &'static str,
    mid: &'static str,
    right: &'static str,
    style: Style,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (j, cw) in col_widths.iter().enumerate() {
        if j == 0 {
            spans.push(Span::styled(left, style));
        } else {
            spans.push(Span::styled(mid, style));
        }
        spans.push(Span::styled("─".repeat(cw + 2), style));
    }
    spans.push(Span::styled(right, style));
    Line::from(spans)
}

/// Pad/truncate `text` into a cell of width `w` respecting alignment.
fn align_cell(text: &str, w: usize, align: Alignment) -> String {
    let tw = UnicodeWidthStr::width(text);
    if tw >= w {
        return crate::text_util::truncate_to_width(text, w);
    }
    let pad = w - tw;
    match align {
        Alignment::Right => format!("{}{}", " ".repeat(pad), text),
        Alignment::Center => {
            let l = pad / 2;
            format!("{}{}{}", " ".repeat(l), text, " ".repeat(pad - l))
        }
        _ => format!("{}{}", text, " ".repeat(pad)),
    }
}

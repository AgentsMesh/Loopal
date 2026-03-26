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
        self.lines.extend(render_table(&rows, &alignments, self.width));
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

fn render_table(rows: &[Vec<String>], alignments: &[Alignment], width: u16) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return Vec::new();
    }
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return Vec::new();
    }
    let col_widths = compute_col_widths(rows, num_cols, width);
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        lines.extend(format_row(row, &col_widths, alignments, i == 0));
        if i == 0 {
            lines.push(separator_line(&col_widths));
        }
    }
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
    let overhead = if num_cols > 1 { (num_cols - 1) * 3 } else { 0 };
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
    bold: bool,
) -> Vec<Line<'static>> {
    let dim = Style::default().fg(Color::DarkGray);
    let cell_style = if bold {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
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
            let spans: Vec<Span<'static>> = col_widths
                .iter()
                .enumerate()
                .flat_map(|(j, cw)| {
                    let sep = (j > 0).then(|| Span::styled(" │ ", dim));
                    let text = wrapped[j].get(li).map(|s| s.as_str()).unwrap_or("");
                    let align = alignments.get(j).copied().unwrap_or(Alignment::None);
                    let padded = align_cell(text, *cw, align);
                    sep.into_iter()
                        .chain(std::iter::once(Span::styled(padded, cell_style)))
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

/// Wrap cell text to fit within `width` display columns.
fn wrap_cell(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() || width == 0 {
        return vec![String::new()];
    }
    let v: Vec<String> = textwrap::wrap(text, width).into_iter().map(|c| c.into_owned()).collect();
    if v.is_empty() { vec![String::new()] } else { v }
}

fn separator_line(col_widths: &[usize]) -> Line<'static> {
    let dim = Style::default().fg(Color::DarkGray);
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (j, cw) in col_widths.iter().enumerate() {
        if j > 0 {
            spans.push(Span::styled("─┼─", dim));
        }
        spans.push(Span::styled("─".repeat(*cw), dim));
    }
    Line::from(spans)
}

/// Pad/truncate `text` into a cell of width `w` respecting alignment.
fn align_cell(text: &str, w: usize, align: Alignment) -> String {
    let tw = UnicodeWidthStr::width(text);
    if tw >= w {
        return truncate_to_width(text, w);
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

/// Truncate to at most `w` display columns (safety net for unbreakable words).
fn truncate_to_width(text: &str, w: usize) -> String {
    let mut buf = String::new();
    let mut col = 0;
    for ch in text.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if col + cw > w {
            break;
        }
        buf.push(ch);
        col += cw;
    }
    buf
}

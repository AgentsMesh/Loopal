/// Table rendering for markdown — collect cells during parse, emit
/// formatted table on `TagEnd::Table`.
///
/// Layout: pipe-separated columns with Unicode box-drawing separator.
/// ```text
/// Header A  │ Header B
/// ──────────┼─────────
/// cell 1    │ cell 2
/// ```
use pulldown_cmark::Alignment;
use ratatui::prelude::*;
use unicode_width::UnicodeWidthStr;

use super::writer::MdWriter;

impl MdWriter {
    // --- Tag handlers called from writer_blocks.rs ---

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
        let lines = render_table(&rows, &alignments, self.width);
        self.lines.extend(lines);
        self.lines.push(Line::from(""));
    }

    pub(super) fn start_table_head(&mut self) {
        self.in_table_header = true;
        self.current_row.clear();
    }

    pub(super) fn end_table_head(&mut self) {
        self.in_table_header = false;
        let row = std::mem::take(&mut self.current_row);
        self.table_rows.push(row);
    }

    pub(super) fn start_table_row(&mut self) {
        self.current_row.clear();
    }

    pub(super) fn end_table_row(&mut self) {
        let row = std::mem::take(&mut self.current_row);
        self.table_rows.push(row);
    }

    pub(super) fn start_table_cell(&mut self) {
        self.current_cell.clear();
    }

    pub(super) fn end_table_cell(&mut self) {
        let cell = std::mem::take(&mut self.current_cell);
        self.current_row.push(cell.trim().to_string());
    }
}

// ---------- free functions ----------

/// Render collected table rows into styled `Line`s.
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
        let is_header = i == 0;
        lines.push(format_row(row, &col_widths, alignments, is_header));
        if is_header {
            lines.push(separator_line(&col_widths));
        }
    }
    lines
}

/// Compute per-column widths, shrinking proportionally if needed.
fn compute_col_widths(rows: &[Vec<String>], num_cols: usize, width: u16) -> Vec<usize> {
    let mut widths: Vec<usize> = vec![3; num_cols];
    for row in rows {
        for (j, cell) in row.iter().enumerate() {
            let w = UnicodeWidthStr::width(cell.as_str());
            widths[j] = widths[j].max(w).max(3);
        }
    }

    // Overhead: " │ " between cols (3 chars × (n-1)) + no outer border
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

/// Format one table row as a styled `Line`.
fn format_row(
    row: &[String],
    col_widths: &[usize],
    alignments: &[Alignment],
    bold: bool,
) -> Line<'static> {
    let dim = Style::default().fg(Color::DarkGray);
    let cell_style = if bold {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    for (j, cw) in col_widths.iter().enumerate() {
        if j > 0 {
            spans.push(Span::styled(" │ ", dim));
        }
        let text = row.get(j).map(|s| s.as_str()).unwrap_or("");
        let padded = align_cell(text, *cw, alignment_at(alignments, j));
        spans.push(Span::styled(padded, cell_style));
    }
    Line::from(spans)
}

/// Build the separator line between header and body.
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

/// Pad `text` into a cell of width `w` respecting alignment.
fn align_cell(text: &str, w: usize, align: Alignment) -> String {
    let tw = UnicodeWidthStr::width(text);
    if tw >= w {
        return truncate_to_width(text, w);
    }
    let pad = w - tw;
    match align {
        Alignment::Right => format!("{}{}", " ".repeat(pad), text),
        Alignment::Center => {
            let left = pad / 2;
            let right = pad - left;
            format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
        }
        _ => format!("{}{}", text, " ".repeat(pad)),
    }
}

/// Truncate `text` to at most `w` display columns.
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

fn alignment_at(alignments: &[Alignment], idx: usize) -> Alignment {
    alignments.get(idx).copied().unwrap_or(Alignment::None)
}

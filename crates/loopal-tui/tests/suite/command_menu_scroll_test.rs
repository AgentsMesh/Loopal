use loopal_tui::app::AutocompleteState;
use loopal_tui::command::CommandEntry;
use loopal_tui::views::command_menu::{render_command_menu, scroll_offset_for_selection};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

fn entry(name: &str) -> CommandEntry {
    CommandEntry {
        name: name.to_string(),
        description: format!("desc for {name}"),
        has_arg: false,
        is_skill: false,
    }
}

fn synthetic_matches(count: usize) -> Vec<CommandEntry> {
    (0..count).map(|i| entry(&format!("/cmd{i:02}"))).collect()
}

fn buffer_contains(terminal: &Terminal<TestBackend>, needle: &str) -> bool {
    let buf = terminal.backend().buffer();
    for y in 0..buf.area.height {
        let mut row = String::new();
        for x in 0..buf.area.width {
            row.push_str(buf.cell((x, y)).unwrap().symbol());
        }
        if row.contains(needle) {
            return true;
        }
    }
    false
}

fn render_menu(matches: Vec<CommandEntry>, selected: usize) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    let ac = AutocompleteState { matches, selected };
    let input_area = Rect::new(0, 18, 60, 1);
    terminal
        .draw(|f| render_command_menu(f, &ac, input_area))
        .unwrap();
    terminal
}

#[test]
fn selected_within_first_page_keeps_zero_offset() {
    for i in 0..8 {
        assert_eq!(scroll_offset_for_selection(i, 8), 0, "selected={i}");
    }
}

#[test]
fn selected_at_first_overflow_shifts_window_by_one() {
    assert_eq!(scroll_offset_for_selection(8, 8), 1);
}

#[test]
fn selected_far_beyond_visible_keeps_it_at_last_row() {
    let visible = 8;
    let selected = 14;
    let offset = scroll_offset_for_selection(selected, visible);
    assert_eq!(offset, selected + 1 - visible);
    assert_eq!(selected - offset, visible - 1);
}

#[test]
fn zero_visible_returns_zero_offset() {
    assert_eq!(scroll_offset_for_selection(0, 0), 0);
    assert_eq!(scroll_offset_for_selection(99, 0), 0);
}

#[test]
fn render_overflow_keeps_selected_visible() {
    let terminal = render_menu(synthetic_matches(15), 14);
    assert!(
        buffer_contains(&terminal, "/cmd14"),
        "selected last item must appear in buffer"
    );
    assert!(
        !buffer_contains(&terminal, "/cmd00"),
        "first item must scroll out when selected is far beyond visible"
    );
}

#[test]
fn render_first_page_shows_top_items() {
    let terminal = render_menu(synthetic_matches(15), 0);
    assert!(buffer_contains(&terminal, "/cmd00"));
    assert!(buffer_contains(&terminal, "/cmd07"));
    assert!(
        !buffer_contains(&terminal, "/cmd14"),
        "items beyond MAX_MENU_ITEMS must not appear when selected is on first page"
    );
}

#[test]
fn render_mid_overflow_shows_window_containing_selected() {
    let terminal = render_menu(synthetic_matches(15), 9);
    assert!(buffer_contains(&terminal, "/cmd09"));
    assert!(
        !buffer_contains(&terminal, "/cmd00"),
        "first item must be scrolled out"
    );
    assert!(
        !buffer_contains(&terminal, "/cmd14"),
        "last item must not be in window yet"
    );
}

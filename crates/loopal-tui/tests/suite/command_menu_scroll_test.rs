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
    let names: Vec<&str> = vec![
        "/plan", "/act", "/clear", "/compact", "/model", "/rewind", "/status", "/mcp", "/resume",
        "/init", "/help", "/exit", "/agents", "/topology", "/skills",
    ];
    let matches: Vec<CommandEntry> = names.iter().map(|n| entry(n)).collect();

    let terminal = render_menu(matches.clone(), 14);
    assert!(
        buffer_contains(&terminal, "/skills"),
        "selected last item must appear in buffer"
    );
    assert!(
        !buffer_contains(&terminal, "/plan"),
        "first item must scroll out when selected is far beyond visible"
    );
}

#[test]
fn render_first_page_shows_top_items() {
    let names: Vec<&str> = vec![
        "/plan", "/act", "/clear", "/compact", "/model", "/rewind", "/status", "/mcp", "/resume",
        "/init", "/help", "/exit", "/agents", "/topology", "/skills",
    ];
    let matches: Vec<CommandEntry> = names.iter().map(|n| entry(n)).collect();

    let terminal = render_menu(matches, 0);
    assert!(buffer_contains(&terminal, "/plan"));
    assert!(buffer_contains(&terminal, "/mcp"));
    assert!(
        !buffer_contains(&terminal, "/skills"),
        "items beyond MAX_MENU_ITEMS must not appear when selected is on first page"
    );
}

#[test]
fn render_mid_overflow_shows_window_containing_selected() {
    let names: Vec<&str> = vec![
        "/plan", "/act", "/clear", "/compact", "/model", "/rewind", "/status", "/mcp", "/resume",
        "/init", "/help", "/exit", "/agents", "/topology", "/skills",
    ];
    let matches: Vec<CommandEntry> = names.iter().map(|n| entry(n)).collect();

    let terminal = render_menu(matches, 9);
    assert!(buffer_contains(&terminal, "/init"));
    assert!(
        !buffer_contains(&terminal, "/plan"),
        "first item must be scrolled out"
    );
    assert!(
        !buffer_contains(&terminal, "/skills"),
        "last item must not be in window yet"
    );
}

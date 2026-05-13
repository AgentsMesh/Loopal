use std::time::Duration;

use ratatui::prelude::*;

pub(crate) fn output_style() -> Style {
    Style::default().fg(Color::Rgb(155, 160, 170))
}

pub(crate) fn dim_style() -> Style {
    Style::default().fg(Color::Rgb(100, 105, 115))
}

pub(crate) fn expand_output(content: &str, max_lines: usize, style: Style) -> Vec<Line<'static>> {
    let all: Vec<&str> = content.lines().collect();
    let total = all.len();
    let mut lines = Vec::new();

    for (i, text) in all.iter().take(max_lines).enumerate() {
        let prefix = if i == 0 { "  ⎿ " } else { "    " };
        lines.push(Line::from(Span::styled(format!("{prefix}{text}"), style)));
    }

    if total > max_lines {
        lines.push(Line::from(Span::styled(
            format!("    … +{} lines", total - max_lines),
            dim_style(),
        )));
    }
    lines
}

pub(crate) fn output_first_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(format!("  ⎿ {text}"), output_style()))
}

pub(crate) fn format_duration_short(d: Duration) -> String {
    let total_ms = d.as_millis();
    if total_ms < 1000 {
        format!("{total_ms}ms")
    } else if total_ms < 60_000 {
        format!("{:.1}s", d.as_secs_f64())
    } else {
        let mins = d.as_secs() / 60;
        let secs = d.as_secs() % 60;
        format!("{mins}m {secs}s")
    }
}

pub(crate) fn completion_line(label: &str, duration: Duration) -> Line<'static> {
    Line::from(Span::styled(
        format!("  ⎿ {label} in {}", format_duration_short(duration)),
        dim_style(),
    ))
}

pub(crate) fn stale_line(reason: &str, duration: Duration) -> Line<'static> {
    let body = if duration.is_zero() {
        format!("  ⎿ Stale ({reason})")
    } else {
        format!(
            "  ⎿ Stale ({reason} after {})",
            format_duration_short(duration)
        )
    };
    Line::from(Span::styled(
        body,
        Style::default().fg(Color::Rgb(200, 170, 80)),
    ))
}

pub(crate) fn cancelled_line(cause: &str, duration: Duration) -> Line<'static> {
    let body = if duration.is_zero() {
        format!("  ⎿ Cancelled ({cause})")
    } else {
        format!(
            "  ⎿ Cancelled ({cause}) after {}",
            format_duration_short(duration)
        )
    };
    Line::from(Span::styled(body, dim_style()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_under_one_second_renders_milliseconds() {
        assert_eq!(format_duration_short(Duration::from_millis(0)), "0ms");
        assert_eq!(format_duration_short(Duration::from_millis(1)), "1ms");
        assert_eq!(format_duration_short(Duration::from_millis(999)), "999ms");
    }

    #[test]
    fn duration_under_one_minute_renders_seconds() {
        assert_eq!(format_duration_short(Duration::from_millis(1000)), "1.0s");
        assert_eq!(format_duration_short(Duration::from_millis(1500)), "1.5s");
        assert_eq!(format_duration_short(Duration::from_secs(59)), "59.0s");
    }

    #[test]
    fn duration_over_one_minute_renders_minutes_and_seconds() {
        assert_eq!(format_duration_short(Duration::from_secs(60)), "1m 0s");
        assert_eq!(format_duration_short(Duration::from_secs(61)), "1m 1s");
        assert_eq!(format_duration_short(Duration::from_secs(120)), "2m 0s");
        assert_eq!(format_duration_short(Duration::from_secs(3661)), "61m 1s");
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn completion_line_includes_label_and_duration() {
        let l = completion_line("Done", Duration::from_millis(500));
        let text = line_text(&l);
        assert!(text.contains("Done"));
        assert!(text.contains("500ms"));
        assert!(text.contains("⎿"));
    }

    #[test]
    fn stale_line_includes_reason_and_duration() {
        let l = stale_line("WatchdogTimeout", Duration::from_secs(120));
        let text = line_text(&l);
        assert!(text.contains("Stale"));
        assert!(text.contains("WatchdogTimeout"));
        assert!(text.contains("2m 0s"));
    }

    #[test]
    fn cancelled_line_includes_cause_and_duration() {
        let l = cancelled_line("UserInterrupt", Duration::from_millis(1234));
        let text = line_text(&l);
        assert!(text.contains("Cancelled"));
        assert!(text.contains("UserInterrupt"));
        assert!(text.contains("1.2s"));
    }

    #[test]
    fn stale_line_omits_duration_when_zero() {
        let l = stale_line("connection lost", Duration::ZERO);
        let text = line_text(&l);
        assert!(text.contains("Stale"));
        assert!(text.contains("connection lost"));
        assert!(!text.contains("after"));
        assert!(!text.contains("0ms"));
    }

    #[test]
    fn cancelled_line_omits_duration_when_zero() {
        let l = cancelled_line("parent cancelled", Duration::ZERO);
        let text = line_text(&l);
        assert!(text.contains("Cancelled"));
        assert!(text.contains("parent cancelled"));
        assert!(!text.contains("after"));
    }
}

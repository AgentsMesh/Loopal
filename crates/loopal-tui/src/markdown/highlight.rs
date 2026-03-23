/// Syntax highlighting via syntect — wraps SyntaxSet + Theme as global singletons.
///
/// Converts syntect styles to ratatui Styles (foreground only, skip background
/// to preserve terminal theme). Falls back to plain text for unsupported
/// languages or oversized inputs.
use std::sync::OnceLock;

use ratatui::prelude::*;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style as SyntectStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// Safety limits — fall back to plain text above these thresholds.
const MAX_HIGHLIGHT_BYTES: usize = 512 * 1024;
const MAX_HIGHLIGHT_LINES: usize = 10_000;

/// ANSI alpha-channel encoding used by bat-compatible themes.
const ANSI_ALPHA_INDEX: u8 = 0x00;
const ANSI_ALPHA_DEFAULT: u8 = 0x01;
const OPAQUE_ALPHA: u8 = 0xFF;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Theme> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(two_face::syntax::extra_newlines)
}

fn theme() -> &'static Theme {
    THEME.get_or_init(|| {
        let ts = ThemeSet::from(two_face::theme::extra());
        ts.themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_else(|| ts.themes.values().next().cloned().unwrap_or_default())
    })
}

/// Highlight `code` using the syntax identified by `lang`.
///
/// Returns one `Line` per source line. Falls back to unstyled text when
/// the language is unknown or the input exceeds safety limits.
pub fn highlight_code_to_lines(code: &str, lang: &str) -> Vec<Line<'static>> {
    if let Some(spans) = highlight_inner(code, lang) {
        spans.into_iter().map(Line::from).collect()
    } else {
        plain_lines(code)
    }
}

fn highlight_inner(code: &str, lang: &str) -> Option<Vec<Vec<Span<'static>>>> {
    if code.is_empty()
        || code.len() > MAX_HIGHLIGHT_BYTES
        || code.lines().count() > MAX_HIGHLIGHT_LINES
    {
        return None;
    }
    let syntax = find_syntax(lang)?;
    let mut h = HighlightLines::new(syntax, theme());
    let ss = syntax_set();
    let mut result: Vec<Vec<Span<'static>>> = Vec::new();

    for line in LinesWithEndings::from(code) {
        let ranges = h.highlight_line(line, ss).ok()?;
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (style, text) in ranges {
            let text = text.trim_end_matches(['\n', '\r']);
            if text.is_empty() {
                continue;
            }
            spans.push(Span::styled(text.to_string(), convert_style(style)));
        }
        if spans.is_empty() {
            spans.push(Span::raw(String::new()));
        }
        result.push(spans);
    }
    Some(result)
}

fn find_syntax(lang: &str) -> Option<&'static SyntaxReference> {
    let ss = syntax_set();
    let patched = match lang {
        "csharp" | "c-sharp" => "c#",
        "golang" => "go",
        "python3" => "python",
        "shell" => "bash",
        _ => lang,
    };
    ss.find_syntax_by_token(patched)
        .or_else(|| ss.find_syntax_by_name(patched))
        .or_else(|| {
            let lower = patched.to_ascii_lowercase();
            ss.syntaxes()
                .iter()
                .find(|s| s.name.to_ascii_lowercase() == lower)
        })
        .or_else(|| ss.find_syntax_by_extension(lang))
}

/// Convert syntect style to ratatui — foreground only, skip background.
fn convert_style(syn: SyntectStyle) -> Style {
    let mut rt = Style::default();
    if let Some(fg) = convert_color(syn.foreground) {
        rt = rt.fg(fg);
    }
    if syn.font_style.contains(FontStyle::BOLD) {
        rt = rt.add_modifier(Modifier::BOLD);
    }
    rt
}

fn convert_color(c: syntect::highlighting::Color) -> Option<Color> {
    match c.a {
        ANSI_ALPHA_INDEX => Some(ansi_color(c.r)),
        ANSI_ALPHA_DEFAULT => None,
        OPAQUE_ALPHA => Some(Color::Rgb(c.r, c.g, c.b)),
        _ => Some(Color::Rgb(c.r, c.g, c.b)),
    }
}

fn ansi_color(idx: u8) -> Color {
    match idx {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::White,
        _ => Color::White,
    }
}

fn plain_lines(code: &str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = code.lines().map(|l| Line::from(l.to_string())).collect();
    if lines.is_empty() {
        lines.push(Line::from(String::new()));
    }
    lines
}

use super::theme::Theme;
use crate::app::App;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

pub fn render_statusbar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let cwd = crate::agent::workspace::workspace_root();
    let cwd_str = cwd
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| cwd.to_string_lossy().into_owned());

    let sid = if app.session_id.len() > 8 {
        &app.session_id[app.session_id.len() - 8..]
    } else {
        &app.session_id
    };

    let session_tokens = app.session_token_usage.total();
    let total_tokens = app.token_usage.total();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left = vec![
        Span::styled(
            format!(" {} ", cwd_str),
            Style::default()
                .fg(theme.header_bg)
                .bg(theme.border)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(format!("sess {sid}"), Style::default().fg(theme.border)),
        Span::raw("  "),
        Span::styled("[/]", theme.bold(theme.accent)),
        Span::raw("  "),
        Span::styled("[Alt+S]", theme.bold(theme.accent)),
        Span::raw(" sessions  "),
        Span::styled("[Esc]", theme.bold(theme.accent)),
        Span::raw(" cancel  "),
        Span::styled("[Alt+Q]", theme.bold(theme.accent)),
        Span::raw(" quit"),
    ];

    let right = vec![
        Span::styled(
            format!("Session: {} tok ", format_tokens(session_tokens)),
            Style::default().fg(theme.success),
        ),
        Span::raw("  "),
        Span::styled(
            format!("Total: {} tok ", format_tokens(total_tokens)),
            Style::default().fg(theme.accent),
        ),
    ];

    let left_line = Line::from(left);
    let right_line = Line::from(right).alignment(Alignment::Right);

    let left_para = Paragraph::new(left_line).block(Block::default());
    let right_para = Paragraph::new(right_line).block(Block::default());

    frame.render_widget(left_para, chunks[0]);
    frame.render_widget(right_para, chunks[1]);
}

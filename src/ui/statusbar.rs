use super::theme::Theme;
use crate::app::App;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

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

    let shortcuts = vec![
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

    let line = Line::from(shortcuts);
    let paragraph = Paragraph::new(line).block(Block::default());
    frame.render_widget(paragraph, area);
}

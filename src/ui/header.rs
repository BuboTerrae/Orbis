use super::theme::Theme;
use crate::app::App;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render_header(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let key_source = if app.config.is_key_from_env(&app.config.active_provider) {
        "ENV"
    } else if app
        .config
        .get_api_key(&app.config.active_provider)
        .is_some()
    {
        "CFG"
    } else {
        "MISSING KEY"
    };

    let status_span = if app.is_streaming {
        Span::styled(
            "● Generating...",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )
    } else if let Some(ref err) = app.status_message {
        Span::styled(format!("⚠ {}", err), Style::default().fg(theme.error))
    } else {
        Span::styled("● Ready", Style::default().fg(theme.success))
    };

    let mode_badge = match app.mode {
        crate::app::AgentMode::Agent => Span::styled(
            " [AGENT] ",
            Style::default()
                .fg(theme.header_bg)
                .bg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        crate::app::AgentMode::DirectChat => Span::styled(
            " [CHAT] ",
            Style::default()
                .fg(theme.header_bg)
                .bg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let security_badge = match app.permission_mode {
        crate::app::PermissionMode::Ask => Span::styled(
            " [Ask] ",
            Style::default()
                .fg(theme.header_bg)
                .bg(theme.user_badge)
                .add_modifier(Modifier::BOLD),
        ),
        crate::app::PermissionMode::AutoApprove => Span::styled(
            " [Auto-Approve] ",
            Style::default()
                .fg(theme.header_bg)
                .bg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        crate::app::PermissionMode::ReadOnly => Span::styled(
            " [Read-Only] ",
            Style::default()
                .fg(theme.header_bg)
                .bg(theme.error)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let title_line = Line::from(vec![
        Span::styled(
            " ORBIS ",
            Style::default()
                .fg(theme.header_bg)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        mode_badge,
        Span::raw(" "),
        security_badge,
        Span::raw("  "),
        Span::styled(
            format!("Provider: {}", app.config.active_provider),
            theme.bold(theme.fg),
        ),
        Span::styled(
            format!(" [{}]", key_source),
            Style::default().fg(if key_source == "MISSING KEY" {
                theme.error
            } else {
                theme.success
            }),
        ),
        Span::raw("  │  "),
        Span::styled(
            format!("Model: {}", app.config.active_model),
            theme.bold(theme.accent),
        ),
        Span::raw("  │  "),
        status_span,
    ]);

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.border));

    let paragraph = Paragraph::new(title_line).block(block);
    frame.render_widget(paragraph, area);
}

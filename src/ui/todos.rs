use super::theme::Theme;
use crate::agent::tools::todo_ops::TodoStatus;
use crate::app::App;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn todo_panel_height(app: &App) -> u16 {
    if app.todos.is_empty() {
        0
    } else {
        (app.todos.len() as u16 + 1).clamp(2, 6)
    }
}

pub fn render_todos(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    if app.todos.is_empty() {
        return;
    }
    let block = Block::default()
        .title(" Plan ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));

    let mut lines = Vec::new();
    for item in &app.todos {
        let (mark, color) = match item.status {
            TodoStatus::Completed => ("✓", theme.success),
            TodoStatus::InProgress => ("▶", theme.warning),
            TodoStatus::Cancelled => ("–", theme.border),
            TodoStatus::Pending => ("○", theme.fg),
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {mark} "),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(item.content.clone(), Style::default().fg(color)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

use super::theme::Theme;
use crate::app::App;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render_input(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let is_focused = app.focused_panel == crate::app::FocusedPanel::Input;

    let title = if app.is_streaming {
        " Prompt  ·  generating  ·  Esc cancel ".to_string()
    } else if app.input_buffer.starts_with('/') {
        " Slash  /provider /model /keys /sessions /new /compact /endpoint /status /version /debug /clear /help ".to_string()
    } else {
        " Prompt  ·  Enter send  ·  Alt+Enter newline ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.input_buffer.starts_with('/') {
            theme.accent
        } else if is_focused {
            theme.border_focused
        } else {
            theme.border
        }));

    let mut content = app.input_buffer.clone();
    if is_focused && !app.is_streaming {
        content.push('█'); // visual cursor
    }

    let input_lines: Vec<Line> = if content.is_empty() && !is_focused {
        vec![Line::from(vec![Span::styled(
            "Type your coding prompt here...",
            Style::default().fg(theme.border),
        )])]
    } else {
        content.lines().map(Line::from).collect()
    };

    let paragraph = Paragraph::new(input_lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

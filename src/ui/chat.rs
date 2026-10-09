use super::theme::Theme;
use crate::app::App;
use crate::provider::Role;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

fn wrap_preview(text: &str, max_lines: usize) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        lines
    } else {
        let mut out = lines[..max_lines].to_vec();
        out.push("…");
        out
    }
}

pub fn render_chat(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .title(" Conversation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if app.focused_panel == crate::app::FocusedPanel::Chat {
                theme.border_focused
            } else {
                theme.border
            },
        ));
    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        match msg.role {
            Role::User => {
                lines.push(Line::from(vec![Span::styled(
                    " YOU ",
                    Style::default()
                        .fg(theme.header_bg)
                        .bg(theme.user_badge)
                        .add_modifier(Modifier::BOLD),
                )]));
                lines.extend(styled_markdown_lines(&msg.content, theme));
                lines.push(Line::raw(""));
            }
            Role::Assistant => {
                lines.push(Line::from(vec![Span::styled(
                    " AGENT ",
                    Style::default()
                        .fg(theme.header_bg)
                        .bg(theme.assistant_badge)
                        .add_modifier(Modifier::BOLD),
                )]));
                lines.extend(styled_markdown_lines(&msg.content, theme));
                if let Some(ref tool_calls) = msg.tool_calls {
                    for tc in tool_calls {
                        let args = compact_json(&tc.arguments);
                        lines.push(Line::from(vec![
                            Span::raw("  └ "),
                            Span::styled(
                                format!("{} ", tc.name),
                                Style::default()
                                    .fg(theme.warning)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(args, Style::default().fg(theme.border)),
                        ]));
                    }
                }
                lines.push(Line::raw(""));
            }
            Role::Tool => {
                let tool_name = msg.name.as_deref().unwrap_or("Tool");
                lines.push(Line::from(vec![Span::styled(
                    format!(" {} ", tool_name),
                    Style::default()
                        .fg(theme.header_bg)
                        .bg(theme.tool_badge)
                        .add_modifier(Modifier::BOLD),
                )]));
                for l in wrap_preview(&msg.content, 24) {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(l.to_string(), Style::default().fg(theme.border_focused)),
                    ]));
                }
                lines.push(Line::raw(""));
            }
            Role::System => {}
        }
    }

    if app.is_streaming {
        lines.push(Line::from(vec![Span::styled(
            " AGENT · thinking ",
            Style::default()
                .fg(theme.header_bg)
                .bg(theme.assistant_badge)
                .add_modifier(Modifier::BOLD),
        )]));
        if app.streaming_buffer.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("…", Style::default().fg(theme.border)),
            ]));
        } else {
            for l in styled_markdown_lines(&app.streaming_buffer, theme) {
                lines.push(l);
            }
        }
        for tc in &app.pending_tool_calls {
            lines.push(Line::from(vec![
                Span::raw("  └ "),
                Span::styled(format!("{} ", tc.name), Style::default().fg(theme.warning)),
                Span::styled(
                    compact_json(&tc.arguments),
                    Style::default().fg(theme.border),
                ),
            ]));
        }
        lines.push(Line::raw(""));
    }

    let inner_h = area.height.saturating_sub(2) as usize;
    let scroll = if app.follow_chat {
        lines.len().saturating_sub(inner_h.max(1))
    } else {
        app.chat_scroll
    } as u16;

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    frame.render_widget(paragraph, area);
}

fn styled_markdown_lines<'a>(text: &str, theme: &'a Theme) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let mut in_fence = false;
    for raw in text.lines() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    raw.to_string(),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            continue;
        }
        let color = if in_fence {
            theme.border_focused
        } else if trimmed.starts_with('#') {
            theme.accent
        } else {
            theme.fg
        };
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(raw.to_string(), Style::default().fg(color)),
        ]));
    }
    lines
}

fn compact_json(value: &serde_json::Value) -> String {
    let s = value.to_string();
    if s.len() <= 80 {
        s
    } else {
        format!("{}…", &s[..80])
    }
}

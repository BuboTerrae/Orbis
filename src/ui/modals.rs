use super::theme::Theme;
use crate::app::{ActiveModal, App};
use crate::provider::config::ProviderType;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn render_modals(frame: &mut Frame, app: &App, theme: &Theme) {
    match &app.active_modal {
        ActiveModal::None => {}
        ActiveModal::ProviderPicker { selected_index } => {
            let area = centered_rect(60, 50, frame.area());
            frame.render_widget(Clear, area);

            let block = Block::default()
                .title(" Select LLM Provider & Model (↑/↓ to navigate, Enter to select, Esc to close) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_focused));

            let providers = ProviderType::all();
            let items: Vec<ListItem> = providers
                .iter()
                .enumerate()
                .map(|(idx, p)| {
                    let is_active = p == &app.config.active_provider;
                    let is_selected = idx == *selected_index;

                    let prefix = if is_selected { " ▶ " } else { "   " };
                    let active_tag = if is_active { " (Active)" } else { "" };
                    let key_info = if app.config.get_api_key(p).is_some() {
                        " [Key Present]"
                    } else {
                        " [Missing Key]"
                    };

                    let line = Line::from(vec![
                        Span::styled(
                            format!("{}{}{}", prefix, p, active_tag),
                            if is_selected {
                                theme.bold(theme.accent)
                            } else {
                                theme.text()
                            },
                        ),
                        Span::styled(
                            key_info,
                            Style::default().fg(if key_info.contains("Missing") {
                                theme.error
                            } else {
                                theme.success
                            }),
                        ),
                        Span::styled(
                            format!(" - Default: {}", p.default_model()),
                            Style::default().fg(theme.border),
                        ),
                    ]);

                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items).block(block);
            frame.render_widget(list, area);
        }

        ActiveModal::Settings {
            selected_tab,
            selected_field,
            input_buffer,
        } => {
            let area = centered_rect(80, 70, frame.area());
            frame.render_widget(Clear, area);

            let block = Block::default()
                .title(" Settings (Tab: Switch Tab, Up/Down: Navigate, Enter: Apply/Select, Esc: Close) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_focused));

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(area.inner(ratatui::layout::Margin::new(1, 1)));

            let tabs = List::new(vec![
                ListItem::new(Line::from(" Keys")).style(if *selected_tab == 0 {
                    theme.bold(theme.accent)
                } else {
                    theme.text()
                }),
                ListItem::new(Line::from(" Models")).style(if *selected_tab == 1 {
                    theme.bold(theme.accent)
                } else {
                    theme.text()
                }),
            ])
            .block(Block::default().borders(Borders::RIGHT).title(" Tabs "));
            frame.render_widget(tabs, chunks[0]);

            if *selected_tab == 0 {
                let fields = [
                    ("Google Gemini Key", 0),
                    ("OpenAI API Key", 1),
                    ("Anthropic API Key", 2),
                    ("OpenRouter API Key", 3),
                    ("DeepSeek API Key", 4),
                    ("Custom API Key", 5),
                ];
                let mut lines = vec![];
                for (label, idx) in fields {
                    let is_selected = *selected_field == idx;
                    let val = if is_selected {
                        input_buffer.clone()
                    } else {
                        "********".to_string()
                    };
                    lines.push(Line::from(vec![
                        Span::styled(if is_selected { "▶ " } else { "  " }, theme.accent),
                        Span::styled(
                            label,
                            if is_selected {
                                theme.bold(theme.fg)
                            } else {
                                theme.text()
                            },
                        ),
                        Span::raw(": "),
                        Span::styled(val, theme.success),
                    ]));
                }
                frame.render_widget(
                    Paragraph::new(lines).block(Block::default().title(" API Keys ")),
                    chunks[1],
                );
            } else {
                let providers = ProviderType::all();
                let items: Vec<ListItem> = providers
                    .iter()
                    .enumerate()
                    .map(|(idx, p)| {
                        let is_selected = *selected_field == idx;
                        let active = if p == &app.config.active_provider {
                            " (Active)"
                        } else {
                            ""
                        };
                        ListItem::new(Line::from(format!(
                            "{}{}{}",
                            if is_selected { "▶ " } else { "  " },
                            p,
                            active
                        )))
                        .style(if is_selected {
                            theme.bold(theme.accent)
                        } else {
                            theme.text()
                        })
                    })
                    .collect();
                frame.render_widget(
                    List::new(items).block(Block::default().title(" Providers ")),
                    chunks[1],
                );
            }
            frame.render_widget(block, area);
        }

        ActiveModal::ToolApproval {
            tool_call_id,
            tool_name,
            arguments,
        } => {
            let area = centered_rect(65, 45, frame.area());
            frame.render_widget(Clear, area);

            let block = Block::default()
                .title(" Tool approval ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.warning));

            let lines = vec![
                Line::from(vec![
                    Span::styled("Run ", theme.text()),
                    Span::styled(tool_name.clone(), theme.bold(theme.warning)),
                    Span::styled(
                        format!("  ({})", tool_call_id),
                        Style::default().fg(theme.border),
                    ),
                ]),
                Line::raw(""),
                Line::styled("Arguments", theme.bold(theme.fg)),
                Line::styled(
                    arguments.to_string(),
                    Style::default().fg(theme.border_focused),
                ),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("[Y] ", theme.bold(theme.success)),
                    Span::styled("once   ", theme.text()),
                    Span::styled("[A] ", theme.bold(theme.accent)),
                    Span::styled("always   ", theme.text()),
                    Span::styled("[N] ", theme.bold(theme.error)),
                    Span::styled("deny", theme.text()),
                ]),
            ];

            let paragraph = Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false });
            frame.render_widget(paragraph, area);
        }

        ActiveModal::SessionPicker { selected_index } => {
            let area = centered_rect(72, 55, frame.area());
            frame.render_widget(Clear, area);
            let block = Block::default()
                .title(" Sessions (↑/↓ Enter resume, Esc close) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_focused));

            let sessions = crate::agent::session::list().unwrap_or_default();
            let items: Vec<ListItem> = if sessions.is_empty() {
                vec![ListItem::new(Line::from(Span::styled(
                    "  No saved sessions yet. Chat once and they persist automatically.",
                    Style::default().fg(theme.border),
                )))]
            } else {
                sessions
                    .iter()
                    .enumerate()
                    .map(|(idx, s)| {
                        let is_selected = idx == *selected_index;
                        let prefix = if is_selected { " ▶ " } else { "   " };
                        let active = if s.id == app.session_id {
                            " (current)"
                        } else {
                            ""
                        };
                        ListItem::new(Line::from(vec![
                            Span::styled(
                                format!("{prefix}{}", s.title),
                                if is_selected {
                                    theme.bold(theme.accent)
                                } else {
                                    theme.text()
                                },
                            ),
                            Span::styled(
                                format!("  {}{active}", &s.id[s.id.len().saturating_sub(8)..]),
                                Style::default().fg(theme.border),
                            ),
                        ]))
                    })
                    .collect()
            };
            frame.render_widget(List::new(items).block(block), area);
        }

        ActiveModal::Help => {
            let area = centered_rect(70, 65, frame.area());
            frame.render_widget(Clear, area);

            let block = Block::default()
                .title(" Orbis - Help & Commands (Press Esc or Enter to close) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_focused));

            let lines = vec![
                Line::styled("Commands", theme.bold(theme.accent)),
                Line::from(vec![
                    Span::styled("  /provider [name]", theme.bold(theme.fg)),
                    Span::raw("  gemini · openai · anthropic · openrouter · deepseek · custom"),
                ]),
                Line::from(vec![
                    Span::styled("  /model <name>", theme.bold(theme.fg)),
                    Span::raw("     set the active model id"),
                ]),
                Line::from(vec![
                    Span::styled("  /keys", theme.bold(theme.fg)),
                    Span::raw("             API key manager"),
                ]),
                Line::from(vec![
                    Span::styled("  /mode [agent|chat]", theme.bold(theme.fg)),
                    Span::raw("  tools on/off"),
                ]),
                Line::from(vec![
                    Span::styled("  /permission [ask|auto|read]", theme.bold(theme.fg)),
                    Span::raw("  tool safety"),
                ]),
                Line::from(vec![
                    Span::styled("  /sessions  /new  /resume [id]", theme.bold(theme.fg)),
                    Span::raw("  persist & restore chats"),
                ]),
                Line::from(vec![
                    Span::styled("  /compact", theme.bold(theme.fg)),
                    Span::raw("          shrink context (keeps tool pairing)"),
                ]),
                Line::from(vec![
                    Span::styled("  /endpoint <url>", theme.bold(theme.fg)),
                    Span::raw("   OpenAI-compatible base URL"),
                ]),
                Line::from(vec![Span::styled(
                    "  /ping  /tree  /status  /version  /debug  /clear  /help",
                    theme.bold(theme.fg),
                )]),
                Line::raw(""),
                Line::styled("Keys", theme.bold(theme.accent)),
                Line::raw("  Alt+P provider  ·  Alt+K keys  ·  Alt+S sessions  ·  Alt+M mode"),
                Line::raw(
                    "  Alt+A permission  ·  Alt+L new  ·  Alt+H help  ·  Alt+Q / Ctrl+C quit",
                ),
                Line::raw("  Enter send  ·  Alt+Enter newline  ·  Esc cancel / clear input"),
                Line::raw("  Tab focus  ·  ↑↓ history (input) or scroll (chat)"),
            ];

            let paragraph = Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false });
            frame.render_widget(paragraph, area);
        }
    }
}

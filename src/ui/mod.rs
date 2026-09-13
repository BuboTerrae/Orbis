pub mod chat;
pub mod header;
pub mod input;
pub mod modals;
pub mod statusbar;
pub mod theme;
pub mod todos;

use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

pub fn render(frame: &mut Frame, app: &App) {
    let theme = theme::Theme::default();
    let todo_h = todos::todo_panel_height(app);

    let mut constraints = vec![Constraint::Length(2), Constraint::Min(8)];
    if todo_h > 0 {
        constraints.push(Constraint::Length(todo_h));
    }
    constraints.push(Constraint::Length(6));
    constraints.push(Constraint::Length(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.area());

    header::render_header(frame, app, chunks[0], &theme);
    chat::render_chat(frame, app, chunks[1], &theme);

    let mut idx = 2;
    if todo_h > 0 {
        todos::render_todos(frame, app, chunks[idx], &theme);
        idx += 1;
    }
    input::render_input(frame, app, chunks[idx], &theme);
    statusbar::render_statusbar(frame, app, chunks[idx + 1], &theme);

    modals::render_modals(frame, app, &theme);
}

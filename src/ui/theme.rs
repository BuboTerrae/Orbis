use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    #[allow(dead_code)]
    pub bg: Color,
    pub fg: Color,
    pub border: Color,
    pub border_focused: Color,
    pub header_bg: Color,
    #[allow(dead_code)]
    pub header_fg: Color,
    pub user_badge: Color,
    pub assistant_badge: Color,
    pub tool_badge: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::Rgb(205, 214, 244),   // Catppuccin Text
            border: Color::Rgb(88, 91, 112), // Surface2
            border_focused: Color::Rgb(137, 180, 250), // Blue
            header_bg: Color::Rgb(30, 30, 46), // Mantle
            header_fg: Color::Rgb(205, 214, 244),
            user_badge: Color::Rgb(137, 220, 235),      // Cyan
            assistant_badge: Color::Rgb(203, 166, 247), // Mauve
            tool_badge: Color::Rgb(249, 226, 175),      // Yellow
            accent: Color::Rgb(137, 180, 250),          // Blue
            success: Color::Rgb(166, 227, 161),         // Green
            warning: Color::Rgb(250, 179, 135),         // Peach
            error: Color::Rgb(243, 139, 168),           // Red
        }
    }
}

impl Theme {
    pub fn bold(&self, color: Color) -> Style {
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    }

    pub fn text(&self) -> Style {
        Style::default().fg(self.fg)
    }
}

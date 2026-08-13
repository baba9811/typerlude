use crate::theme::{ThemeColor, ThemeSpec};
use anyhow::Result;
use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Copy, Debug)]
pub(super) struct ThemeStyles {
    pub(super) base: Style,
    pub(super) accent: Style,
    pub(super) correct: Style,
    pub(super) error: Style,
    pub(super) cursor: Style,
    pub(super) dim: Style,
}

pub(super) fn styles(theme: &ThemeSpec) -> Result<ThemeStyles> {
    let palette = theme.palette()?;
    let background = color(palette.background);
    let role = |value| Style::default().fg(color(value)).bg(background);
    Ok(ThemeStyles {
        base: role(palette.foreground),
        accent: role(palette.accent),
        correct: role(palette.correct),
        error: role(palette.error).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        cursor: role(palette.cursor).add_modifier(Modifier::BOLD | Modifier::REVERSED),
        dim: role(palette.dim),
    })
}

const fn color(color: ThemeColor) -> Color {
    match color {
        ThemeColor::Reset => Color::Reset,
        ThemeColor::Black => Color::Black,
        ThemeColor::Red => Color::Red,
        ThemeColor::Green => Color::Green,
        ThemeColor::Yellow => Color::Yellow,
        ThemeColor::Blue => Color::Blue,
        ThemeColor::Magenta => Color::Magenta,
        ThemeColor::Cyan => Color::Cyan,
        ThemeColor::Gray => Color::Gray,
        ThemeColor::DarkGray => Color::DarkGray,
        ThemeColor::LightRed => Color::LightRed,
        ThemeColor::LightGreen => Color::LightGreen,
        ThemeColor::LightYellow => Color::LightYellow,
        ThemeColor::LightBlue => Color::LightBlue,
        ThemeColor::LightMagenta => Color::LightMagenta,
        ThemeColor::LightCyan => Color::LightCyan,
        ThemeColor::White => Color::White,
        ThemeColor::Rgb(red, green, blue) => Color::Rgb(red, green, blue),
    }
}

#[cfg(test)]
mod tests {
    use super::styles;
    use crate::theme::parse_theme;
    use ratatui::style::{Color, Modifier};

    fn source() -> String {
        r##"schema_version = 1
id = "adapter-test"
background = "black"
foreground = "white"
accent = "cyan"
correct = "green"
error = "red"
cursor = "yellow"
dim = "dark_gray"
"##
        .into()
    }

    #[test]
    fn named_and_rgb_colors_map_exactly_to_ratatui() {
        for (value, expected) in [
            ("reset", Color::Reset),
            ("black", Color::Black),
            ("red", Color::Red),
            ("green", Color::Green),
            ("yellow", Color::Yellow),
            ("blue", Color::Blue),
            ("magenta", Color::Magenta),
            ("cyan", Color::Cyan),
            ("gray", Color::Gray),
            ("dark_gray", Color::DarkGray),
            ("light_red", Color::LightRed),
            ("light_green", Color::LightGreen),
            ("light_yellow", Color::LightYellow),
            ("light_blue", Color::LightBlue),
            ("light_magenta", Color::LightMagenta),
            ("light_cyan", Color::LightCyan),
            ("white", Color::White),
            ("#000000", Color::Rgb(0, 0, 0)),
            ("#FFFFFF", Color::Rgb(255, 255, 255)),
            ("#aBcDeF", Color::Rgb(0xab, 0xcd, 0xef)),
        ] {
            let theme = parse_theme(
                &source().replace("accent = \"cyan\"", &format!("accent = \"{value}\"")),
            )
            .unwrap();
            assert_eq!(styles(&theme).unwrap().accent.fg, Some(expected), "{value}");
        }
    }

    #[test]
    fn role_styles_preserve_background_and_emphasis() {
        let styles = styles(&parse_theme(&source()).unwrap()).unwrap();

        assert_eq!(styles.base.fg, Some(Color::White));
        assert_eq!(styles.base.bg, Some(Color::Black));
        assert_eq!(styles.accent.bg, Some(Color::Black));
        assert_eq!(styles.correct.bg, Some(Color::Black));
        assert_eq!(styles.error.bg, Some(Color::Black));
        assert_eq!(styles.cursor.bg, Some(Color::Black));
        assert_eq!(styles.dim.bg, Some(Color::Black));
        assert!(
            styles
                .error
                .add_modifier
                .contains(Modifier::BOLD | Modifier::UNDERLINED)
        );
        assert!(
            styles
                .cursor
                .add_modifier
                .contains(Modifier::BOLD | Modifier::REVERSED)
        );
    }
}

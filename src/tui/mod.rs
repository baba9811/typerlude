mod boss_battle;
mod content;
mod format;
mod game;
mod home;
mod practice;
mod result;
mod settings;
mod stats;
mod theme;

use self::content::{render_content, render_content_detail};
use self::game::{render_game, render_game_options, render_game_result, render_games};
use self::home::{render_home, render_mode_options};
use self::practice::render_practice;
use self::result::render_result;
use self::settings::{render_help, render_settings, render_themes};
use self::stats::{render_goals, render_history, render_stats, render_weak_keys};
use self::theme::{ThemeStyles, styles as theme_styles};
use crate::{
    app::{ActivePractice, App, Screen},
    diagnostic::terminal_safe,
    i18n::{TextKey, text},
    model::{Language, PracticeKind},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;

pub(crate) const fn supports_size(width: u16, height: u16) -> bool {
    width >= MIN_WIDTH && height >= MIN_HEIGHT
}

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    if !supports_size(area.width, area.height) {
        render_too_small(frame, app.settings.ui_language, area);
        return;
    }

    let Some(styles) = selected_styles(app) else {
        return;
    };
    frame.render_widget(Block::default().style(styles.base), area);

    let warning = warning_text(&app.warnings);
    let warning_height = if warning.is_empty() { 0 } else { 5 };
    let regions =
        Layout::vertical([Constraint::Min(1), Constraint::Length(warning_height)]).split(area);
    let main = regions[0];
    match app.screen() {
        Screen::Home => render_home(frame, app, main, styles),
        Screen::ModeOptions => render_mode_options(frame, app, main, styles),
        Screen::Practice => render_practice(frame, app, main, styles),
        Screen::Result => render_result(frame, app, main, styles),
        Screen::Games => render_games(frame, app, main, styles),
        Screen::GameOptions => render_game_options(frame, app, main, styles),
        Screen::Game => render_game(frame, app, main, styles),
        Screen::GameResult => render_game_result(frame, app, main, styles),
        Screen::Stats => render_stats(frame, app, main, styles),
        Screen::History => render_history(frame, app, main, styles),
        Screen::WeakKeys => render_weak_keys(frame, app, main, styles),
        Screen::Goals => render_goals(frame, app, main, styles),
        Screen::Content => render_content(frame, app, main, styles),
        Screen::ContentDetail => render_content_detail(frame, app, main, styles),
        Screen::Settings => render_settings(frame, app, main, styles),
        Screen::Themes => render_themes(frame, app, main, styles),
        Screen::Help => render_help(frame, app, main, styles),
    }
    if warning_height != 0 {
        render_warning(
            frame,
            app.settings.ui_language,
            &warning,
            regions[1],
            styles,
        );
    }
}

pub fn practice_cursor(area: Rect, active: &ActivePractice) -> Option<(u16, u16)> {
    if area.width == 0 || area.height == 0 {
        return None;
    }

    let (column, row) = practice_cursor_offset(usize::from(area.width), active);
    let scroll = practice_scroll(area, active);
    let column = column.min(usize::from(area.width - 1)) as u16;
    let row = row.saturating_sub(scroll).min(usize::from(area.height - 1)) as u16;
    Some((area.x.saturating_add(column), area.y.saturating_add(row)))
}

fn practice_cursor_offset(width: usize, active: &ActivePractice) -> (usize, usize) {
    let mut column = 0_usize;
    let mut row = 0_usize;
    for (grapheme, _) in active.engine.target_cells().take(active.engine.cursor()) {
        if grapheme == "\n" {
            column = 0;
            row = row.saturating_add(1);
            continue;
        }
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if column != 0 && column.saturating_add(grapheme_width) > width {
            column = 0;
            row = row.saturating_add(1);
        }
        column = column.saturating_add(grapheme_width);
        if column >= width {
            row = row.saturating_add(1);
            column = 0;
        }
    }

    (column, row)
}

fn practice_scroll(area: Rect, active: &ActivePractice) -> usize {
    let (_, row) = practice_cursor_offset(usize::from(area.width), active);
    let visible_before_cursor = if active.kind() == PracticeKind::Long {
        usize::from(area.height / 2)
    } else {
        usize::from(area.height.saturating_sub(1))
    };
    row.saturating_sub(visible_before_cursor)
}

fn selected_styles(app: &App) -> Option<ThemeStyles> {
    let preview = (app.screen() == Screen::Themes)
        .then(|| app.themes.ids().nth(app.focus()))
        .flatten();
    preview
        .and_then(|id| app.themes.get(id))
        .and_then(|theme| theme_styles(theme).ok())
        .or_else(|| {
            app.themes
                .get(&app.settings.theme)
                .and_then(|theme| theme_styles(theme).ok())
        })
        .or_else(|| {
            app.themes
                .get("default")
                .and_then(|theme| theme_styles(theme).ok())
        })
}

fn render_too_small(frame: &mut Frame<'_>, language: Language, area: Rect) {
    let (minimum, current) = match language {
        Language::Ko => ("최소 크기", "현재 크기"),
        Language::En => ("Minimum size", "Current size"),
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{}\n{minimum}: {MIN_WIDTH}x{MIN_HEIGHT}\n{current}: {}x{}",
            text(language, TextKey::TooSmall),
            area.width,
            area.height
        ))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false }),
        area,
    );
}

fn titled<'a>(title: &'a str, styles: ThemeStyles) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(styles.accent)
        .style(styles.base)
        .title(Span::styled(title, styles.accent))
}

fn warning_text(warnings: &[String]) -> String {
    warnings
        .iter()
        .map(|warning| {
            terminal_safe(warning)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|warning| !warning.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn render_warning(
    frame: &mut Frame<'_>,
    language: Language,
    warning: &str,
    area: Rect,
    styles: ThemeStyles,
) {
    frame.render_widget(
        Paragraph::new(warning)
            .style(styles.error)
            .wrap(Wrap { trim: true })
            .block(titled(text(language, TextKey::CorruptFile), styles)),
        area,
    );
}

fn no_data(language: Language, styles: ThemeStyles) -> Paragraph<'static> {
    Paragraph::new(text(language, TextKey::NoData)).style(styles.dim)
}

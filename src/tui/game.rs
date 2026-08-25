use super::{format::language_name, theme::ThemeStyles, titled};
use crate::{
    app::App,
    game::{GameKind, word_rain::LOGICAL_WIDTH},
    i18n::{TextKey, text},
    model::{Difficulty, Language},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Wrap},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(super) fn render_games(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::HomeGames), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let items = GameKind::ALL.into_iter().enumerate().map(|(index, kind)| {
        let marker = if index == app.focus() { "> " } else { "  " };
        ListItem::new(Line::from(vec![
            Span::styled(marker, styles.accent),
            Span::styled(game_name(language, kind), styles.base),
        ]))
    });
    frame.render_widget(List::new(items).style(styles.base), inner);
}

pub(super) fn render_game_options(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    styles: ThemeStyles,
) {
    let language = app.settings.ui_language;
    let options = app.game_options();
    let block = titled(game_name(language, options.kind), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let marker = |index| if index == app.focus() { "> " } else { "  " };
    let mut lines = vec![
        Line::from(vec![
            Span::styled(marker(0), styles.accent),
            Span::styled(
                format!(
                    "{}: {}",
                    text(language, TextKey::Language),
                    language_name(options.language)
                ),
                styles.base,
            ),
        ]),
        Line::from(vec![
            Span::styled(marker(1), styles.accent),
            Span::styled(
                format!(
                    "{}: {}",
                    text(language, TextKey::Difficulty),
                    difficulty_name(language, options.difficulty)
                ),
                styles.base,
            ),
        ]),
        Line::from(vec![
            Span::styled(marker(2), styles.accent),
            Span::styled(text(language, TextKey::Start), styles.base),
        ]),
    ];
    if let Some(error) = &options.error {
        lines.push(Line::from(Span::styled(error.clone(), styles.error)));
    }
    lines.push(Line::from(match language {
        Language::Ko => "←→: 변경 · Enter: 선택 · Esc: 뒤로",
        Language::En => "←→: Change · Enter: Select · Esc: Back",
    }));
    frame.render_widget(Paragraph::new(lines).style(styles.base), inner);
}

pub(super) fn render_game(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let Some(active) = app.active_word_rain() else {
        frame.render_widget(titled(text(language, TextKey::WordRain), styles), area);
        return;
    };
    let regions = Layout::vertical([
        Constraint::Min(8),
        Constraint::Length(1),
        Constraint::Length(5),
    ])
    .split(area);
    let title = format!(
        "{} · {}",
        text(language, TextKey::WordRain),
        difficulty_name(language, active.game.difficulty())
    );
    let playfield = titled(&title, styles).title_bottom(Span::styled(
        format!(" {} ", text(language, TextKey::CollisionLine)),
        styles.error,
    ));
    let sky = playfield.inner(regions[0]);
    frame.render_widget(playfield, regions[0]);

    for word in active.game.active_words() {
        if sky.width == 0 || sky.height == 0 {
            break;
        }
        let targeted = active.game.target_id() == Some(word.id());
        let marker_width = u16::from(targeted) * 2;
        let render_width = word.width().saturating_add(marker_width).min(sky.width);
        let max_left = sky.width.saturating_sub(render_width);
        let logical_max = LOGICAL_WIDTH.saturating_sub(word.width()).max(1);
        let scaled = u32::from(word.left()) * u32::from(max_left) / u32::from(logical_max);
        let row = ((word.progress().clamp(0.0, 1.0) * f64::from(sky.height)).floor() as u16)
            .min(sky.height - 1);
        let matched = active.game.matched_graphemes(word.id());
        let mut spans = Vec::new();
        if targeted {
            spans.push(Span::styled("▶ ", styles.accent));
        }
        spans.extend(
            word.text()
                .graphemes(true)
                .enumerate()
                .map(|(index, grapheme)| {
                    if index < matched {
                        Span::styled(
                            grapheme.to_owned(),
                            styles.correct.add_modifier(Modifier::UNDERLINED),
                        )
                    } else {
                        Span::styled(grapheme.to_owned(), styles.base)
                    }
                }),
        );
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect::new(
                sky.x.saturating_add(scaled as u16),
                sky.y.saturating_add(row),
                render_width,
                1,
            ),
        );
    }

    frame.render_widget(
        Paragraph::new(format!(
            "{}: {} · {}: {} · {}: {}",
            text(language, TextKey::Score),
            active.game.score(),
            text(language, TextKey::Level),
            active.game.current_level(),
            text(language, TextKey::Combo),
            active.game.combo(),
        ))
        .alignment(Alignment::Center)
        .style(styles.base),
        regions[1],
    );

    let target = active
        .game
        .target_id()
        .and_then(|id| active.game.active_words().find(|word| word.id() == id))
        .map(|word| word.text())
        .unwrap_or("—");
    let entered = active.game.input();
    let invalid = !active.game.input().is_empty() && !active.game.input_is_valid();
    let input = vec![
        Line::from(vec![
            Span::styled("> ", styles.accent),
            Span::styled(entered, if invalid { styles.error } else { styles.base }),
            Span::styled(
                if invalid {
                    format!("  ! {}", text(language, TextKey::CorrectionNeeded))
                } else {
                    String::new()
                },
                styles.error,
            ),
        ]),
        Line::from(format!("{}: {target}", text(language, TextKey::Target))),
        Line::from(match language {
            Language::Ko => "Esc: 일시 정지 · Backspace: 수정",
            Language::En => "Esc: Pause · Backspace: Correct",
        }),
    ];
    let input_block = titled(text(language, TextKey::Input), styles);
    let input_area = input_block.inner(regions[2]);
    frame.render_widget(
        Paragraph::new(input).style(styles.base).block(input_block),
        regions[2],
    );
    if !active.game.is_paused() && input_area.width > 2 && input_area.height > 0 {
        let entered_width = UnicodeWidthStr::width(entered)
            .min(usize::from(input_area.width.saturating_sub(3)))
            as u16;
        frame.set_cursor_position((input_area.x + 2 + entered_width, input_area.y));
    }

    if active.game.is_paused() {
        let overlay = centered(regions[0], 54, 5);
        let message = if active.leave_confirmation {
            text(language, TextKey::LeaveGameConfirm)
        } else {
            match language {
                Language::Ko => "Esc: 계속 · q: 나가기",
                Language::En => "Esc: Resume · q: Leave",
            }
        };
        frame.render_widget(
            Paragraph::new(message)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .style(styles.base)
                .block(titled(text(language, TextKey::Pause), styles)),
            overlay,
        );
    }
}

pub(super) fn render_game_result(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    styles: ThemeStyles,
) {
    let language = app.settings.ui_language;
    let Some(result) = app.game_result() else {
        frame.render_widget(titled(text(language, TextKey::WordRain), styles), area);
        return;
    };
    let lines = vec![
        Line::from(Span::styled(
            text(language, TextKey::GameOver),
            styles.accent,
        )),
        Line::from(format!(
            "{}: {}",
            text(language, TextKey::Score),
            result.score
        )),
        Line::from(format!(
            "{}: {}",
            text(language, TextKey::Cleared),
            result.cleared
        )),
        Line::from(format!(
            "{}: {}",
            text(language, TextKey::MaxCombo),
            result.max_combo
        )),
        Line::from(format!(
            "{}: {}",
            text(language, TextKey::Level),
            result.level
        )),
        Line::from(format!(
            "{}: {:.1}s",
            text(language, TextKey::Duration),
            result.active_time.as_secs_f64()
        )),
        Line::from(format!(
            "{}: {}",
            text(language, TextKey::MissedWord),
            result.missed_word
        )),
        Line::from(""),
        Line::from(format!(
            "Enter: {} · Esc: {}",
            text(language, TextKey::Retry),
            text(language, TextKey::HomeGames)
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(styles.base)
            .block(titled(text(language, TextKey::WordRain), styles)),
        area,
    );
}

const fn game_name(language: Language, kind: GameKind) -> &'static str {
    match kind {
        GameKind::WordRain => text(language, TextKey::WordRain),
    }
}

const fn difficulty_name(language: Language, difficulty: Difficulty) -> &'static str {
    let key = match difficulty {
        Difficulty::Easy => TextKey::Easy,
        Difficulty::Medium => TextKey::Medium,
        Difficulty::Hard => TextKey::Hard,
        Difficulty::Mixed => TextKey::Mixed,
    };
    text(language, key)
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height.min(area.height)),
        Constraint::Fill(1),
    ])
    .split(area)[1];
    Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width.min(vertical.width)),
        Constraint::Fill(1),
    ])
    .split(vertical)[1]
}

use super::{
    format::{focus_marker, language_name, practice_name, terminal_line, toggle_name},
    result::render_update_notice,
    theme::ThemeStyles,
    titled,
};
use crate::{
    app::{App, QUICK_COUNT_PRESETS, QUICK_TIME_PRESETS, TEST_DURATION_PRESETS, key_stages},
    i18n::{TextKey, text},
    model::{Difficulty, Language, PracticeKind},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

pub(super) fn render_home(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::AppTitle), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let regions = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(if app.update_notice.is_some() { 3 } else { 0 }),
    ])
    .split(inner);
    let actions = [
        TextKey::HomeQuick,
        TextKey::HomeKeys,
        TextKey::HomeWords,
        TextKey::HomeSentence,
        TextKey::HomeLong,
        TextKey::HomeTest,
        TextKey::HomeStats,
        TextKey::HomeGoals,
        TextKey::HomeContent,
        TextKey::HomeSettings,
    ];
    let items = actions.into_iter().enumerate().map(|(index, key)| {
        let marker = if index == app.focus() { "> " } else { "  " };
        ListItem::new(Line::from(vec![
            Span::styled(marker, styles.accent),
            Span::styled(text(language, key), styles.base),
        ]))
    });
    frame.render_widget(List::new(items).style(styles.base), regions[0]);
    render_update_notice(frame, app, regions[1], styles);
}

pub(super) fn render_mode_options(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    styles: ThemeStyles,
) {
    let language = app.settings.ui_language;
    let options = app.mode_options();
    let block = titled(practice_name(language, options.kind), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let row = |index: usize, label: &str, value: String| {
        Line::from(vec![
            Span::styled(focus_marker(app, index), styles.accent),
            Span::styled(format!("{label}: {value}"), styles.base),
        ])
    };
    let start = |index| {
        Line::from(vec![
            Span::styled(focus_marker(app, index), styles.accent),
            Span::styled(text(language, TextKey::Start), styles.base),
        ])
    };
    let long_row = |index: usize, label: &str, value: &str| {
        let marker = focus_marker(app, index);
        let width = inner
            .width
            .saturating_sub(UnicodeWidthStr::width(marker) as u16);
        Line::from(vec![
            Span::styled(marker, styles.accent),
            Span::styled(
                terminal_line(&format!("{label}: {value}"), width),
                styles.base,
            ),
        ])
    };
    let long_line = |value: String| Line::from(terminal_line(&value, inner.width));
    let mut lines = match options.kind {
        PracticeKind::Quick => {
            let source = match options.quick_source {
                crate::app::QuickSource::Words => text(language, TextKey::Words),
                crate::app::QuickSource::Quote => text(language, TextKey::Quote),
            };
            let stop = if options.quick_items {
                text(language, TextKey::Items)
            } else {
                text(language, TextKey::Time)
            };
            let preset = if options.quick_items {
                QUICK_COUNT_PRESETS[options.quick_preset].to_string()
            } else {
                format!("{}s", QUICK_TIME_PRESETS[options.quick_preset])
            };
            vec![
                row(
                    0,
                    text(language, TextKey::Language),
                    language_name(options.language).into(),
                ),
                row(1, text(language, TextKey::Source), source.into()),
                row(2, text(language, TextKey::Stop), stop.into()),
                row(3, text(language, TextKey::Preset), preset),
                start(4),
            ]
        }
        PracticeKind::Key => vec![
            row(
                0,
                text(language, TextKey::Language),
                language_name(options.language).into(),
            ),
            row(
                1,
                text(language, TextKey::Stage),
                format!(
                    "{}: {}",
                    options.key_stage,
                    key_stages(options.language)[usize::from(options.key_stage - 1)].title
                ),
            ),
            row(
                2,
                text(language, TextKey::Random),
                toggle_name(language, options.key_random).into(),
            ),
            row(
                3,
                text(language, TextKey::RepeatWeakKeys),
                toggle_name(language, options.key_weak_repeat).into(),
            ),
            start(4),
        ],
        PracticeKind::Words => vec![
            row(
                0,
                text(language, TextKey::Language),
                language_name(options.language).into(),
            ),
            row(
                1,
                text(language, TextKey::Difficulty),
                difficulty_name(language, options.word_difficulty).into(),
            ),
            start(2),
        ],
        PracticeKind::Sentence => vec![
            row(
                0,
                text(language, TextKey::Language),
                language_name(options.language).into(),
            ),
            start(1),
        ],
        PracticeKind::Long => {
            let items = app.long_items(options.language, None);
            let mut lines = vec![long_row(
                0,
                text(language, TextKey::Language),
                language_name(options.language),
            )];
            if let Some(item) = items.get(options.long_selection) {
                let visible = usize::from(inner.height)
                    .saturating_sub(8)
                    .max(1)
                    .min(items.len());
                let start = options
                    .long_selection
                    .saturating_sub(visible / 2)
                    .min(items.len().saturating_sub(visible));
                lines.extend(items[start..start + visible].iter().enumerate().map(
                    |(offset, item)| {
                        long_row(
                            start + offset + 1,
                            item.title.as_deref().unwrap_or(&item.id),
                            "",
                        )
                    },
                ));
                lines.extend([
                    long_line(format!(
                        "{}: {}",
                        text(language, TextKey::Title),
                        item.title.as_deref().unwrap_or(&item.id)
                    )),
                    long_line(format!(
                        "{}: {}",
                        text(language, TextKey::Author),
                        item.source.author
                    )),
                    long_line(format!(
                        "{}: {}",
                        text(language, TextKey::Source),
                        item.source.source_url
                    )),
                    long_line(format!(
                        "{}: {}",
                        text(language, TextKey::License),
                        item.source.license
                    )),
                    long_line(format!(
                        "{}: {}",
                        text(language, TextKey::Difficulty),
                        item.difficulty
                            .map_or_else(|| "-".into(), |value| value.to_string())
                    )),
                    long_line(format!(
                        "{}: {}",
                        text(language, TextKey::Tags),
                        item.tags.join(", ")
                    )),
                ]);
            } else {
                lines.push(long_line(text(language, TextKey::NoData).into()));
            }
            lines
        }
        PracticeKind::Test => {
            let items = app.long_items(options.language, None);
            let selection = options
                .test_selection
                .checked_sub(1)
                .and_then(|index| items.get(index))
                .map_or_else(
                    || text(language, TextKey::Random).into(),
                    |item| item.title.as_deref().unwrap_or(&item.id).into(),
                );
            vec![
                row(
                    0,
                    text(language, TextKey::Language),
                    language_name(options.language).into(),
                ),
                row(
                    1,
                    text(language, TextKey::Duration),
                    format!("{}s", TEST_DURATION_PRESETS[options.test_preset]),
                ),
                row(2, text(language, TextKey::Text), selection),
                start(3),
            ]
        }
    };
    let instruction = format!(
        "←→ / Enter {} · Esc {}",
        text(language, TextKey::Confirm),
        text(language, TextKey::Back)
    );
    lines.push(Line::from(if options.kind == PracticeKind::Long {
        terminal_line(&instruction, inner.width)
    } else {
        instruction
    }));
    let paragraph = Paragraph::new(lines).style(styles.base);
    if options.kind == PracticeKind::Long {
        frame.render_widget(paragraph, inner);
    } else {
        frame.render_widget(paragraph.wrap(Wrap { trim: false }), inner);
    }
}

const fn difficulty_name(language: Language, difficulty: Difficulty) -> &'static str {
    text(
        language,
        match difficulty {
            Difficulty::Easy => TextKey::Easy,
            Difficulty::Medium => TextKey::Medium,
            Difficulty::Hard => TextKey::Hard,
            Difficulty::Mixed => TextKey::Mixed,
        },
    )
}

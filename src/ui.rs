use crate::{
    app::{
        ActivePractice, App, CustomTextSource, Grade, PracticeMode, QUICK_COUNT_PRESETS,
        QUICK_TIME_PRESETS, Screen, StopRule, TEST_DURATION_PRESETS, key_stages,
    },
    cli::terminal_safe,
    content::ContentKind,
    i18n::{TextKey, result_actions, text},
    model::{Difficulty, Language, PracticeKind},
    stats::{adaptive_candidates, history, intended_key_counts, streak, summarize, weak_keys},
    theme::ThemeStyles,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    symbols,
    text::{Line, Span, Text},
    widgets::{
        Axis, Block, Borders, Chart, Dataset, Gauge, GraphType, List, ListItem, Paragraph,
        Sparkline, Wrap,
    },
};
use std::{mem, time::Duration};
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
        Screen::ModeSelect => render_mode_select(frame, app, main, styles),
        Screen::ModeOptions => render_mode_options(frame, app, main, styles),
        Screen::Practice => render_practice(frame, app, main, styles),
        Screen::Result => render_result(frame, app, main, styles),
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
    app.themes
        .get(&app.settings.theme)
        .and_then(|theme| theme.styles().ok())
        .or_else(|| {
            app.themes
                .get("default")
                .and_then(|theme| theme.styles().ok())
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

fn render_home(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
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

fn render_mode_select(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let actions = [
        TextKey::HomeQuick,
        TextKey::HomeKeys,
        TextKey::HomeWords,
        TextKey::HomeSentence,
        TextKey::HomeLong,
        TextKey::HomeTest,
    ];
    let title = actions
        .get(app.focus())
        .copied()
        .unwrap_or(TextKey::HomeQuick);
    let block = titled(text(language, title), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let regions = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(inner);
    frame.render_widget(
        List::new(actions.into_iter().enumerate().map(|(index, key)| {
            let marker = if index == app.focus() { "> " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::styled(marker, styles.accent),
                Span::styled(text(language, key), styles.base),
            ]))
        }))
        .style(styles.base),
        regions[0],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "Tab / ↑↓ · Enter {} · Esc {}",
            text(language, TextKey::Confirm),
            text(language, TextKey::Back)
        ))
        .style(styles.dim),
        regions[1],
    );
}

fn render_mode_options(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
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
            let mut lines = vec![row(
                0,
                text(language, TextKey::Language),
                language_name(options.language).into(),
            )];
            lines.extend(items.iter().enumerate().map(|(index, item)| {
                row(
                    index + 1,
                    item.title.as_deref().unwrap_or(&item.id),
                    String::new(),
                )
            }));
            if let Some(item) = items.get(options.long_selection) {
                lines.extend([
                    Line::from(""),
                    Line::from(format!(
                        "{}: {}",
                        text(language, TextKey::Title),
                        terminal_safe(item.title.as_deref().unwrap_or(&item.id))
                    )),
                    Line::from(format!(
                        "{}: {}",
                        text(language, TextKey::Author),
                        terminal_safe(&item.source.author)
                    )),
                    Line::from(format!(
                        "{}: {}",
                        text(language, TextKey::Source),
                        terminal_safe(&item.source.source_url)
                    )),
                    Line::from(format!(
                        "{}: {}",
                        text(language, TextKey::License),
                        terminal_safe(&item.source.license)
                    )),
                    Line::from(format!(
                        "{}: {}",
                        text(language, TextKey::Difficulty),
                        item.difficulty
                            .map_or_else(|| "-".into(), |value| value.to_string())
                    )),
                    Line::from(format!(
                        "{}: {}",
                        text(language, TextKey::Tags),
                        terminal_safe(&item.tags.join(", "))
                    )),
                ]);
            } else {
                lines.push(Line::from(text(language, TextKey::NoData)));
            }
            lines
        }
        PracticeKind::Test => vec![
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
            start(2),
        ],
    };
    lines.push(Line::from(format!(
        "←→ / Enter {} · Esc {}",
        text(language, TextKey::Confirm),
        text(language, TextKey::Back)
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .style(styles.base)
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_practice(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::Progress), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(active) = app.active_practice() else {
        frame.render_widget(no_data(language, styles), inner);
        return;
    };
    let key_mode = active.kind() == PracticeKind::Key;
    let long_mode = active.kind() == PracticeKind::Long;
    let keyboard_height = u16::from(key_mode && app.settings.show_keyboard) * 4;
    let finger_height = u16::from(key_mode && app.settings.show_finger_guide);
    let live_height = if long_mode { 5 } else { 2 };
    let regions = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(live_height),
        Constraint::Length(keyboard_height),
        Constraint::Length(finger_height),
        Constraint::Length(2),
    ])
    .split(inner);
    let scroll = practice_scroll(regions[0], active);
    let target = target_lines(active, regions[0].width, scroll, regions[0].height, styles);
    frame.render_widget(
        Paragraph::new(Text::from(target)).style(styles.base),
        regions[0],
    );
    if let Some(progress) = active.long_scroll() {
        let live =
            Layout::vertical([Constraint::Length(4), Constraint::Length(1)]).split(regions[1]);
        frame.render_widget(
            Paragraph::new(practice_live_lines(active, language)).style(styles.base),
            live[0],
        );
        let paragraph = match language {
            Language::Ko => "문단",
            Language::En => "Paragraph",
        };
        frame.render_widget(
            Gauge::default()
                .ratio(progress.percent as f64 / 100.0)
                .label(format!(
                    "{paragraph} {}/{} · {}%",
                    progress.active_paragraph, progress.total_paragraphs, progress.percent
                ))
                .gauge_style(styles.accent),
            live[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new(practice_live_lines(active, language)).style(styles.base),
            regions[1],
        );
    }
    if keyboard_height != 0 {
        frame.render_widget(
            Paragraph::new(keyboard_lines(active, styles)).style(styles.base),
            regions[2],
        );
    }
    if finger_height != 0 {
        frame.render_widget(
            Paragraph::new(finger_line(active, language, styles)).style(styles.accent),
            regions[3],
        );
    }
    let mut footer = Vec::new();
    if active.kind() == PracticeKind::Test {
        footer.push(Line::from(if active.leave_confirmation() {
            test_leave_confirmation(language)
        } else {
            test_leave_instruction(language)
        }));
    } else {
        let action = if active.engine.is_paused() {
            TextKey::Resume
        } else {
            TextKey::Pause
        };
        footer.push(Line::from(format!(
            "{}: Esc / Ctrl+P",
            text(language, action)
        )));
        if active.leave_confirmation() {
            footer.push(Line::from(leave_confirmation(language)));
        } else if let Some(status) = app.practice_status() {
            footer.push(Line::from(terminal_safe(status)));
        } else if active.engine.is_paused() {
            footer.push(Line::from(match language {
                Language::Ko => "q: 연습 끝내기",
                Language::En => "q: Finish practice",
            }));
        }
    }
    frame.render_widget(Paragraph::new(footer).style(styles.dim), regions[4]);
    if !active.engine.is_paused()
        && let Some(cursor) = practice_cursor(regions[0], active)
    {
        frame.set_cursor_position(cursor);
    }
}

fn practice_live_lines(active: &ActivePractice, language: Language) -> Vec<Line<'static>> {
    let metrics = active.live_metrics();
    if let PracticeMode::Key { stage, .. } = active.mode {
        let title = key_stages(active.engine.language())
            .get(usize::from(stage).saturating_sub(1))
            .map_or("", |stage| stage.title);
        let (completed, streak) = key_progress(active);
        let stage_label = match language {
            Language::Ko => "단계",
            Language::En => "Stage",
        };
        return vec![
            Line::from(format!(
                "{stage_label} {stage}: {title} · {}: {:.1}% · {}: {streak}",
                text(language, TextKey::Accuracy),
                metrics.accuracy,
                text(language, TextKey::Streak),
            )),
            Line::from(format!(
                "{}: {completed}/{} · {}: {}",
                text(language, TextKey::Progress),
                active.engine.target_len(),
                text(language, TextKey::Errors),
                metrics.errors,
            )),
        ];
    }
    let speed = match active.engine.language() {
        Language::Ko => metrics.kpm,
        Language::En => metrics.wpm,
    };
    let summary = format!(
        "{}: {speed:.1} · {}: {:.1}% · {}: {}",
        text(language, TextKey::Speed),
        text(language, TextKey::Accuracy),
        metrics.accuracy,
        text(language, TextKey::Errors),
        metrics.errors
    );
    if let PracticeMode::Long { item_id, .. } = &active.mode {
        let (title, author, source, license, difficulty, tags) =
            active.long_metadata().map_or_else(
                || (item_id.as_str(), "", "", "", None, String::new()),
                |metadata| {
                    let (author, source, license) = match (metadata.custom_source, language) {
                        (Some(CustomTextSource::File), Language::Ko) => {
                            ("로컬 파일", "사용자 제공 텍스트", "재배포하지 않음")
                        }
                        (Some(CustomTextSource::Stdin), Language::Ko) => {
                            ("표준 입력", "사용자 제공 텍스트", "재배포하지 않음")
                        }
                        (Some(CustomTextSource::File), Language::En) => {
                            ("Local file", "User-provided text", "Not redistributed")
                        }
                        (Some(CustomTextSource::Stdin), Language::En) => {
                            ("Standard input", "User-provided text", "Not redistributed")
                        }
                        (None, _) => (
                            metadata.author.as_str(),
                            metadata.source.as_str(),
                            metadata.license.as_str(),
                        ),
                    };
                    (
                        metadata.title.as_str(),
                        author,
                        source,
                        license,
                        metadata.difficulty,
                        metadata.tags.join(", "),
                    )
                },
            );
        let difficulty_label = match language {
            Language::Ko => "난이도",
            Language::En => "Difficulty",
        };
        let tags_label = match language {
            Language::Ko => "태그",
            Language::En => "Tags",
        };
        let details = match difficulty {
            Some(difficulty) if !tags.is_empty() => {
                format!("{difficulty_label}: {difficulty} · {tags_label}: {tags} · {summary}")
            }
            Some(difficulty) => format!("{difficulty_label}: {difficulty} · {summary}"),
            None if !tags.is_empty() => format!("{tags_label}: {tags} · {summary}"),
            None => summary,
        };
        return vec![
            Line::from(title.to_owned()),
            Line::from(format!("{author} · {license}")),
            Line::from(source.to_owned()),
            Line::from(details),
        ];
    }
    let detail = match &active.mode {
        PracticeMode::Quick { completed } => {
            let remaining = match active.stop {
                StopRule::ActiveTime(limit) => {
                    format!("{}s", limit.saturating_sub(metrics.active).as_secs())
                }
                StopRule::Items(total) => total.saturating_sub(*completed).to_string(),
                StopRule::TargetEnd => "0".into(),
            };
            format!(
                "{}: {completed} · {}: {remaining}",
                text(language, TextKey::Progress),
                text(language, TextKey::Remaining)
            )
        }
        PracticeMode::Words {
            completed, streak, ..
        } => {
            let current = active.current_item_delta().map_or(0.0, |delta| delta.speed);
            let (current_label, average_label) = current_average(language);
            format!(
                "{current_label}: {current:.1} · {average_label}: {speed:.1} · {}: {streak} · {}: {completed}",
                text(language, TextKey::Streak),
                text(language, TextKey::Progress)
            )
        }
        PracticeMode::Sentence {
            completed,
            last_item,
        } => last_item.as_ref().map_or_else(
            || format!("{}: {completed}", text(language, TextKey::Progress)),
            |delta| {
                format!(
                    "{}: {completed} · {}: {:.1} · {}: {:.1}%",
                    text(language, TextKey::Progress),
                    text(language, TextKey::Speed),
                    delta.speed,
                    text(language, TextKey::Accuracy),
                    delta.accuracy
                )
            },
        ),
        PracticeMode::Test { .. } => match active.stop {
            StopRule::ActiveTime(limit) => format!(
                "{}: {}s · {}: {}%",
                text(language, TextKey::Remaining),
                limit.saturating_sub(metrics.active).as_secs(),
                text(language, TextKey::Progress),
                (metrics.active.as_secs_f64() / limit.as_secs_f64() * 100.0).clamp(0.0, 100.0)
                    as usize,
            ),
            StopRule::TargetEnd | StopRule::Items(_) => String::new(),
        },
        PracticeMode::Key { .. } | PracticeMode::Long { .. } => String::new(),
    };
    vec![Line::from(summary), Line::from(detail)]
}

fn key_progress(active: &ActivePractice) -> (usize, usize) {
    let mut streak = 0_usize;
    for (_, entered) in active.engine.target_cells().take(active.engine.cursor()) {
        if entered == Some(true) {
            streak = streak.saturating_add(1);
        } else {
            streak = 0;
        }
    }
    (active.engine.cursor(), streak)
}

const KEYBOARD_ROWS: [&[char]; 4] = [
    &[
        '`', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=',
    ],
    &[
        'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\\',
    ],
    &['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', '\''],
    &['z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/'],
];

fn keyboard_lines(active: &ActivePractice, styles: ThemeStyles) -> Vec<Line<'static>> {
    let language = active.engine.language();
    let current = current_physical_key(active);
    KEYBOARD_ROWS
        .iter()
        .enumerate()
        .map(|(row, keys)| {
            let mut spans = Vec::new();
            if row == 3 {
                spans.push(Span::styled(
                    "[Shift] ",
                    if current.is_some_and(|(_, shift)| shift) {
                        styles.cursor
                    } else {
                        styles.dim
                    },
                ));
            }
            for &key in *keys {
                let label = keyboard_label(language, key);
                spans.push(Span::styled(
                    format!("[{label}]"),
                    if current.is_some_and(|(base, _)| base == key) {
                        styles.cursor
                    } else {
                        styles.base
                    },
                ));
                spans.push(Span::raw(" "));
            }
            if row == 3 {
                spans.push(Span::styled(
                    "[Space]",
                    if current.is_some_and(|(base, _)| base == ' ') {
                        styles.cursor
                    } else {
                        styles.base
                    },
                ));
            }
            Line::from(spans)
        })
        .collect()
}

fn finger_line(active: &ActivePractice, language: Language, styles: ThemeStyles) -> Line<'static> {
    let Some((key, _)) = current_physical_key(active) else {
        return Line::default();
    };
    let label = match language {
        Language::Ko => "손가락",
        Language::En => "Finger",
    };
    Line::from(Span::styled(
        format!("{label}: {}", finger_name(language, key)),
        styles.accent,
    ))
}

fn current_physical_key(active: &ActivePractice) -> Option<(char, bool)> {
    let target = active
        .engine
        .target_cells()
        .nth(active.engine.cursor())?
        .0
        .chars()
        .next()?;
    physical_key(active.engine.language(), target)
}

fn physical_key(language: Language, key: char) -> Option<(char, bool)> {
    if language == Language::Ko {
        let korean = match key {
            'ㅂ' => ('q', false),
            'ㅃ' => ('q', true),
            'ㅈ' => ('w', false),
            'ㅉ' => ('w', true),
            'ㄷ' => ('e', false),
            'ㄸ' => ('e', true),
            'ㄱ' => ('r', false),
            'ㄲ' => ('r', true),
            'ㅅ' => ('t', false),
            'ㅆ' => ('t', true),
            'ㅛ' => ('y', false),
            'ㅕ' => ('u', false),
            'ㅑ' => ('i', false),
            'ㅐ' => ('o', false),
            'ㅒ' => ('o', true),
            'ㅔ' => ('p', false),
            'ㅖ' => ('p', true),
            'ㅁ' => ('a', false),
            'ㄴ' => ('s', false),
            'ㅇ' => ('d', false),
            'ㄹ' => ('f', false),
            'ㅎ' => ('g', false),
            'ㅗ' => ('h', false),
            'ㅓ' => ('j', false),
            'ㅏ' => ('k', false),
            'ㅣ' => ('l', false),
            'ㅋ' => ('z', false),
            'ㅌ' => ('x', false),
            'ㅊ' => ('c', false),
            'ㅍ' => ('v', false),
            'ㅠ' => ('b', false),
            'ㅜ' => ('n', false),
            'ㅡ' => ('m', false),
            _ => return english_physical_key(key),
        };
        return Some(korean);
    }
    english_physical_key(key)
}

fn english_physical_key(key: char) -> Option<(char, bool)> {
    Some(match key {
        'A'..='Z' => (key.to_ascii_lowercase(), true),
        '!' => ('1', true),
        '@' => ('2', true),
        '#' => ('3', true),
        '$' => ('4', true),
        '%' => ('5', true),
        '^' => ('6', true),
        '&' => ('7', true),
        '*' => ('8', true),
        '(' => ('9', true),
        ')' => ('0', true),
        '_' => ('-', true),
        '+' => ('=', true),
        '{' => ('[', true),
        '}' => (']', true),
        '|' => ('\\', true),
        '"' => ('\'', true),
        ':' => (';', true),
        '<' => (',', true),
        '>' => ('.', true),
        '?' => ('/', true),
        '~' => ('`', true),
        'a'..='z'
        | '0'..='9'
        | ';'
        | '-'
        | '='
        | '['
        | ']'
        | '\\'
        | '\''
        | ','
        | '.'
        | '/'
        | '`'
        | ' ' => (key, false),
        _ => return None,
    })
}

fn keyboard_label(language: Language, key: char) -> char {
    if language == Language::En {
        return key.to_ascii_uppercase();
    }
    match key {
        'q' => 'ㅂ',
        'w' => 'ㅈ',
        'e' => 'ㄷ',
        'r' => 'ㄱ',
        't' => 'ㅅ',
        'y' => 'ㅛ',
        'u' => 'ㅕ',
        'i' => 'ㅑ',
        'o' => 'ㅐ',
        'p' => 'ㅔ',
        'a' => 'ㅁ',
        's' => 'ㄴ',
        'd' => 'ㅇ',
        'f' => 'ㄹ',
        'g' => 'ㅎ',
        'h' => 'ㅗ',
        'j' => 'ㅓ',
        'k' => 'ㅏ',
        'l' => 'ㅣ',
        'z' => 'ㅋ',
        'x' => 'ㅌ',
        'c' => 'ㅊ',
        'v' => 'ㅍ',
        'b' => 'ㅠ',
        'n' => 'ㅜ',
        'm' => 'ㅡ',
        _ => key,
    }
}

const fn finger_name(language: Language, key: char) -> &'static str {
    match (language, key) {
        (Language::Ko, 'q' | 'a' | 'z' | '1' | '`') => "왼쪽 새끼",
        (Language::Ko, 'w' | 's' | 'x' | '2') => "왼쪽 약지",
        (Language::Ko, 'e' | 'd' | 'c' | '3') => "왼쪽 중지",
        (Language::Ko, 'r' | 'f' | 'v' | 't' | 'g' | 'b' | '4' | '5') => "왼쪽 검지",
        (Language::Ko, 'y' | 'h' | 'n' | 'u' | 'j' | 'm' | '6' | '7') => "오른쪽 검지",
        (Language::Ko, 'i' | 'k' | ',' | '8') => "오른쪽 중지",
        (Language::Ko, 'o' | 'l' | '.' | '9') => "오른쪽 약지",
        (Language::Ko, ' ') => "엄지",
        (Language::Ko, _) => "오른쪽 새끼",
        (Language::En, 'q' | 'a' | 'z' | '1' | '`') => "left pinky",
        (Language::En, 'w' | 's' | 'x' | '2') => "left ring",
        (Language::En, 'e' | 'd' | 'c' | '3') => "left middle",
        (Language::En, 'r' | 'f' | 'v' | 't' | 'g' | 'b' | '4' | '5') => "left index",
        (Language::En, 'y' | 'h' | 'n' | 'u' | 'j' | 'm' | '6' | '7') => "right index",
        (Language::En, 'i' | 'k' | ',' | '8') => "right middle",
        (Language::En, 'o' | 'l' | '.' | '9') => "right ring",
        (Language::En, ' ') => "thumb",
        (Language::En, _) => "right pinky",
    }
}

const fn current_average(language: Language) -> (&'static str, &'static str) {
    match language {
        Language::Ko => ("현재", "평균"),
        Language::En => ("Current", "Average"),
    }
}

const fn leave_confirmation(language: Language) -> &'static str {
    match language {
        Language::Ko => "끝내려면 q를 다시 누르세요",
        Language::En => "Press q again to finish",
    }
}

const fn test_leave_instruction(language: Language) -> &'static str {
    match language {
        Language::Ko => "Esc: 나가기",
        Language::En => "Esc: Leave",
    }
}

const fn test_leave_confirmation(language: Language) -> &'static str {
    match language {
        Language::Ko => "Q: 확인 · Esc: 취소",
        Language::En => "Q: Confirm · Esc: Cancel",
    }
}

fn target_lines<'a>(
    active: &'a ActivePractice,
    width: u16,
    scroll: usize,
    height: u16,
    styles: ThemeStyles,
) -> Vec<Line<'a>> {
    let width = usize::from(width);
    let height = usize::from(height);
    let end_row = scroll.saturating_add(height);
    let cursor = active.engine.cursor();
    let mut lines = Vec::with_capacity(height);
    let mut spans = Vec::new();
    let mut column = 0_usize;
    let mut row = 0_usize;
    for (index, (grapheme, entered)) in active.engine.target_cells().enumerate() {
        if row >= end_row {
            break;
        }
        if grapheme == "\n" {
            if row >= scroll {
                lines.push(Line::from(mem::take(&mut spans)));
            } else {
                spans.clear();
            }
            column = 0;
            row = row.saturating_add(1);
            continue;
        }
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if width != 0 && column != 0 && column.saturating_add(grapheme_width) > width {
            if row >= scroll {
                lines.push(Line::from(mem::take(&mut spans)));
            } else {
                spans.clear();
            }
            column = 0;
            row = row.saturating_add(1);
            if row >= end_row {
                break;
            }
        }
        let style = match entered {
            Some(true) => styles.correct,
            Some(false) => styles.error,
            None if index == cursor => styles.cursor,
            None => styles.dim,
        };
        if row >= scroll {
            spans.push(Span::styled(grapheme, style));
        }
        column = column.saturating_add(grapheme_width);
        if width != 0 && column >= width {
            if row >= scroll {
                lines.push(Line::from(mem::take(&mut spans)));
            } else {
                spans.clear();
            }
            column = 0;
            row = row.saturating_add(1);
        }
    }
    if !spans.is_empty() && row >= scroll && row < end_row {
        lines.push(Line::from(spans));
    }
    lines
}

fn render_result(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = if app.result.is_some() {
        titled(text(language, TextKey::Result), styles).title_bottom(Span::styled(
            result_actions(language, app.can_start_next()),
            styles.accent,
        ))
    } else {
        titled(text(language, TextKey::Result), styles)
    };
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let regions = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(if app.update_notice.is_some() { 3 } else { 0 }),
    ])
    .split(inner);
    let body = regions[0];
    render_update_notice(frame, app, regions[1], styles);
    let Some(result) = app.result.as_ref() else {
        frame.render_widget(no_data(language, styles), body);
        return;
    };
    let session = &result.session;
    let (speed, unit) = match session.language {
        Language::Ko => (session.kpm, "KPM"),
        Language::En => (session.wpm, "WPM"),
    };
    let mut lines = vec![
        Line::from(Span::styled(format!("{speed:.1} {unit}"), styles.accent)),
        Line::from(format!(
            "{}: {:.1}%",
            text(language, TextKey::Accuracy),
            session.accuracy
        )),
        Line::from(format!(
            "{}: {}",
            text(language, TextKey::Errors),
            session.errors
        )),
        Line::from(format!(
            "{}: {} ms",
            text(language, TextKey::Duration),
            session.duration_ms
        )),
    ];
    if let Some(previous) = result.previous_speed {
        lines.push(Line::from(format!(
            "{}: {previous:.1} {unit}",
            text(language, TextKey::Previous)
        )));
    }
    if let Some(best) = result.best_speed {
        lines.push(Line::from(format!(
            "{}: {best:.1} {unit}",
            text(language, TextKey::Best)
        )));
    }
    if let Some(delta) = result.speed_delta {
        lines.push(Line::from(format!("{delta:+.1} {unit}")));
    }
    if let Some(grade) = result.grade {
        lines.push(Line::from(format!(
            "{}: {}",
            match language {
                Language::Ko => "Typeul 상대 등급",
                Language::En => "Typeul relative grade",
            },
            grade_name(grade)
        )));
    }
    if let Some(long) = result.long {
        let (rolling, graphemes) = match language {
            Language::Ko => ("최고 30초 속도", "글자"),
            Language::En => ("Best rolling 30s", "Graphemes"),
        };
        lines.push(Line::from(format!(
            "{rolling}: {:.1} {unit}",
            long.best_rolling_speed
        )));
        lines.push(Line::from(format!(
            "{graphemes}: {}/{}",
            long.completed_graphemes, long.total_graphemes
        )));
        lines.push(Line::from(format!(
            "{}: {}%",
            text(language, TextKey::Progress),
            long.percent
        )));
    }
    for (label, met) in [
        (TextKey::Speed, result.speed_goal_met),
        (TextKey::Accuracy, result.accuracy_goal_met),
        (TextKey::DailyMinutes, result.daily_minutes_met),
    ] {
        lines.push(Line::from(format!(
            "{}: {}",
            text(language, label),
            text(
                language,
                if met {
                    TextKey::GoalMet
                } else {
                    TextKey::GoalMissed
                }
            )
        )));
    }
    if let Some(error) = &result.save_error {
        lines.push(Line::from(Span::styled(
            format!(
                "{}: {}",
                text(language, TextKey::SaveFailed),
                terminal_safe(error)
            ),
            styles.error,
        )));
    }
    if !result.weak_keys.is_empty() && lines.len() < usize::from(body.height) {
        lines.push(Line::from(Span::styled(
            text(language, TextKey::WeakKeys),
            styles.accent,
        )));
        lines.extend(
            result
                .weak_keys
                .iter()
                .take(usize::from(body.height).saturating_sub(lines.len()))
                .map(|key| Line::from(format!("{}: {:.1}%", key.key, key.accuracy))),
        );
    }
    frame.render_widget(Paragraph::new(lines).style(styles.base), body);
}

fn render_update_notice(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let Some(notice) = &app.update_notice else {
        return;
    };
    let language = app.settings.ui_language;
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!(
                    "{}: {} → {}",
                    text(language, TextKey::UpdateAvailable),
                    notice.current,
                    notice.latest
                ),
                styles.accent,
            )),
            Line::from(format!(
                "{}: {}",
                text(language, TextKey::UpdateCommand),
                notice.instructions()
            )),
            Line::from(format!(
                "l: {} · s: {}",
                text(language, TextKey::UpdateLater),
                text(language, TextKey::UpdateSkip)
            )),
        ])
        .style(styles.base),
        area,
    );
}

const fn grade_name(grade: Grade) -> &'static str {
    match grade {
        Grade::A => "A",
        Grade::B => "B",
        Grade::C => "C",
        Grade::D => "D",
    }
}

fn filter_lines(app: &App, language: Language, navigation: bool) -> Vec<Line<'static>> {
    let all = match language {
        Language::Ko => "전체",
        Language::En => "All",
    };
    let range = [
        (crate::stats::Range::Days7, "7"),
        (crate::stats::Range::Days30, "30"),
        (crate::stats::Range::Days90, "90"),
        (crate::stats::Range::All, all),
    ]
    .into_iter()
    .map(|(range, label)| {
        if app.stats_range() == range {
            format!("[{label}]")
        } else {
            label.to_owned()
        }
    })
    .collect::<Vec<_>>()
    .join("  ");
    let (range_label, language_label, mode_label) = match language {
        Language::Ko => ("범위", "언어", "모드"),
        Language::En => ("Range", "Language", "Mode"),
    };
    let mode = app
        .stats_mode()
        .map_or(all, |mode| practice_name(language, mode));
    let mut lines = vec![
        Line::from(format!("{}{range_label}: {range}", focus_marker(app, 0))),
        Line::from(format!(
            "{}{language_label}: {}",
            focus_marker(app, 1),
            language_name(app.stats_language())
        )),
        Line::from(format!("{}{mode_label}: {mode}", focus_marker(app, 2))),
    ];
    if navigation {
        lines.push(Line::from(format!(
            "{}{}",
            focus_marker(app, 3),
            text(language, TextKey::History)
        )));
        lines.push(Line::from(format!(
            "{}{}",
            focus_marker(app, 4),
            text(language, TextKey::WeakKeys)
        )));
    }
    lines
}

fn focus_marker(app: &App, index: usize) -> &'static str {
    if app.focus() == index { "> " } else { "  " }
}

fn finite_nonnegative(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn duration_name(language: Language, duration: Duration) -> String {
    let minutes = duration.as_secs() / 60;
    let hours = minutes / 60;
    let minutes = minutes % 60;
    match (language, hours) {
        (Language::Ko, 0) => format!("{minutes}분"),
        (Language::Ko, _) => format!("{hours}시간 {minutes}분"),
        (Language::En, 0) => format!("{minutes} min"),
        (Language::En, _) => format!("{hours}h {minutes}m"),
    }
}

fn render_trend(
    frame: &mut Frame<'_>,
    label: &str,
    values: &[u64],
    area: Rect,
    styles: ThemeStyles,
) {
    let regions = Layout::horizontal([Constraint::Length(18), Constraint::Min(1)]).split(area);
    frame.render_widget(Paragraph::new(label).style(styles.dim), regions[0]);
    frame.render_widget(
        Sparkline::default().data(values).style(styles.accent),
        regions[1],
    );
}

fn render_stats(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::HomeStats), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(filter_lines(app, language, true)).style(styles.accent),
        Rect::new(inner.x, inner.y, inner.width, 5.min(inner.height)),
    );
    let data = Rect::new(
        inner.x,
        inner.y.saturating_add(5),
        inner.width,
        inner.height.saturating_sub(5),
    );
    let today = app.stats_today();
    let selected = history(
        &app.sessions,
        app.stats_range(),
        today,
        Some(app.stats_language()),
        app.stats_mode(),
    );
    if selected.is_empty() {
        frame.render_widget(no_data(language, styles), data);
        return;
    }

    let overview = summarize(selected.iter().copied());
    let accuracy = if overview.accuracy.is_finite() {
        overview.accuracy.clamp(0.0, 100.0)
    } else {
        0.0
    };
    let regions = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(4),
    ])
    .split(data);
    let (sessions_label, total_label, goal_label) = match language {
        Language::Ko => ("세션", "총 시간", "목표"),
        Language::En => ("Sessions", "Total time", "Goal"),
    };
    let (unit, average, best, speed_goal) = match app.stats_language() {
        Language::Ko => (
            "KPM",
            overview.korean.average,
            overview.korean.best,
            app.settings.target_kpm,
        ),
        Language::En => (
            "WPM",
            overview.english.average,
            overview.english.best,
            app.settings.target_wpm,
        ),
    };
    let average = finite_nonnegative(average);
    let best = finite_nonnegative(best);
    let practice_streak = streak(app.sessions.iter().map(|session| session.local_date), today);
    let points = app.stats_points();
    let minutes = app
        .sessions
        .iter()
        .filter(|session| session.local_date == today)
        .fold(0_u64, |total, session| {
            total.saturating_add(session.duration_ms)
        })
        / 60_000;
    let minute_unit = match language {
        Language::Ko => "분",
        Language::En => "min",
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!("{sessions_label}: {}", overview.sessions)),
            Line::from(format!(
                "{total_label}: {}",
                duration_name(language, overview.total)
            )),
            Line::from(format!(
                "{}: {:.1}%",
                text(language, TextKey::Accuracy),
                accuracy
            )),
            Line::from(format!("{unit} {average:.1}/{best:.1}")),
            Line::from(format!(
                "{}: {practice_streak}",
                text(language, TextKey::Streak)
            )),
            Line::from(format!(
                "{goal_label}: {unit} {average:.0}/{speed_goal} · {:.1}/{:.1}% · {minutes}/{} {minute_unit}",
                accuracy, app.settings.target_accuracy, app.settings.daily_minutes,
            )),
        ])
        .style(styles.base),
        regions[0],
    );
    let speed_values = points
        .iter()
        .map(|point| {
            let speed = point.speed;
            if speed.is_finite() {
                speed.max(0.0)
            } else {
                0.0
            }
        })
        .collect::<Vec<_>>();
    let accuracy_values = points
        .iter()
        .map(|point| finite_nonnegative(point.accuracy).min(100.0) as u64)
        .collect::<Vec<_>>();
    let minute_values = points
        .iter()
        .map(|point| finite_nonnegative(point.minutes * 60.0).min(u64::MAX as f64) as u64)
        .collect::<Vec<_>>();
    render_trend(
        frame,
        match language {
            Language::Ko => "정확도 추이",
            Language::En => "Accuracy trend",
        },
        &accuracy_values,
        regions[1],
        styles,
    );
    render_trend(
        frame,
        match language {
            Language::Ko => "시간 추이",
            Language::En => "Minutes trend",
        },
        &minute_values,
        regions[2],
        styles,
    );
    let points = speed_values
        .iter()
        .enumerate()
        .map(|(index, &speed)| (index as f64, speed))
        .collect::<Vec<_>>();
    let y_max = speed_values.iter().copied().fold(1.0_f64, f64::max);
    let x_max = points.len().saturating_sub(1).max(1) as f64;
    let title = match language {
        Language::Ko => "속도 추이",
        Language::En => "Speed trend",
    };
    frame.render_widget(
        Chart::new(vec![
            Dataset::default()
                .data(&points)
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(styles.accent),
        ])
        .block(titled(title, styles))
        .x_axis(Axis::default().bounds([0.0, x_max]))
        .y_axis(Axis::default().bounds([0.0, y_max])),
        regions[3],
    );
}

fn render_history(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::History), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(filter_lines(app, language, false)).style(styles.accent),
        Rect::new(inner.x, inner.y, inner.width, 3.min(inner.height)),
    );
    let data = Rect::new(
        inner.x,
        inner.y.saturating_add(3),
        inner.width,
        inner.height.saturating_sub(3),
    );
    let today = app.stats_today();
    let items = history(
        &app.sessions,
        app.stats_range(),
        today,
        Some(app.stats_language()),
        app.stats_mode(),
    )
    .into_iter();
    let mut items = items.peekable();
    if items.peek().is_none() {
        frame.render_widget(no_data(language, styles), data);
        return;
    }
    let items = items.map(|session| {
        let (speed, unit) = match session.language {
            Language::Ko => (session.kpm, "KPM"),
            Language::En => (session.wpm, "WPM"),
        };
        ListItem::new(format!(
            "{} {} {} {} {:.1} {unit} {:.1}%",
            session.id,
            session.local_date,
            practice_name(language, session.mode),
            language_name(session.language),
            finite_nonnegative(speed),
            session.accuracy
        ))
    });
    frame.render_widget(List::new(items).style(styles.base), data);
}

fn render_weak_keys(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::WeakKeys), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let counts = intended_key_counts(&app.sessions, app.stats_language());
    let keys = weak_keys(&counts, 10);
    if keys.is_empty() {
        frame.render_widget(no_data(language, styles), inner);
        return;
    }
    let suggestions = adaptive_candidates(&app.content, &app.sessions, app.stats_language(), 0)
        .into_iter()
        .take(3)
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    let key_limit = usize::from(inner.height).saturating_sub(usize::from(!suggestions.is_empty()));
    let mut rows = keys
        .into_iter()
        .take(key_limit)
        .map(|key| {
            ListItem::new(format!(
                "{}: {:.1}% ({})",
                key.key,
                key.accuracy,
                key.correct.saturating_add(key.errors)
            ))
        })
        .collect::<Vec<_>>();
    if !suggestions.is_empty() {
        rows.push(ListItem::new(format!(
            "{}: {}",
            match language {
                Language::Ko => "추천 콘텐츠",
                Language::En => "Suggested content",
            },
            suggestions.join(", ")
        )));
    }
    frame.render_widget(List::new(rows).style(styles.base), inner);
}

fn render_goals(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::HomeGoals), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let daily_minutes = match language {
        Language::Ko => format!("{}분", app.settings.daily_minutes),
        Language::En => format!("{} min", app.settings.daily_minutes),
    };
    let (kpm, wpm, accuracy, daily, edit) = match language {
        Language::Ko => (
            "한국어 목표",
            "영어 목표",
            "정확도 목표",
            "하루 연습",
            "←/→ 편집",
        ),
        Language::En => (
            "Korean target",
            "English target",
            "Accuracy target",
            "Daily practice",
            "←/→ edit",
        ),
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "{}{kpm}: {} KPM",
                focus_marker(app, 0),
                app.settings.target_kpm
            )),
            Line::from(format!(
                "{}{wpm}: {} WPM",
                focus_marker(app, 1),
                app.settings.target_wpm
            )),
            Line::from(format!(
                "{}{accuracy}: {:.1}%",
                focus_marker(app, 2),
                app.settings.target_accuracy
            )),
            Line::from(format!("{}{daily}: {daily_minutes}", focus_marker(app, 3))),
            Line::from(edit),
        ])
        .style(styles.base),
        inner,
    );
}

fn render_content(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::HomeContent), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let packs = app.content_packs();
    if packs.is_empty() {
        frame.render_widget(no_data(language, styles), inner);
        return;
    }
    let (enabled, disabled, built_in, user) = match language {
        Language::Ko => ("활성", "비활성", "내장", "사용자"),
        Language::En => ("enabled", "disabled", "built-in", "user"),
    };
    let visible = usize::from(inner.height / 2).max(1);
    let start = app.focus().saturating_sub(visible - 1);
    frame.render_widget(
        List::new(
            packs
                .iter()
                .enumerate()
                .skip(start)
                .take(visible)
                .map(|(index, pack)| {
                    let mut kinds = Vec::new();
                    for &kind in &pack.kinds {
                        let name = content_kind_name(language, kind);
                        if !kinds.contains(&name) {
                            kinds.push(name);
                        }
                    }
                    let kinds = kinds.join(",");
                    ListItem::new(vec![
                        Line::from(format!(
                            "{}{} · {} · {} · {} · {}",
                            if app.focus() == index { "> " } else { "  " },
                            pack.id,
                            if pack.enabled { enabled } else { disabled },
                            if pack.built_in { built_in } else { user },
                            language_name(pack.language),
                            pack.items,
                        )),
                        Line::from(format!(
                            "    {} · {} · {}",
                            pack.licenses.join(","),
                            kinds,
                            pack.sample_item_id,
                        )),
                    ])
                }),
        )
        .style(styles.base),
        inner,
    );
}

fn render_content_detail(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::Sources), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(summary) = app.content_detail_pack() else {
        frame.render_widget(no_data(language, styles), inner);
        return;
    };
    let pack_id_value = &summary.id;
    let (
        item_id,
        pack_id,
        author,
        source_id,
        source_url,
        license,
        license_url,
        retrieved,
        modified,
        yes,
        no,
    ) = match language {
        Language::Ko => (
            "항목 ID",
            "팩 ID",
            "저자",
            "출처 ID",
            "출처 URL",
            "라이선스",
            "라이선스 URL",
            "확인일",
            "수정됨",
            "예",
            "아니요",
        ),
        Language::En => (
            "item ID",
            "pack ID",
            "author",
            "source ID",
            "source URL",
            "license",
            "license URL",
            "retrieved",
            "modified",
            "yes",
            "no",
        ),
    };
    let mut lines = vec![Line::from(format!(
        "{pack_id}: {pack_id_value} · {}: {} · {}: {} · {}",
        match language {
            Language::Ko => "언어",
            Language::En => "language",
        },
        language_name(summary.language),
        match language {
            Language::Ko => "항목 수",
            Language::En => "items",
        },
        summary.items,
        match (language, summary.enabled) {
            (Language::Ko, true) => "활성",
            (Language::Ko, false) => "비활성",
            (Language::En, true) => "enabled",
            (Language::En, false) => "disabled",
        }
    ))];
    if let Some(provenance) = summary.provenance.get(app.focus()) {
        let scope = if let Some(sample_item_id) = &provenance.item_id {
            format!("{item_id}: {sample_item_id}")
        } else {
            match language {
                Language::Ko => "범위: 팩",
                Language::En => "scope: pack",
            }
            .to_owned()
        };
        lines.push(Line::from(format!(
            "{} {}/{} · {scope} · ↑/↓",
            match language {
                Language::Ko => "출처",
                Language::En => "Provenance",
            },
            app.focus() + 1,
            summary.provenance.len()
        )));
        let source = &provenance.source;
        lines.extend([
            Line::from(format!("{author}: {}", source.author)),
            Line::from(format!("{source_id}: {}", source.source_id)),
            Line::from(format!("{source_url}: {}", source.source_url)),
            Line::from(format!("{license}: {}", source.license)),
            Line::from(format!("{license_url}: {}", source.license_url)),
            Line::from(format!(
                "{retrieved}: {} · {modified}: {}",
                source.retrieved_at,
                if source.modified { yes } else { no }
            )),
        ]);
    }
    lines.extend([
        Line::from("typeul content add PACK.toml · typeul content validate PACK.toml"),
        Line::from("typeul content disable PACK_ID · typeul licenses"),
    ]);
    lines.push(Line::from(if summary.built_in {
        match language {
            Language::Ko => "내장 팩은 비활성화할 수 없습니다",
            Language::En => "Built-in packs cannot be disabled",
        }
    } else if !summary.enabled {
        match language {
            Language::Ko => "사용자 팩이 비활성화됨",
            Language::En => "User pack is disabled",
        }
    } else if app.content_disable_confirmation() {
        match language {
            Language::Ko => "확인하려면 d를 다시 누르세요",
            Language::En => "Press d again to confirm",
        }
    } else {
        match language {
            Language::Ko => "d: 비활성화",
            Language::En => "d: Disable",
        }
    }));
    frame.render_widget(
        Paragraph::new(lines)
            .style(styles.base)
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_settings(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::HomeSettings), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let (
        practice_language,
        ui_language,
        keyboard,
        finger_guide,
        live_speed,
        accuracy,
        adaptive,
        updates,
    ) = match language {
        Language::Ko => (
            "연습 언어",
            "화면 언어",
            "키보드",
            "손가락 안내",
            "실시간 속도",
            "정확도 표시",
            "적응형",
            "업데이트 확인",
        ),
        Language::En => (
            "Language",
            "UI language",
            "keyboard",
            "finger guide",
            "live speed",
            "accuracy",
            "adaptive",
            "updates",
        ),
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "{}{practice_language}: {}",
                focus_marker(app, 0),
                language_name(app.settings.language)
            )),
            Line::from(format!(
                "{}{ui_language}: {}",
                focus_marker(app, 1),
                language_name(app.settings.ui_language)
            )),
            Line::from(format!(
                "{}{}: {}",
                focus_marker(app, 2),
                text(language, TextKey::Theme),
                app.settings.theme
            )),
            Line::from(format!(
                "{}{keyboard}: {}",
                focus_marker(app, 3),
                toggle_name(language, app.settings.show_keyboard)
            )),
            Line::from(format!(
                "{}{finger_guide}: {}",
                focus_marker(app, 4),
                toggle_name(language, app.settings.show_finger_guide)
            )),
            Line::from(format!(
                "{}{live_speed}: {}",
                focus_marker(app, 5),
                toggle_name(language, app.settings.show_live_speed)
            )),
            Line::from(format!(
                "{}{accuracy}: {}",
                focus_marker(app, 6),
                toggle_name(language, app.settings.show_accuracy)
            )),
            Line::from(format!(
                "{}{adaptive}: {}",
                focus_marker(app, 7),
                toggle_name(language, app.settings.adaptive)
            )),
            Line::from(format!(
                "{}{updates}: {}",
                focus_marker(app, 8),
                toggle_name(language, app.settings.check_updates)
            )),
        ])
        .style(styles.base),
        inner,
    );
}

const fn language_name(language: Language) -> &'static str {
    match language {
        Language::Ko => "ko",
        Language::En => "en",
    }
}

const fn toggle_name(language: Language, enabled: bool) -> &'static str {
    match (language, enabled) {
        (Language::Ko, true) => "켜짐",
        (Language::Ko, false) => "꺼짐",
        (Language::En, true) => "true",
        (Language::En, false) => "false",
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

fn practice_name(language: Language, kind: PracticeKind) -> &'static str {
    text(
        language,
        match kind {
            PracticeKind::Quick => TextKey::HomeQuick,
            PracticeKind::Key => TextKey::HomeKeys,
            PracticeKind::Words => TextKey::HomeWords,
            PracticeKind::Sentence => TextKey::HomeSentence,
            PracticeKind::Long => TextKey::HomeLong,
            PracticeKind::Test => TextKey::HomeTest,
        },
    )
}

fn content_kind_name(language: Language, kind: ContentKind) -> &'static str {
    text(
        language,
        match kind {
            ContentKind::Word => TextKey::HomeWords,
            ContentKind::Sentence | ContentKind::Quote => TextKey::HomeSentence,
            ContentKind::Text => TextKey::HomeLong,
        },
    )
}

fn render_themes(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::Theme), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let visible = usize::from(inner.height).max(1);
    let start = app.focus().saturating_sub(visible - 1);
    frame.render_widget(
        List::new(
            app.themes
                .ids()
                .enumerate()
                .skip(start)
                .take(visible)
                .map(|(index, id)| {
                    let marker = if app.focus() == index { "> " } else { "  " };
                    let selected = if id == app.settings.theme { "*" } else { " " };
                    ListItem::new(format!("{marker}{selected} {id}"))
                }),
        )
        .style(styles.base),
        inner,
    );
}

fn render_help(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::Help), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines = match language {
        Language::Ko => vec![
            Line::from("이동: Tab / Shift+Tab / ↑↓ / j / k"),
            Line::from("선택/편집: Enter / ←→"),
            Line::from("뒤로: Esc · 종료: q / Ctrl+C · 도움말: ?"),
            Line::from("연습: Esc / Ctrl+P 일시 정지 · 결과: r 다시 연습"),
            Line::from("일시 정지 중 나가기: q를 두 번 누르기"),
            Line::from("업데이트 알림: l 나중에 · s 이번 버전 건너뛰기"),
            Line::from("콘텐츠 비활성화: 상세 화면에서 d 두 번"),
        ],
        Language::En => vec![
            Line::from("Move: Tab / Shift+Tab / ↑↓ / j / k"),
            Line::from("Select/Edit: Enter / ←→"),
            Line::from("Back: Esc · Quit: q / Ctrl+C · Help: ?"),
            Line::from("Practice: Esc / Ctrl+P pause · Result: r retry"),
            Line::from("Leave while paused: press q twice"),
            Line::from("Update notice: l later · s skip this version"),
            Line::from("Content Disable: press d twice in detail"),
        ],
    };
    lines.extend([
        Line::from(""),
        Line::from("typeul quick|keys|words|sentence|long|test"),
        Line::from("typeul stats|history|themes"),
        Line::from("typeul content list"),
        Line::from("typeul content add PACK.toml"),
        Line::from("typeul content validate [PACK.toml]"),
        Line::from("typeul content disable PACK_ID"),
        Line::from("typeul paths|licenses|update"),
        Line::from("typeul --help|--version|--smoke"),
        Line::from("typeul FILE | typeul practice FILE | cat FILE | typeul"),
    ]);
    frame.render_widget(Paragraph::new(lines).style(styles.base), inner);
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

use super::{
    format::speed_values, no_data, practice_cursor, practice_scroll, theme::ThemeStyles, titled,
};
use crate::{
    app::{ActivePractice, App, CustomTextSource, PracticeMode, StopRule, key_stages},
    diagnostic::terminal_safe,
    i18n::{TextKey, text},
    model::{Language, PracticeKind},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span, Text},
    widgets::{Gauge, Paragraph},
};
use std::mem;
use unicode_width::UnicodeWidthStr;

pub(super) fn render_practice(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let Some(active) = app.active_practice() else {
        let block = titled(text(language, TextKey::Progress), styles);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(no_data(language, styles), inner);
        return;
    };
    let practice = match active.engine.language() {
        Language::Ko => "KO",
        Language::En => "EN",
    };
    let observed = match (language, active.observed_input_language()) {
        (_, None) => "—",
        (Language::En, Some(Language::Ko)) => "KO",
        (Language::En, Some(Language::En)) => "EN",
        (Language::Ko, Some(Language::Ko)) => "한글",
        (Language::Ko, Some(Language::En)) => "영문",
    };
    let mismatch = active
        .observed_input_language()
        .is_some_and(|observed| observed != active.engine.language());
    let status = match language {
        Language::Ko => format!(
            "연습 {practice} · 입력 {observed}{}",
            if mismatch { " ⚠" } else { "" }
        ),
        Language::En => format!(
            "Practice {practice} · Input {observed}{}",
            if mismatch { " ⚠" } else { "" }
        ),
    };
    let block = titled(text(language, TextKey::Progress), styles).title_bottom(Span::styled(
        status,
        if mismatch {
            styles.error
        } else {
            styles.accent
        },
    ));
    let inner = block.inner(area);
    frame.render_widget(block, area);
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
    let input_cursor_area = if key_mode {
        let scroll = practice_scroll(regions[0], active);
        let target = target_lines(active, regions[0].width, scroll, regions[0].height, styles);
        frame.render_widget(
            Paragraph::new(Text::from(target)).style(styles.base),
            regions[0],
        );
        None
    } else {
        let typing =
            Layout::vertical([Constraint::Length(3), Constraint::Min(3)]).split(regions[0]);
        let input_block = titled(text(language, TextKey::Input), styles);
        let input_inner = input_block.inner(typing[0]);
        frame.render_widget(input_block, typing[0]);
        frame.render_widget(
            Paragraph::new(input_line(active, styles)).style(styles.base),
            input_inner,
        );
        let prompt_block = titled(text(language, TextKey::Prompt), styles);
        let prompt_inner = prompt_block.inner(typing[1]);
        frame.render_widget(prompt_block, typing[1]);
        frame.render_widget(
            Paragraph::new(prompt_lines(active, styles)).style(styles.base),
            prompt_inner,
        );
        Some(input_inner)
    };
    if let Some(progress) = active.long_scroll() {
        let live =
            Layout::vertical([Constraint::Length(4), Constraint::Length(1)]).split(regions[1]);
        frame.render_widget(
            Paragraph::new(practice_live_lines(
                active,
                language,
                app.settings.show_live_speed,
                app.settings.show_accuracy,
            ))
            .style(styles.base),
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
            Paragraph::new(practice_live_lines(
                active,
                language,
                app.settings.show_live_speed,
                app.settings.show_accuracy,
            ))
            .style(styles.base),
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
    if !active.engine.is_paused() {
        let cursor = if let Some(area) = input_cursor_area {
            input_cursor(area, active)
        } else {
            practice_cursor(regions[0], active)
        };
        if let Some(cursor) = cursor {
            frame.set_cursor_position(cursor);
        }
    }
}

fn practice_live_lines(
    active: &ActivePractice,
    language: Language,
    show_speed: bool,
    show_accuracy: bool,
) -> Vec<Line<'static>> {
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
        let mut status = vec![format!("{stage_label} {stage}: {title}")];
        if show_accuracy {
            status.push(format!(
                "{}: {:.1}%",
                text(language, TextKey::Accuracy),
                metrics.accuracy
            ));
        }
        status.push(format!("{}: {streak}", text(language, TextKey::Streak)));
        return vec![
            Line::from(status.join(" · ")),
            Line::from(format!(
                "{}: {completed}/{} · {}: {}",
                text(language, TextKey::Progress),
                active.engine.target_len(),
                text(language, TextKey::Errors),
                metrics.errors,
            )),
        ];
    }
    let mut summary = Vec::new();
    if show_speed {
        summary.push(format!(
            "{}: {}",
            text(language, TextKey::Speed),
            speed_values(language, metrics.kpm, metrics.wpm)
        ));
    }
    if show_accuracy {
        summary.push(format!(
            "{}: {:.1}%",
            text(language, TextKey::Accuracy),
            metrics.accuracy
        ));
    }
    summary.push(format!(
        "{}: {}",
        text(language, TextKey::Errors),
        metrics.errors
    ));
    let summary = summary.join(" · ");
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
                StopRule::TargetEnd | StopRule::TargetOrActiveTime(_) => "0".into(),
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
            let (current_kpm, current_wpm) = active
                .current_item_delta()
                .map_or((0.0, 0.0), |delta| (delta.kpm, delta.wpm));
            let (current_label, average_label) = current_average(language);
            let mut fields = Vec::new();
            if show_speed {
                fields.extend([
                    format!(
                        "{current_label}: {}",
                        speed_values(language, current_kpm, current_wpm)
                    ),
                    format!(
                        "{average_label}: {}",
                        speed_values(language, metrics.kpm, metrics.wpm)
                    ),
                ]);
            }
            fields.extend([
                format!("{}: {streak}", text(language, TextKey::Streak)),
                format!("{}: {completed}", text(language, TextKey::Progress)),
            ]);
            fields.join(" · ")
        }
        PracticeMode::Sentence {
            completed,
            last_item,
        } => {
            let mut fields = vec![format!(
                "{}: {completed}",
                text(language, TextKey::Progress)
            )];
            if let Some(delta) = last_item {
                if show_speed {
                    fields.push(format!(
                        "{}: {}",
                        text(language, TextKey::Speed),
                        speed_values(language, delta.kpm, delta.wpm)
                    ));
                }
                if show_accuracy {
                    fields.push(format!(
                        "{}: {:.1}%",
                        text(language, TextKey::Accuracy),
                        delta.accuracy
                    ));
                }
            }
            fields.join(" · ")
        }
        PracticeMode::Test { .. } => match active.stop {
            StopRule::ActiveTime(limit) | StopRule::TargetOrActiveTime(limit) => format!(
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

fn input_line<'a>(active: &'a ActivePractice, styles: ThemeStyles) -> Line<'a> {
    let Some(range) = active.engine.current_line_range() else {
        return Line::default();
    };
    let spans = active
        .engine
        .input_cells()
        .skip(range.start)
        .take(range.len())
        .filter_map(|(_, entered, correct)| {
            let correct = correct?;
            let symbol = entered.map_or("·", |entered| if entered == "\n" { "↵" } else { entered });
            Some(Span::styled(
                symbol,
                if correct {
                    styles.correct
                } else {
                    styles.error
                },
            ))
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

fn prompt_lines<'a>(active: &'a ActivePractice, styles: ThemeStyles) -> Vec<Line<'a>> {
    let ranges = active.engine.line_ranges().collect::<Vec<_>>();
    if ranges.is_empty() {
        return Vec::new();
    }
    let current = active.engine.current_line_index().min(ranges.len() - 1);
    let start = current.saturating_sub(1);
    let end = current.saturating_add(2).min(ranges.len());
    let cells = active.engine.target_cells().collect::<Vec<_>>();
    let cursor = active.engine.cursor();
    ranges[start..end]
        .iter()
        .map(|range| {
            Line::from(
                range
                    .clone()
                    .map(|index| {
                        let (grapheme, entered) = cells[index];
                        Span::styled(
                            if grapheme == "\n" { "↵" } else { grapheme },
                            match entered {
                                Some(true) => styles.correct,
                                Some(false) => styles.error,
                                None if index == cursor => styles.cursor,
                                None => styles.dim,
                            },
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn input_cursor(area: Rect, active: &ActivePractice) -> Option<(u16, u16)> {
    if area.width == 0 || area.height == 0 {
        return None;
    }
    let range = active.engine.current_line_range()?;
    let entered = active.engine.cursor().saturating_sub(range.start);
    let width = active
        .engine
        .input_cells()
        .skip(range.start)
        .take(entered.min(range.len()))
        .fold(0_usize, |width, (_, entered, correct)| {
            if correct.is_none() || entered.is_none_or(|entered| entered == "\n") {
                width.saturating_add(1)
            } else {
                width.saturating_add(UnicodeWidthStr::width(entered.unwrap_or_default()))
            }
        })
        .min(usize::from(area.width - 1)) as u16;
    Some((area.x.saturating_add(width), area.y))
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

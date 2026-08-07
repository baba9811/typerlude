use crate::{
    app::{ActivePractice, App, Grade, Screen},
    cli::terminal_safe,
    content::ContentKind,
    i18n::{TextKey, text},
    model::{Language, PracticeKind},
    stats::{Range, history, intended_key_counts, progress, summarize, weak_keys},
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
use std::mem;
use unicode_width::UnicodeWidthStr;

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
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

    let width = usize::from(area.width);
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

    let column = column.min(usize::from(area.width - 1)) as u16;
    let row = row.min(usize::from(area.height - 1)) as u16;
    Some((area.x.saturating_add(column), area.y.saturating_add(row)))
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
    frame.render_widget(List::new(items).style(styles.base), inner);
}

fn render_mode_select(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::HomeQuick), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let regions = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(inner);
    frame.render_widget(
        List::new(
            [
                TextKey::HomeQuick,
                TextKey::HomeKeys,
                TextKey::HomeWords,
                TextKey::HomeSentence,
                TextKey::HomeLong,
                TextKey::HomeTest,
            ]
            .into_iter()
            .map(|key| ListItem::new(text(language, key))),
        )
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

fn render_practice(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::Progress), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(active) = app.active_practice() else {
        frame.render_widget(no_data(language, styles), inner);
        return;
    };
    frame.render_widget(
        Paragraph::new(Text::from(target_lines(active, inner.width, styles))).style(styles.base),
        inner,
    );
    if let Some(cursor) = practice_cursor(inner, active) {
        frame.set_cursor_position(cursor);
    }
}

fn target_lines<'a>(active: &'a ActivePractice, width: u16, styles: ThemeStyles) -> Vec<Line<'a>> {
    let width = usize::from(width);
    let cursor = active.engine.cursor();
    let mut lines = Vec::new();
    let mut spans = Vec::new();
    let mut column = 0_usize;
    for (index, (grapheme, entered)) in active.engine.target_cells().enumerate() {
        if grapheme == "\n" {
            lines.push(Line::from(mem::take(&mut spans)));
            column = 0;
            continue;
        }
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if width != 0 && column != 0 && column.saturating_add(grapheme_width) > width {
            lines.push(Line::from(mem::take(&mut spans)));
            column = 0;
        }
        let style = match entered {
            Some(true) => styles.correct,
            Some(false) => styles.error,
            None if index == cursor => styles.cursor,
            None => styles.dim,
        };
        spans.push(Span::styled(grapheme, style));
        column = column.saturating_add(grapheme_width);
        if width != 0 && column >= width {
            lines.push(Line::from(mem::take(&mut spans)));
            column = 0;
        }
    }
    if !spans.is_empty() || lines.is_empty() {
        lines.push(Line::from(spans));
    }
    lines
}

fn render_result(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::Result), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(result) = app.result.as_ref() else {
        frame.render_widget(no_data(language, styles), inner);
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
            text(language, TextKey::TestGrade),
            grade_name(grade)
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
    if !result.weak_keys.is_empty() && lines.len() < usize::from(inner.height) {
        lines.push(Line::from(Span::styled(
            text(language, TextKey::WeakKeys),
            styles.accent,
        )));
        lines.extend(
            result
                .weak_keys
                .iter()
                .take(usize::from(inner.height).saturating_sub(lines.len()))
                .map(|key| Line::from(format!("{}: {:.1}%", key.key, key.accuracy))),
        );
    }
    frame.render_widget(Paragraph::new(lines).style(styles.base), inner);
}

const fn grade_name(grade: Grade) -> &'static str {
    match grade {
        Grade::A => "A",
        Grade::B => "B",
        Grade::C => "C",
        Grade::D => "D",
    }
}

fn render_stats(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::HomeStats), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let ranges = Rect::new(inner.x, inner.y, inner.width, 1.min(inner.height));
    let all = match language {
        Language::Ko => "전체",
        Language::En => "All",
    };
    frame.render_widget(
        Paragraph::new(format!("7  [30]  90  {all}")).style(styles.accent),
        ranges,
    );
    let data = Rect::new(
        inner.x,
        inner.y.saturating_add(1),
        inner.width,
        inner.height.saturating_sub(1),
    );
    let Some(today) = app.sessions.iter().map(|session| session.local_date).max() else {
        frame.render_widget(no_data(language, styles), data);
        return;
    };
    let selected = history(
        &app.sessions,
        Range::Days30,
        today,
        Some(app.settings.language),
        None,
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
        Constraint::Length(5),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(5),
    ])
    .split(data);
    let (sessions_label, total_label) = match language {
        Language::Ko => ("세션", "총 시간"),
        Language::En => ("Sessions", "Total time"),
    };
    let (unit, average, best) = match app.settings.language {
        Language::Ko => ("KPM", overview.korean.average, overview.korean.best),
        Language::En => ("WPM", overview.english.average, overview.english.best),
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!("{sessions_label}: {}", overview.sessions)),
            Line::from(format!("{total_label}: {} ms", overview.total.as_millis())),
            Line::from(format!(
                "{}: {:.1}%",
                text(language, TextKey::Accuracy),
                accuracy
            )),
            Line::from(format!("{unit} {average:.1}/{best:.1}")),
        ])
        .style(styles.base),
        regions[0],
    );
    frame.render_widget(
        Gauge::default()
            .label(format!(
                "{} {:.1}%",
                text(language, TextKey::Accuracy),
                accuracy
            ))
            .ratio(accuracy / 100.0)
            .gauge_style(styles.accent),
        regions[1],
    );
    let speed_values = progress(
        &app.sessions,
        Range::Days30,
        today,
        app.settings.language,
        None,
    )
    .into_iter()
    .map(|point| {
        let speed = point.speed;
        if speed.is_finite() {
            speed.max(0.0)
        } else {
            0.0
        }
    })
    .collect::<Vec<_>>();
    let speeds = speed_values
        .iter()
        .copied()
        .map(|speed| speed.max(0.0).min(u64::MAX as f64) as u64)
        .collect::<Vec<_>>();
    frame.render_widget(
        Sparkline::default().data(&speeds).style(styles.accent),
        regions[2],
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
    if app.sessions.is_empty() {
        frame.render_widget(no_data(language, styles), inner);
        return;
    }
    let items = history(
        &app.sessions,
        Range::All,
        app.sessions[0].local_date,
        None,
        None,
    )
    .into_iter()
    .map(|session| {
        ListItem::new(format!(
            "{} {} {} {:.1}%",
            session.id,
            session.local_date,
            practice_name(language, session.mode),
            session.accuracy
        ))
    });
    frame.render_widget(List::new(items).style(styles.base), inner);
}

fn render_weak_keys(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::WeakKeys), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let counts = intended_key_counts(&app.sessions, app.settings.language);
    let keys = weak_keys(&counts, 10);
    if keys.is_empty() {
        frame.render_widget(no_data(language, styles), inner);
        return;
    }
    frame.render_widget(
        List::new(keys.into_iter().map(|key| {
            ListItem::new(format!(
                "{}: {:.1}% ({})",
                key.key,
                key.accuracy,
                key.correct.saturating_add(key.errors)
            ))
        }))
        .style(styles.base),
        inner,
    );
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
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!("{} KPM", app.settings.target_kpm)),
            Line::from(format!("{} WPM", app.settings.target_wpm)),
            Line::from(format!("{:.1}%", app.settings.target_accuracy)),
            Line::from(daily_minutes),
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
    let mut items = app.content.items().peekable();
    if items.peek().is_none() {
        frame.render_widget(no_data(language, styles), inner);
        return;
    }
    frame.render_widget(
        List::new(items.map(|item| {
            ListItem::new(format!(
                "{} · {} · {} · {}",
                item.id,
                item.pack_id,
                content_kind_name(language, item.kind),
                item.source.license
            ))
        }))
        .style(styles.base),
        inner,
    );
}

fn render_content_detail(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::Sources), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(item) = app.content.items().next() else {
        frame.render_widget(no_data(language, styles), inner);
        return;
    };
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
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!("{item_id}: {}", item.id)),
            Line::from(format!("{pack_id}: {}", item.pack_id)),
            Line::from(format!("{author}: {}", item.source.author)),
            Line::from(format!("{source_id}: {}", item.source.source_id)),
            Line::from(format!("{source_url}: {}", item.source.source_url)),
            Line::from(format!("{license}: {}", item.source.license)),
            Line::from(format!("{license_url}: {}", item.source.license_url)),
            Line::from(format!("{retrieved}: {}", item.source.retrieved_at)),
            Line::from(format!(
                "{modified}: {}",
                if item.source.modified { yes } else { no }
            )),
        ])
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
    let (practice_language, ui_language, keyboard, finger_guide, live_speed, adaptive, updates) =
        match language {
            Language::Ko => (
                "연습 언어",
                "화면 언어",
                "키보드",
                "손가락 안내",
                "실시간 속도",
                "적응형",
                "업데이트 확인",
            ),
            Language::En => (
                "Language",
                "UI language",
                "keyboard",
                "finger guide",
                "live speed",
                "adaptive",
                "updates",
            ),
        };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "{practice_language}: {}",
                language_name(app.settings.language)
            )),
            Line::from(format!(
                "{ui_language}: {}",
                language_name(app.settings.ui_language)
            )),
            Line::from(format!(
                "{}: {}",
                text(language, TextKey::Theme),
                app.settings.theme
            )),
            Line::from(format!(
                "{keyboard}: {}",
                toggle_name(language, app.settings.show_keyboard)
            )),
            Line::from(format!(
                "{finger_guide}: {}",
                toggle_name(language, app.settings.show_finger_guide)
            )),
            Line::from(format!(
                "{live_speed}: {}",
                toggle_name(language, app.settings.show_live_speed)
            )),
            Line::from(format!(
                "{adaptive}: {}",
                toggle_name(language, app.settings.adaptive)
            )),
            Line::from(format!(
                "{updates}: {}",
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
    frame.render_widget(
        List::new(app.themes.ids().map(|id| {
            let marker = if id == app.settings.theme { "> " } else { "  " };
            ListItem::new(format!("{marker}{id}"))
        }))
        .style(styles.base),
        inner,
    );
}

fn render_help(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::Help), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "Tab / ↑↓ · Enter {} · Esc {}",
                text(language, TextKey::Confirm),
                text(language, TextKey::Back)
            )),
            Line::from(format!(
                "q {} · ? {} · Ctrl+C",
                text(language, TextKey::Quit),
                text(language, TextKey::Help)
            )),
            Line::from("typeul quick|keys|words|sentence|long|test"),
            Line::from("typeul stats|history|themes"),
            Line::from("typeul content list|add|validate|disable"),
            Line::from("typeul paths|licenses|update"),
        ])
        .style(styles.base),
        inner,
    );
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

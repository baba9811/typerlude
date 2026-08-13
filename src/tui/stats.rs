use super::{
    format::{focus_marker, language_name, practice_name, speed_values},
    no_data,
    theme::ThemeStyles,
    titled,
};
use crate::{
    app::App,
    i18n::{TextKey, text},
    model::Language,
    stats::{
        adaptive_candidates, has_key_attempts, history, intended_key_counts, streak, summarize,
        weak_keys,
    },
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    symbols,
    text::Line,
    widgets::{Axis, Chart, Dataset, GraphType, List, ListItem, Paragraph, Sparkline},
};
use std::time::Duration;

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

pub(super) fn render_stats(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
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
    let kpm_average = finite_nonnegative(overview.kpm.average);
    let kpm_best = finite_nonnegative(overview.kpm.best);
    let wpm_average = finite_nonnegative(overview.wpm.average);
    let wpm_best = finite_nonnegative(overview.wpm.best);
    let stats_language = app.stats_language();
    let (average, speed_goal) = match stats_language {
        Language::Ko => (kpm_average, app.settings.target_kpm),
        Language::En => (wpm_average, app.settings.target_wpm),
    };
    let goal_speed = match (language, stats_language) {
        (Language::Ko, Language::Ko) => {
            format!("타수 {average:.0}/{speed_goal} 타/분")
        }
        (Language::En, Language::Ko) => format!("KPM {average:.0}/{speed_goal}"),
        (_, Language::En) => format!("WPM {average:.0}/{speed_goal}"),
    };
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
            Line::from(match language {
                Language::Ko => format!(
                    "타수 {kpm_average:.1}/{kpm_best:.1} 타/분 · WPM {wpm_average:.1}/{wpm_best:.1}"
                ),
                Language::En => format!(
                    "KPM {kpm_average:.1}/{kpm_best:.1} · WPM {wpm_average:.1}/{wpm_best:.1}"
                ),
            }),
            Line::from(format!(
                "{}: {practice_streak}",
                text(language, TextKey::Streak)
            )),
            Line::from(format!(
                "{goal_label}: {goal_speed} · {:.1}/{:.1}% · {minutes}/{} {minute_unit}",
                accuracy, app.settings.target_accuracy, app.settings.daily_minutes,
            )),
        ])
        .style(styles.base),
        regions[0],
    );
    let kpm_points = points
        .iter()
        .enumerate()
        .map(|(index, point)| (index as f64, finite_nonnegative(point.kpm)))
        .collect::<Vec<_>>();
    let wpm_points = points
        .iter()
        .enumerate()
        .map(|(index, point)| (index as f64, finite_nonnegative(point.wpm)))
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
    let y_max = kpm_points
        .iter()
        .chain(&wpm_points)
        .map(|&(_, speed)| speed)
        .fold(1.0_f64, f64::max);
    let x_max = kpm_points.len().saturating_sub(1).max(1) as f64;
    let title = match language {
        Language::Ko => "속도 추이",
        Language::En => "Speed trend",
    };
    frame.render_widget(
        Chart::new(vec![
            Dataset::default()
                .name(match language {
                    Language::Ko => "타수",
                    Language::En => "KPM",
                })
                .data(&kpm_points)
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(styles.accent),
            Dataset::default()
                .name("WPM")
                .data(&wpm_points)
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(styles.correct),
        ])
        .block(titled(title, styles))
        .x_axis(Axis::default().bounds([0.0, x_max]))
        .y_axis(Axis::default().bounds([0.0, y_max]))
        .hidden_legend_constraints((Constraint::Min(0), Constraint::Min(0))),
        regions[3],
    );
}

pub(super) fn render_history(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
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
        ListItem::new(format!(
            "{} {} {} {} {:.1}% {}",
            session.local_date,
            practice_name(language, session.mode),
            language_name(session.language),
            speed_values(
                language,
                finite_nonnegative(session.kpm),
                finite_nonnegative(session.wpm),
            ),
            session.accuracy,
            session.id,
        ))
    });
    frame.render_widget(List::new(items).style(styles.base), data);
}

pub(super) fn render_weak_keys(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::WeakKeys), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let counts = intended_key_counts(&app.sessions, app.stats_language());
    let keys = weak_keys(&counts, 10);
    if keys.is_empty() {
        if has_key_attempts(&counts, 10) {
            frame.render_widget(
                Paragraph::new(text(language, TextKey::WeakKeysPerfect)).style(styles.accent),
                inner,
            );
        } else {
            frame.render_widget(no_data(language, styles), inner);
        }
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

pub(super) fn render_goals(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
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
                "{}{kpm}: {} {}",
                focus_marker(app, 0),
                app.settings.target_kpm,
                match language {
                    Language::Ko => "타/분",
                    Language::En => "KPM",
                }
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

use super::{
    format::{speed_values, terminal_line},
    no_data,
    theme::ThemeStyles,
    titled,
};
use crate::{
    app::{App, Grade},
    diagnostic::terminal_safe,
    i18n::{TextKey, result_actions, text},
    model::Language,
    stats::has_key_attempts,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

pub(super) fn render_result(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
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
    let target = match language {
        Language::Ko => "목표",
        Language::En => "goal",
    };
    let speed_goal = format!(
        "{:.0} {}",
        result.speed_goal,
        match session.language {
            Language::Ko => "KPM",
            Language::En => "WPM",
        }
    );
    let mut lines = vec![
        Line::from(format!(
            "{}: {} / {target} {speed_goal} · {}",
            text(language, TextKey::Speed),
            speed_values(language, session.kpm, session.wpm),
            text(
                language,
                if result.speed_goal_met {
                    TextKey::GoalMet
                } else {
                    TextKey::GoalMissed
                }
            )
        )),
        Line::from(format!(
            "{}: {:.1}% / {target} {:.1}% · {}",
            text(language, TextKey::Accuracy),
            session.accuracy,
            result.accuracy_goal,
            text(
                language,
                if result.accuracy_goal_met {
                    TextKey::GoalMet
                } else {
                    TextKey::GoalMissed
                }
            )
        )),
        Line::from(format!(
            "{}: {} · {}: {}",
            text(language, TextKey::Errors),
            session.errors,
            text(language, TextKey::Duration),
            format_duration(language, session.duration_ms)
        )),
        Line::from(format!(
            "{}: {target} {} {} · {}",
            text(language, TextKey::DailyMinutes),
            result.daily_minutes_goal,
            match language {
                Language::Ko => "분",
                Language::En => "min",
            },
            text(
                language,
                if result.daily_minutes_met {
                    TextKey::GoalMet
                } else {
                    TextKey::GoalMissed
                }
            )
        )),
    ];
    if let (Some(kpm), Some(wpm)) = (result.previous_kpm, result.previous_wpm) {
        lines.push(Line::from(format!(
            "{}: {}",
            text(language, TextKey::Previous),
            speed_values(language, kpm, wpm)
        )));
    }
    if let (Some(kpm), Some(wpm)) = (result.best_kpm, result.best_wpm) {
        lines.push(Line::from(format!(
            "{}: {}",
            text(language, TextKey::Best),
            speed_values(language, kpm, wpm)
        )));
    }
    if let (Some(kpm), Some(wpm)) = (result.kpm_delta, result.wpm_delta) {
        lines.push(Line::from(signed_speed_values(language, kpm, wpm)));
    }
    if let Some(grade) = result.grade {
        lines.push(Line::from(format!(
            "{}: {}",
            match language {
                Language::Ko => "Typerlude 상대 등급",
                Language::En => "Typerlude relative grade",
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
            "{rolling}: {}",
            speed_values(language, long.best_rolling_kpm, long.best_rolling_wpm)
        )));
        lines.push(Line::from(format!(
            "{graphemes}: {}/{} · {}: {}%",
            long.completed_graphemes,
            long.total_graphemes,
            text(language, TextKey::Progress),
            long.percent
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
        let keys = result
            .weak_keys
            .iter()
            .take(5)
            .map(|key| format!("{} {:.1}%", key.key, key.accuracy))
            .collect::<Vec<_>>()
            .join(" · ");
        lines.push(Line::from(Span::styled(
            terminal_line(
                &format!("{}: {keys}", text(language, TextKey::WeakKeys)),
                body.width,
            ),
            styles.accent,
        )));
    } else if has_key_attempts(&session.intended_keys, 1) && lines.len() < usize::from(body.height)
    {
        lines.push(Line::from(Span::styled(
            text(language, TextKey::WeakKeysPerfect),
            styles.accent,
        )));
    }
    frame.render_widget(Paragraph::new(lines).style(styles.base), body);
}

pub(super) fn render_update_notice(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    styles: ThemeStyles,
) {
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

fn signed_speed_values(ui_language: Language, kpm: f64, wpm: f64) -> String {
    match ui_language {
        Language::Ko => format!("타수 {kpm:+.1} 타/분 · WPM {wpm:+.1}"),
        Language::En => format!("KPM {kpm:+.1} · WPM {wpm:+.1}"),
    }
}

fn format_duration(language: Language, milliseconds: u64) -> String {
    let centiseconds = milliseconds.saturating_add(5) / 10;
    let hours = centiseconds / 360_000;
    let minutes = centiseconds / 6_000 % 60;
    let seconds = centiseconds % 6_000;
    let whole = seconds / 100;
    let fraction = seconds % 100;
    match (language, hours, minutes) {
        (Language::Ko, 0, 0) => format!("{whole}.{fraction:02}초"),
        (Language::Ko, 0, _) => format!("{minutes}분 {whole}.{fraction:02}초"),
        (Language::Ko, _, _) => format!("{hours}시간 {minutes}분 {whole}.{fraction:02}초"),
        (Language::En, 0, 0) => format!("{whole}.{fraction:02} sec"),
        (Language::En, 0, _) => format!("{minutes} min {whole}.{fraction:02} sec"),
        (Language::En, _, _) => format!("{hours} hr {minutes} min {whole}.{fraction:02} sec"),
    }
}

const fn grade_name(grade: Grade) -> &'static str {
    match grade {
        Grade::A => "A",
        Grade::B => "B",
        Grade::C => "C",
        Grade::D => "D",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_speed_values_localize_both_units() {
        assert_eq!(
            signed_speed_values(Language::En, 10.0, -2.0),
            "KPM +10.0 · WPM -2.0"
        );
        assert_eq!(
            signed_speed_values(Language::Ko, 10.0, -2.0),
            "타수 +10.0 타/분 · WPM -2.0"
        );
    }

    #[test]
    fn result_duration_rounds_before_splitting_units() {
        assert_eq!(format_duration(Language::En, 4_444), "4.44 sec");
        assert_eq!(format_duration(Language::Ko, 2_673_444), "44분 33.44초");
        assert_eq!(
            format_duration(Language::En, 3_723_999),
            "1 hr 2 min 4.00 sec"
        );
        assert_eq!(format_duration(Language::Ko, 59_999), "1분 0.00초");
    }
}

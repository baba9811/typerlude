use super::{App, Grade, ItemDelta, LongOutcome, PracticeMode, ResultView, Screen};
use crate::{
    model::{Difficulty, Language, PracticeKind},
    practice::Metrics,
    stats::weak_keys,
    storage::{SessionRecord, save_session},
};
use anyhow::{Result, bail};
use std::time::Instant;
use time::OffsetDateTime;

pub fn grade(speed: f64, speed_goal: f64, accuracy: f64, accuracy_goal: f64) -> Grade {
    if speed >= speed_goal && accuracy >= accuracy_goal {
        Grade::A
    } else if speed >= speed_goal * 0.8 && accuracy >= 95.0 {
        Grade::B
    } else if speed >= speed_goal * 0.6 && accuracy >= 90.0 {
        Grade::C
    } else {
        Grade::D
    }
}

impl App {
    pub fn finish_practice(&mut self, now: Instant) -> Result<ResultView> {
        let Some(active) = self.practice.as_ref() else {
            bail!("no active practice");
        };
        if active.engine.metrics(now).attempted_units == 0 {
            bail!("cannot finish practice without an attempt");
        }

        let Some(mut active) = self.practice.take() else {
            bail!("no active practice");
        };
        if let Some(stream) = active.stream.clone() {
            self.retry_stream = Some(stream);
        }
        let metrics = active.engine.finalize(now);
        let language = active.engine.language();
        let kind = active.kind();
        let long = (kind == PracticeKind::Long).then(|| {
            let completed_graphemes = active.engine.cursor();
            let total_graphemes = active.engine.target_len();
            let (best_rolling_kpm, best_rolling_wpm) = active.engine.best_rolling_speeds();
            LongOutcome {
                best_rolling_kpm,
                best_rolling_wpm,
                completed_graphemes,
                total_graphemes,
                percent: completed_graphemes.saturating_mul(100) / total_graphemes,
            }
        });
        let started_at = active
            .started_at_utc
            .unwrap_or_else(OffsetDateTime::now_utc);
        let content_id = active
            .content_ids
            .first()
            .cloned()
            .unwrap_or_else(|| practice_id(kind).into());
        let long_difficulty = active
            .long_metadata
            .as_ref()
            .and_then(|metadata| metadata.difficulty);
        let difficulty = match active.mode {
            PracticeMode::Words { difficulty, .. } => match difficulty {
                Difficulty::Easy => Some(1),
                Difficulty::Medium => Some(2),
                Difficulty::Hard => Some(3),
                Difficulty::Mixed => None,
            },
            PracticeMode::Long { .. } => long_difficulty,
            _ => None,
        };
        let session = SessionRecord::from_result(
            started_at,
            language,
            kind,
            content_id,
            difficulty,
            &metrics,
            active.engine.intended_keys(),
        );
        let speed = session_speed(&session);
        let comparable = self
            .sessions
            .iter()
            .filter(|prior| prior.language == language && prior.mode == kind)
            .collect::<Vec<_>>();
        let previous = comparable
            .iter()
            .copied()
            .filter(|prior| prior.kpm.is_finite() && prior.wpm.is_finite())
            .max_by(|left, right| {
                left.started_at_unix_ms
                    .cmp(&right.started_at_unix_ms)
                    .then_with(|| left.id.cmp(&right.id))
            });
        let previous_kpm = previous.map(|prior| prior.kpm);
        let previous_wpm = previous.map(|prior| prior.wpm);
        let best_kpm = comparable
            .iter()
            .map(|prior| prior.kpm)
            .filter(|speed| speed.is_finite())
            .max_by(f64::total_cmp);
        let best_wpm = comparable
            .iter()
            .map(|prior| prior.wpm)
            .filter(|speed| speed.is_finite())
            .max_by(f64::total_cmp);
        let speed_goal = match language {
            Language::Ko => f64::from(self.settings.target_kpm),
            Language::En => f64::from(self.settings.target_wpm),
        };
        let prior_duration = self
            .sessions
            .iter()
            .filter(|prior| prior.local_date == session.local_date)
            .fold(0_u64, |total, prior| {
                total.saturating_add(prior.duration_ms)
            });
        let daily_target = u64::from(self.settings.daily_minutes).saturating_mul(60_000);
        let result_grade = (kind == PracticeKind::Test).then(|| {
            grade(
                speed,
                speed_goal,
                session.accuracy,
                self.settings.target_accuracy,
            )
        });
        let mut view = ResultView {
            previous_kpm,
            previous_wpm,
            best_kpm,
            best_wpm,
            kpm_delta: previous_kpm.map(|previous| session.kpm - previous),
            wpm_delta: previous_wpm.map(|previous| session.wpm - previous),
            speed_goal,
            accuracy_goal: self.settings.target_accuracy,
            daily_minutes_goal: self.settings.daily_minutes,
            speed_goal_met: speed >= speed_goal,
            accuracy_goal_met: session.accuracy >= self.settings.target_accuracy,
            daily_minutes_met: prior_duration.saturating_add(session.duration_ms) >= daily_target,
            weak_keys: weak_keys(&session.intended_keys, 1)
                .into_iter()
                .take(5)
                .collect(),
            grade: result_grade,
            save_error: None,
            long,
            session,
        };
        match save_session(&self.paths, &view.session) {
            Ok(_) => self.sessions.push(view.session.clone()),
            Err(error) => view.save_error = Some(error.root_cause().to_string()),
        }

        self.remember_focus();
        self.screen = Screen::Result;
        self.parent = Screen::Home;
        self.parent_before_help = None;
        self.focus = 0;
        self.result = Some(view.clone());
        Ok(view)
    }
}

pub(super) fn item_delta(before: &Metrics, after: &Metrics) -> ItemDelta {
    let correct_units = after.correct_units.saturating_sub(before.correct_units);
    let correct_cells = after.correct_cells.saturating_sub(before.correct_cells);
    let attempted_units = after.attempted_units.saturating_sub(before.attempted_units);
    let correct_attempts = correct_attempts(after).saturating_sub(correct_attempts(before));
    let minutes = after.active.saturating_sub(before.active).as_secs_f64() / 60.0;
    let kpm = if minutes > 0.0 {
        correct_units as f64 / minutes
    } else {
        0.0
    };
    ItemDelta {
        correct_units,
        attempted_units,
        errors: after.errors.saturating_sub(before.errors),
        kpm,
        wpm: if minutes > 0.0 {
            correct_cells as f64 / minutes / 5.0
        } else {
            0.0
        },
        accuracy: if attempted_units == 0 {
            100.0
        } else {
            correct_attempts as f64 / attempted_units as f64 * 100.0
        },
    }
}

fn correct_attempts(metrics: &Metrics) -> u64 {
    (metrics.accuracy / 100.0 * metrics.attempted_units as f64)
        .round()
        .clamp(0.0, metrics.attempted_units as f64) as u64
}

fn session_speed(session: &SessionRecord) -> f64 {
    match session.language {
        Language::Ko => session.kpm,
        Language::En => session.wpm,
    }
}

const fn practice_id(kind: PracticeKind) -> &'static str {
    match kind {
        PracticeKind::Quick => "quick",
        PracticeKind::Key => "key",
        PracticeKind::Words => "words",
        PracticeKind::Sentence => "sentence",
        PracticeKind::Long => "long",
        PracticeKind::Test => "test",
    }
}

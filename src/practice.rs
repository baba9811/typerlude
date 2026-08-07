use crate::{
    model::{Language, PracticeKind},
    typing::{key_units, split_graphemes, unit_count},
};
use anyhow::{Result, bail};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
struct Cell {
    text: String,
    correct: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub active: Duration,
    pub correct_units: u64,
    pub attempted_units: u64,
    pub errors: u64,
    pub backspaces: u64,
    pub cpm: f64,
    pub kpm: f64,
    pub wpm: f64,
    pub accuracy: f64,
}

pub struct PracticeEngine {
    language: Language,
    kind: PracticeKind,
    target: Vec<String>,
    input: Vec<Cell>,
    started_at: Option<Instant>,
    paused_at: Option<Instant>,
    paused_total: Duration,
    limit: Option<Duration>,
    attempted_units: u64,
    correct_attempt_units: u64,
    errors: u64,
    backspaces: u64,
    intended: BTreeMap<char, [u64; 2]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputOutcome {
    Accepted,
    Finished,
    IgnoredWhilePaused,
}

impl PracticeEngine {
    pub fn new(
        language: Language,
        kind: PracticeKind,
        target: &str,
        limit: Option<Duration>,
    ) -> Result<Self> {
        let target = split_graphemes(target);
        if target.is_empty() {
            bail!("practice target cannot be empty");
        }

        Ok(Self {
            language,
            kind,
            target,
            input: Vec::new(),
            started_at: None,
            paused_at: None,
            paused_total: Duration::ZERO,
            limit,
            attempted_units: 0,
            correct_attempt_units: 0,
            errors: 0,
            backspaces: 0,
            intended: BTreeMap::new(),
        })
    }

    pub fn input(&mut self, text: &str, now: Instant) -> InputOutcome {
        if self.paused_at.is_some() {
            return InputOutcome::IgnoredWhilePaused;
        }
        if self.is_finished(now) {
            return InputOutcome::Finished;
        }

        for grapheme in split_graphemes(text) {
            let Some(target) = self.target.get(self.input.len()) else {
                break;
            };
            self.started_at.get_or_insert(now);

            let correct = grapheme == *target;
            let units = unit_count(self.language, target);
            self.attempted_units += units;
            if correct {
                self.correct_attempt_units += units;
            } else {
                self.errors += 1;
            }
            for unit in key_units(self.language, target) {
                self.intended.entry(unit).or_default()[usize::from(!correct)] += 1;
            }
            self.input.push(Cell {
                text: grapheme,
                correct,
            });

            if self.target_complete() {
                return InputOutcome::Finished;
            }
        }

        InputOutcome::Accepted
    }

    pub fn backspace(&mut self) -> bool {
        if self.target_complete() {
            return false;
        }
        if self.input.pop().is_some() {
            self.backspaces += 1;
            true
        } else {
            false
        }
    }

    pub fn toggle_pause(&mut self, now: Instant) -> bool {
        if self.kind == PracticeKind::Test {
            return false;
        }

        if let Some(paused_at) = self.paused_at.take() {
            if self.started_at.is_some() {
                self.paused_total += now.saturating_duration_since(paused_at);
            }
        } else {
            self.paused_at = Some(now);
        }
        true
    }

    pub fn metrics(&self, now: Instant) -> Metrics {
        let active = self.active(now);
        let correct_cells = self
            .input
            .iter()
            .zip(&self.target)
            .filter(|(cell, target)| cell.correct && cell.text == **target)
            .count() as u64;
        let correct_units = self
            .input
            .iter()
            .zip(&self.target)
            .filter(|(cell, target)| cell.correct && cell.text == **target)
            .map(|(_, target)| unit_count(self.language, target))
            .sum();
        let minutes = active.as_secs_f64() / 60.0;
        let cpm = if minutes > 0.0 {
            correct_cells as f64 / minutes
        } else {
            0.0
        };
        let kpm = if minutes > 0.0 {
            correct_units as f64 / minutes
        } else {
            0.0
        };

        Metrics {
            active,
            correct_units,
            attempted_units: self.attempted_units,
            errors: self.errors,
            backspaces: self.backspaces,
            cpm,
            kpm,
            wpm: cpm / 5.0,
            accuracy: if self.attempted_units == 0 {
                100.0
            } else {
                self.correct_attempt_units as f64 / self.attempted_units as f64 * 100.0
            },
        }
    }

    pub fn intended_keys(&self) -> &BTreeMap<char, [u64; 2]> {
        &self.intended
    }

    pub fn is_finished(&self, now: Instant) -> bool {
        self.target_complete() || self.limit.is_some_and(|limit| self.active(now) >= limit)
    }

    fn target_complete(&self) -> bool {
        self.input.len() == self.target.len() && self.input.iter().all(|cell| cell.correct)
    }

    fn active(&self, now: Instant) -> Duration {
        let Some(started_at) = self.started_at else {
            return Duration::ZERO;
        };
        let current_pause = self.paused_at.map_or(Duration::ZERO, |paused_at| {
            now.saturating_duration_since(paused_at)
        });
        now.saturating_duration_since(started_at)
            .saturating_sub(self.paused_total)
            .saturating_sub(current_pause)
    }
}

#[cfg(test)]
mod tests {
    use super::{InputOutcome, PracticeEngine};
    use crate::model::{Language, PracticeKind};
    use std::time::{Duration, Instant};

    #[test]
    fn correction_does_not_erase_an_accuracy_error_or_inflate_speed() {
        let start = Instant::now();
        let mut engine =
            PracticeEngine::new(Language::Ko, PracticeKind::Sentence, "한", None).unwrap();
        engine.input("강", start);
        assert!(engine.backspace());
        engine.input("한", start + Duration::from_secs(60));

        let metrics = engine.metrics(start + Duration::from_secs(60));
        assert_eq!(metrics.correct_units, 3);
        assert_eq!(metrics.attempted_units, 6);
        assert_eq!(metrics.errors, 1);
        assert_eq!(metrics.backspaces, 1);
        assert_eq!(metrics.kpm, 3.0);
        assert_eq!(metrics.accuracy, 50.0);
    }

    #[test]
    fn paused_time_is_excluded_and_tests_refuse_pause() {
        let start = Instant::now();
        let mut practice =
            PracticeEngine::new(Language::En, PracticeKind::Words, "ab", None).unwrap();
        practice.input("a", start);
        assert!(practice.toggle_pause(start + Duration::from_secs(10)));
        assert!(practice.toggle_pause(start + Duration::from_secs(40)));
        practice.input("b", start + Duration::from_secs(70));
        assert_eq!(
            practice.metrics(start + Duration::from_secs(70)).active,
            Duration::from_secs(40)
        );

        let mut test = PracticeEngine::new(
            Language::En,
            PracticeKind::Test,
            "ab",
            Some(Duration::from_secs(60)),
        )
        .unwrap();
        assert!(!test.toggle_pause(start));
    }

    #[test]
    fn a_current_pause_does_not_add_active_time() {
        let start = Instant::now();
        let mut engine =
            PracticeEngine::new(Language::En, PracticeKind::Words, "ab", None).unwrap();
        engine.input("a", start);
        engine.toggle_pause(start + Duration::from_secs(10));

        assert_eq!(
            engine.metrics(start + Duration::from_secs(40)).active,
            Duration::from_secs(10)
        );
    }

    #[test]
    fn a_timed_test_finishes_at_its_active_deadline() {
        let start = Instant::now();
        let mut engine = PracticeEngine::new(
            Language::En,
            PracticeKind::Test,
            "abcdef",
            Some(Duration::from_secs(60)),
        )
        .unwrap();
        engine.input("a", start);

        assert!(!engine.is_finished(start + Duration::from_secs(59)));
        assert!(engine.is_finished(start + Duration::from_secs(60)));
    }

    #[test]
    fn empty_targets_are_rejected() {
        assert!(PracticeEngine::new(Language::En, PracticeKind::Words, "", None).is_err());
    }

    #[test]
    fn empty_input_does_not_start_the_timer_or_change_accuracy() {
        let start = Instant::now();
        let mut engine = PracticeEngine::new(Language::En, PracticeKind::Words, "a", None).unwrap();

        assert_eq!(engine.input("", start), InputOutcome::Accepted);
        let metrics = engine.metrics(start + Duration::from_secs(60));
        assert_eq!(metrics.active, Duration::ZERO);
        assert_eq!(metrics.attempted_units, 0);
        assert_eq!(metrics.accuracy, 100.0);
    }

    #[test]
    fn multi_grapheme_input_is_normalized_and_stops_at_completion() {
        let start = Instant::now();
        let mut engine =
            PracticeEngine::new(Language::Ko, PracticeKind::Sentence, "한글", None).unwrap();

        assert_eq!(engine.input("한글더", start), InputOutcome::Finished);
        let metrics = engine.metrics(start + Duration::from_secs(60));
        assert_eq!(metrics.correct_units, 6);
        assert_eq!(metrics.attempted_units, 6);
        assert_eq!(metrics.errors, 0);
        assert_eq!(metrics.cpm, 2.0);
        assert_eq!(metrics.kpm, 6.0);
        assert_eq!(metrics.wpm, 0.4);
    }

    #[test]
    fn intended_key_buckets_track_correct_and_erroneous_attempts() {
        let start = Instant::now();
        let mut engine =
            PracticeEngine::new(Language::Ko, PracticeKind::Sentence, "한", None).unwrap();
        engine.input("강", start);
        engine.backspace();
        engine.input("한", start);

        assert_eq!(engine.intended.get(&'ㅎ'), Some(&[1, 1]));
        assert_eq!(engine.intended.get(&'ㅏ'), Some(&[1, 1]));
        assert_eq!(engine.intended.get(&'ㄴ'), Some(&[1, 1]));
    }

    #[test]
    fn a_wrong_full_length_cell_requires_correction() {
        let start = Instant::now();
        let mut engine = PracticeEngine::new(Language::En, PracticeKind::Words, "a", None).unwrap();

        assert_eq!(engine.input("x", start), InputOutcome::Accepted);
        assert!(!engine.is_finished(start));
        assert_eq!(engine.input("a", start), InputOutcome::Accepted);
        assert_eq!(engine.metrics(start).attempted_units, 1);
        assert!(engine.backspace());
        assert_eq!(engine.input("a", start), InputOutcome::Finished);
        assert_eq!(engine.metrics(start).attempted_units, 2);
    }

    #[test]
    fn paused_finished_and_timed_out_inputs_do_not_change_metrics() {
        let start = Instant::now();
        let mut paused =
            PracticeEngine::new(Language::En, PracticeKind::Words, "ab", None).unwrap();
        paused.input("a", start);
        paused.toggle_pause(start);
        let before_pause = paused.metrics(start);
        assert_eq!(
            paused.input("x", start + Duration::from_secs(1)),
            InputOutcome::IgnoredWhilePaused
        );
        assert_eq!(paused.metrics(start), before_pause);

        let mut completed =
            PracticeEngine::new(Language::En, PracticeKind::Words, "a", None).unwrap();
        assert_eq!(completed.input("a", start), InputOutcome::Finished);
        let before_extra = completed.metrics(start);
        assert_eq!(completed.input("x", start), InputOutcome::Finished);
        assert_eq!(completed.metrics(start), before_extra);

        let mut timed = PracticeEngine::new(
            Language::En,
            PracticeKind::Test,
            "ab",
            Some(Duration::from_secs(1)),
        )
        .unwrap();
        timed.input("a", start);
        let at_deadline = timed.metrics(start + Duration::from_secs(1));
        assert_eq!(
            timed.input("b", start + Duration::from_secs(1)),
            InputOutcome::Finished
        );
        assert_eq!(timed.metrics(start + Duration::from_secs(1)), at_deadline);
    }
}

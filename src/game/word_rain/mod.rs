use crate::{
    game::GameDifficulty,
    model::Language,
    typing::{key_units, unit_count},
};
use anyhow::{Result, bail};
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(crate) const LOGICAL_WIDTH: u16 = 72;
const LOGICAL_HEIGHT: f64 = 16.0;
const MAX_WORD_WIDTH: usize = 24;
const MAX_TICK: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub(crate) struct FallingWord {
    id: u64,
    text: String,
    width: u16,
    left: u16,
    progress: f64,
}

impl FallingWord {
    pub(crate) const fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) const fn width(&self) -> u16 {
        self.width
    }

    pub(crate) const fn left(&self) -> u16 {
        self.left
    }

    pub(crate) const fn progress(&self) -> f64 {
        self.progress
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WordRainOutcome {
    pub(crate) score: u64,
    pub(crate) cleared: u64,
    pub(crate) max_combo: u64,
    pub(crate) level: u64,
    pub(crate) active_time: Duration,
    pub(crate) missed_word: String,
}

pub(crate) struct WordRain {
    language: Language,
    difficulty: GameDifficulty,
    words: Vec<String>,
    active: Vec<FallingWord>,
    rng: fastrand::Rng,
    next_id: u64,
    target: Option<u64>,
    input: String,
    spawn_elapsed: Duration,
    last_tick: Instant,
    active_time: Duration,
    score: u64,
    combo: u64,
    max_combo: u64,
    cleared: u64,
    paused: bool,
    viewport_supported: bool,
    outcome: Option<WordRainOutcome>,
}

impl WordRain {
    pub(crate) fn new(
        language: Language,
        difficulty: GameDifficulty,
        words: Vec<String>,
        seed: u64,
        now: Instant,
    ) -> Result<Self> {
        if words.is_empty()
            || words.iter().any(|word| {
                let width = UnicodeWidthStr::width(word.as_str());
                !(1..=MAX_WORD_WIDTH).contains(&width)
            })
        {
            bail!("word rain requires playable words");
        }

        let mut game = Self {
            language,
            difficulty,
            words,
            active: Vec::new(),
            rng: fastrand::Rng::with_seed(seed),
            next_id: 1,
            target: None,
            input: String::new(),
            spawn_elapsed: Duration::ZERO,
            last_tick: now,
            active_time: Duration::ZERO,
            score: 0,
            combo: 0,
            max_combo: 0,
            cleared: 0,
            paused: false,
            viewport_supported: true,
            outcome: None,
        };
        if !game.spawn() {
            bail!("word rain could not place its first word");
        }
        Ok(game)
    }

    fn level(&self) -> u64 {
        1_u64.saturating_add(self.cleared / 10)
    }

    pub(crate) const fn difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    pub(crate) fn active_words(&self) -> impl ExactSizeIterator<Item = &FallingWord> {
        self.active.iter()
    }

    pub(crate) const fn is_paused(&self) -> bool {
        self.paused
    }

    pub(crate) const fn outcome(&self) -> Option<&WordRainOutcome> {
        self.outcome.as_ref()
    }

    pub(crate) const fn score(&self) -> u64 {
        self.score
    }

    pub(crate) const fn combo(&self) -> u64 {
        self.combo
    }

    pub(crate) fn current_level(&self) -> u64 {
        self.level()
    }

    fn speed_multiplier(&self) -> f64 {
        1.10_f64.powf(self.level().saturating_sub(1) as f64)
    }

    fn effective_fall_time(&self) -> Duration {
        Duration::from_secs_f64(self.base_fall_seconds() / self.speed_multiplier())
    }

    fn spawn_interval(&self) -> Duration {
        Duration::from_secs_f64(self.base_spawn_seconds() / self.speed_multiplier())
    }

    fn base_fall_seconds(&self) -> f64 {
        match self.difficulty {
            GameDifficulty::Easy => 18.0,
            GameDifficulty::Medium => 14.0,
            GameDifficulty::Hard => 10.0,
            GameDifficulty::Hell => 7.0,
        }
    }

    fn base_spawn_seconds(&self) -> f64 {
        match self.difficulty {
            GameDifficulty::Easy => 2.4,
            GameDifficulty::Medium => 2.0,
            GameDifficulty::Hard => 1.6,
            GameDifficulty::Hell => 1.2,
        }
    }

    pub(crate) fn tick(&mut self, now: Instant) {
        if self.outcome.is_some() || self.paused || !self.viewport_supported {
            self.last_tick = now;
            return;
        }

        let elapsed = now.saturating_duration_since(self.last_tick).min(MAX_TICK);
        self.last_tick = now;
        self.active_time = self.active_time.saturating_add(elapsed);
        let fall_seconds = self.effective_fall_time().as_secs_f64();
        let progress = if fall_seconds == 0.0 {
            f64::INFINITY
        } else {
            elapsed.as_secs_f64() / fall_seconds
        };
        for word in &mut self.active {
            word.progress += progress;
        }

        if let Some(missed) = self
            .active
            .iter()
            .filter(|word| word.progress >= 1.0)
            .max_by(|left, right| {
                left.progress
                    .total_cmp(&right.progress)
                    .then_with(|| right.id.cmp(&left.id))
            })
        {
            self.outcome = Some(WordRainOutcome {
                score: self.score,
                cleared: self.cleared,
                max_combo: self.max_combo,
                level: self.level(),
                active_time: self.active_time,
                missed_word: missed.text.clone(),
            });
            return;
        }

        self.spawn_elapsed = self.spawn_elapsed.saturating_add(elapsed);
        if self.spawn_elapsed >= self.spawn_interval() && self.spawn() {
            self.spawn_elapsed = Duration::ZERO;
        }
    }

    pub(crate) fn toggle_pause(&mut self, now: Instant) -> bool {
        if self.outcome.is_some() {
            return false;
        }
        self.paused = !self.paused;
        self.last_tick = now;
        true
    }

    pub(crate) fn set_viewport_supported(&mut self, supported: bool, now: Instant) {
        if self.viewport_supported != supported {
            self.viewport_supported = supported;
            self.last_tick = now;
        }
    }

    pub(crate) fn input_char(&mut self, character: char) {
        if self.outcome.is_some() || self.paused || !self.viewport_supported {
            return;
        }
        let was_empty = self.input.is_empty();
        self.input.push(character);
        if was_empty && self.target.is_none() {
            self.target = self.select_target();
        }
        if !self.input_is_valid() {
            self.combo = 0;
            return;
        }

        let input_units = key_units(self.language, &self.input);
        let complete = self.target.and_then(|id| {
            self.active
                .iter()
                .find(|word| word.id == id)
                .map(|word| key_units(self.language, &word.text) == input_units)
        });
        if complete == Some(true) {
            self.complete_target();
        }
    }

    pub(crate) fn backspace(&mut self) -> bool {
        if self.outcome.is_some() || self.paused || !self.viewport_supported {
            return false;
        }
        let removed = self.input.pop().is_some();
        if self.input.is_empty() {
            self.target = None;
        }
        removed
    }

    pub(crate) fn submit_input(&mut self) {
        if self.outcome.is_some() || self.paused || !self.viewport_supported {
            return;
        }
        self.input.clear();
        self.target = None;
    }

    pub(crate) fn input(&self) -> &str {
        &self.input
    }

    pub(crate) fn input_is_valid(&self) -> bool {
        if self.input.is_empty() {
            return true;
        }
        let Some(word) = self
            .target
            .and_then(|id| self.active.iter().find(|word| word.id == id))
        else {
            return false;
        };
        let input = key_units(self.language, &self.input);
        key_units(self.language, &word.text).starts_with(&input)
    }

    pub(crate) const fn target_id(&self) -> Option<u64> {
        self.target
    }

    pub(crate) fn matched_graphemes(&self, id: u64) -> usize {
        if self.target != Some(id) {
            return 0;
        }
        let Some(word) = self.active.iter().find(|word| word.id == id) else {
            return 0;
        };
        let target = key_units(self.language, &word.text);
        let input = key_units(self.language, &self.input);
        let common = target
            .iter()
            .zip(input.iter())
            .take_while(|(target, input)| target == input)
            .count();
        let mut units = 0;
        word.text
            .graphemes(true)
            .take_while(|grapheme| {
                units += key_units(self.language, grapheme).len();
                units <= common
            })
            .count()
    }

    fn select_target(&self) -> Option<u64> {
        let input = key_units(self.language, &self.input);
        self.active
            .iter()
            .filter(|word| key_units(self.language, &word.text).starts_with(&input))
            .max_by(|left, right| {
                left.progress
                    .total_cmp(&right.progress)
                    .then_with(|| right.id.cmp(&left.id))
            })
            .map(|word| word.id)
    }

    fn complete_target(&mut self) {
        let Some(index) = self
            .target
            .and_then(|id| self.active.iter().position(|word| word.id == id))
        else {
            return;
        };
        let level = self.level();
        self.combo = self.combo.saturating_add(1);
        self.max_combo = self.max_combo.max(self.combo);
        let factor = 10_u64
            .saturating_mul(level)
            .saturating_add(self.combo.min(20));
        self.score = self.score.saturating_add(
            unit_count(self.language, &self.active[index].text).saturating_mul(factor),
        );
        self.active.remove(index);
        self.cleared = self.cleared.saturating_add(1);
        self.input.clear();
        self.target = None;
    }

    fn spawn(&mut self) -> bool {
        let active_initials = self
            .active
            .iter()
            .filter_map(|word| key_units(self.language, &word.text).first().copied())
            .collect::<Vec<_>>();
        let unused = self
            .words
            .iter()
            .enumerate()
            .filter(|(_, word)| !self.active.iter().any(|active| active.text == **word))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let unique = unused
            .iter()
            .copied()
            .filter(|&index| {
                key_units(self.language, &self.words[index])
                    .first()
                    .is_some_and(|unit| !active_initials.contains(unit))
            })
            .collect::<Vec<_>>();
        let candidates = if !unique.is_empty() {
            &unique
        } else if !unused.is_empty() {
            &unused
        } else {
            return self.spawn_from_candidates(&(0..self.words.len()).collect::<Vec<_>>());
        };
        self.spawn_from_candidates(candidates)
    }

    fn spawn_from_candidates(&mut self, candidates: &[usize]) -> bool {
        let text = self.words[candidates[self.rng.usize(..candidates.len())]].clone();
        let width = UnicodeWidthStr::width(text.as_str()) as u16;
        let safe = (0..=LOGICAL_WIDTH - width)
            .filter(|&left| self.is_safe_column(left, width))
            .collect::<Vec<_>>();
        if safe.is_empty() {
            return false;
        }
        let left = safe[self.rng.usize(..safe.len())];
        self.active.push(FallingWord {
            id: self.next_id,
            text,
            width,
            left,
            progress: 0.0,
        });
        self.next_id = self.next_id.saturating_add(1);
        true
    }

    fn is_safe_column(&self, left: u16, width: u16) -> bool {
        self.active.iter().all(|other| {
            (other.progress).abs() >= 2.0 / LOGICAL_HEIGHT
                || left.saturating_add(width).saturating_add(2) <= other.left
                || other.left.saturating_add(other.width).saturating_add(2) <= left
        })
    }
}

#[cfg(test)]
mod tests;

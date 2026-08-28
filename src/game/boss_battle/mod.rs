use crate::{
    game::GameDifficulty,
    model::Language,
    typing::{key_units, normalize_nfc, unit_count},
};
use anyhow::{Result, bail};
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

const BATTLE_LIMIT: Duration = Duration::from_secs(90);
const MAX_TICK: Duration = Duration::from_millis(250);
const INTRO_DURATION: Duration = Duration::from_millis(800);
const PHASE_DURATION: Duration = Duration::from_millis(750);
const ATTACK_DURATION: Duration = Duration::from_millis(600);
const FINISH_DURATION: Duration = Duration::from_secs(1);
const HIT_DURATION: Duration = Duration::from_millis(180);
const MAX_WORD_WIDTH: usize = 24;
const WARDEN_HEALTH: [u64; 4] = [280, 480, 720, 975];
const QUEEN_HEALTH: [u64; 4] = [180, 320, 500, 653];
// Word-slot rollback loses different physical-unit chunks across the two content catalogs.
const ARCHON_HEALTH_KO: [u64; 4] = [310, 500, 850, 1_090];
const ARCHON_HEALTH_EN: [u64; 4] = [303, 500, 895, 1_220];
const QUEEN_STAGGER: [Duration; 4] = [
    Duration::from_millis(1_500),
    Duration::from_millis(1_200),
    Duration::from_millis(1_500),
    Duration::from_millis(1_200),
];
const WARDEN_CAST: [Duration; 4] = [
    Duration::from_secs(12),
    Duration::from_secs(9),
    Duration::from_secs(7),
    Duration::from_secs(6),
];
const WARDEN_CORE: [Duration; 4] = [
    Duration::from_secs(6),
    Duration::from_secs(5),
    Duration::from_secs(4),
    Duration::from_secs(4),
];
const ARCHON_CANTICLE: [Duration; 4] = [
    Duration::from_secs(14),
    Duration::from_secs(25),
    Duration::from_secs(25),
    Duration::from_secs(22),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(crate) enum BossKind {
    IronWarden,
    ThornQueen,
    NullArchon,
}

impl BossKind {
    pub(crate) const ALL: [Self; 3] = [Self::IronWarden, Self::ThornQueen, Self::NullArchon];

    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BossPhase {
    One,
    Two,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BattleCue {
    Intro,
    Hit,
    PhaseTransition,
    BossAttack,
    Victory,
    Defeat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum BossPatternView {
    Warden {
        locks: u8,
        core_exposed: bool,
        cast_progress: f64,
    },
    Queen {
        target_id: Option<u64>,
    },
    NullArchon {
        checksum: u8,
        canticle_progress: f64,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct BossPrompt {
    id: u64,
    text: String,
    deadline: Duration,
    elapsed: Duration,
}

impl BossPrompt {
    pub(crate) const fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn remaining(&self) -> Duration {
        self.deadline.saturating_sub(self.elapsed)
    }

    pub(crate) fn progress(&self) -> f64 {
        if self.deadline.is_zero() {
            1.0
        } else {
            (self.elapsed.as_secs_f64() / self.deadline.as_secs_f64()).clamp(0.0, 1.0)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BossBattleOutcome {
    pub(crate) victory: bool,
    pub(crate) boss: BossKind,
    pub(crate) language: Language,
    pub(crate) difficulty: GameDifficulty,
    pub(crate) score: u64,
    pub(crate) active_time: Duration,
    pub(crate) hearts: u8,
    pub(crate) correct_units: u64,
    pub(crate) attempted_units: u64,
    pub(crate) max_combo: u64,
}

#[derive(Clone, Copy, Debug)]
struct CueState {
    kind: BattleCue,
    elapsed: Duration,
    duration: Duration,
    locking: bool,
}

impl CueState {
    const fn new(kind: BattleCue, duration: Duration, locking: bool) -> Self {
        Self {
            kind,
            elapsed: Duration::ZERO,
            duration,
            locking,
        }
    }

    fn progress(self) -> f64 {
        if self.duration.is_zero() {
            1.0
        } else {
            (self.elapsed.as_secs_f64() / self.duration.as_secs_f64()).clamp(0.0, 1.0)
        }
    }
}

#[derive(Debug)]
struct WardenState {
    locks: u8,
    cast_units: u64,
    cast_elapsed: Duration,
    cast_deadline: Duration,
    core_remaining: Duration,
}

impl WardenState {
    fn new(difficulty: GameDifficulty) -> Self {
        Self {
            locks: 0,
            cast_units: 0,
            cast_elapsed: Duration::ZERO,
            cast_deadline: warden_cast(difficulty, BossPhase::One, 0),
            core_remaining: Duration::ZERO,
        }
    }
}

#[derive(Debug)]
struct ArchonState {
    checksum_units: Vec<u64>,
    canticle_units: u64,
    canticle_elapsed: Duration,
    canticle_deadline: Duration,
}

impl ArchonState {
    fn new(difficulty: GameDifficulty) -> Self {
        Self {
            checksum_units: Vec::with_capacity(3),
            canticle_units: 0,
            canticle_elapsed: Duration::ZERO,
            canticle_deadline: archon_canticle(difficulty, BossPhase::One, 0),
        }
    }
}

#[derive(Debug)]
enum BossPattern {
    Warden(WardenState),
    Queen,
    NullArchon(ArchonState),
}

pub(crate) struct BossBattle {
    boss: BossKind,
    language: Language,
    difficulty: GameDifficulty,
    words: Vec<String>,
    rng: fastrand::Rng,
    next_prompt_id: u64,
    prompts: Vec<BossPrompt>,
    target: Option<u64>,
    input: String,
    input_error: bool,
    health: u64,
    max_health: u64,
    hearts: u8,
    combo: u64,
    max_combo: u64,
    correct_units: u64,
    attempted_units: u64,
    active_time: Duration,
    last_tick: Instant,
    paused: bool,
    viewport_supported: bool,
    phase: BossPhase,
    phase_transitioning: bool,
    cue: Option<CueState>,
    pending_finish: Option<bool>,
    pattern: BossPattern,
    outcome: Option<BossBattleOutcome>,
}

impl BossBattle {
    pub(crate) fn new(
        boss: BossKind,
        language: Language,
        difficulty: GameDifficulty,
        words: Vec<String>,
        seed: u64,
        now: Instant,
    ) -> Result<Self> {
        let words = words
            .into_iter()
            .map(|word| normalize_nfc(&word))
            .collect::<Vec<_>>();
        if words.is_empty()
            || words.iter().any(|word| {
                let width = UnicodeWidthStr::width(word.as_str());
                !(1..=MAX_WORD_WIDTH).contains(&width)
            })
        {
            bail!("boss battle requires playable words");
        }

        let (max_health, pattern) = match boss {
            BossKind::IronWarden => (
                WARDEN_HEALTH[difficulty_slot(difficulty)],
                BossPattern::Warden(WardenState::new(difficulty)),
            ),
            BossKind::ThornQueen => (
                QUEEN_HEALTH[difficulty_slot(difficulty)],
                BossPattern::Queen,
            ),
            BossKind::NullArchon => (
                archon_health(language, difficulty),
                BossPattern::NullArchon(ArchonState::new(difficulty)),
            ),
        };
        let mut battle = Self {
            boss,
            language,
            difficulty,
            words,
            rng: fastrand::Rng::with_seed(seed),
            next_prompt_id: 1,
            prompts: Vec::new(),
            target: None,
            input: String::new(),
            input_error: false,
            health: max_health,
            max_health,
            hearts: 3,
            combo: 0,
            max_combo: 0,
            correct_units: 0,
            attempted_units: 0,
            active_time: Duration::ZERO,
            last_tick: now,
            paused: false,
            viewport_supported: true,
            phase: BossPhase::One,
            phase_transitioning: false,
            cue: Some(CueState::new(BattleCue::Intro, INTRO_DURATION, true)),
            pending_finish: None,
            pattern,
            outcome: None,
        };
        match boss {
            BossKind::IronWarden | BossKind::NullArchon => battle.spawn_prompt(),
            BossKind::ThornQueen => battle.fill_queen_lanes(),
        }
        Ok(battle)
    }

    pub(crate) const fn boss(&self) -> BossKind {
        self.boss
    }

    pub(crate) const fn difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    pub(crate) const fn phase(&self) -> BossPhase {
        self.phase
    }

    pub(crate) const fn health(&self) -> u64 {
        self.health
    }

    pub(crate) const fn max_health(&self) -> u64 {
        self.max_health
    }

    pub(crate) const fn hearts(&self) -> u8 {
        self.hearts
    }

    pub(crate) const fn combo(&self) -> u64 {
        self.combo
    }

    pub(crate) const fn active_time(&self) -> Duration {
        self.active_time
    }

    pub(crate) fn time_remaining(&self) -> Duration {
        BATTLE_LIMIT.saturating_sub(self.active_time)
    }

    pub(crate) fn input(&self) -> &str {
        &self.input
    }

    pub(crate) fn prompts(&self) -> std::slice::Iter<'_, BossPrompt> {
        self.prompts.iter()
    }

    pub(crate) const fn target_id(&self) -> Option<u64> {
        self.target
    }

    pub(crate) fn pattern_view(&self) -> BossPatternView {
        match &self.pattern {
            BossPattern::Warden(state) => BossPatternView::Warden {
                locks: state.locks,
                core_exposed: state.locks == 3,
                cast_progress: duration_progress(state.cast_elapsed, state.cast_deadline),
            },
            BossPattern::Queen => BossPatternView::Queen {
                target_id: self.target,
            },
            BossPattern::NullArchon(state) => BossPatternView::NullArchon {
                checksum: state.checksum_units.len() as u8,
                canticle_progress: duration_progress(
                    state.canticle_elapsed,
                    state.canticle_deadline,
                ),
            },
        }
    }

    pub(crate) fn cue(&self) -> Option<(BattleCue, f64)> {
        self.cue.map(|cue| (cue.kind, cue.progress()))
    }

    pub(crate) const fn is_paused(&self) -> bool {
        self.paused
    }

    pub(crate) const fn outcome(&self) -> Option<&BossBattleOutcome> {
        self.outcome.as_ref()
    }

    pub(crate) fn tick(&mut self, now: Instant) {
        if self.outcome.is_some() || self.paused || !self.viewport_supported {
            self.last_tick = now;
            return;
        }

        let mut elapsed = now.saturating_duration_since(self.last_tick).min(MAX_TICK);
        self.last_tick = now;
        elapsed = self.consume_locking_cue(elapsed);
        if elapsed.is_zero() || self.outcome.is_some() || self.input_locked() {
            return;
        }

        let active = elapsed.min(self.time_remaining());
        self.active_time = self.active_time.saturating_add(active);
        self.advance_nonlocking_cue(active);
        if self.active_time >= BATTLE_LIMIT {
            self.start_finish(false);
            return;
        }

        for prompt in &mut self.prompts {
            prompt.elapsed = prompt.elapsed.saturating_add(active).min(prompt.deadline);
        }
        match self.boss {
            BossKind::IronWarden => self.tick_warden(active),
            BossKind::ThornQueen => self.tick_queen(),
            BossKind::NullArchon => self.tick_archon(active),
        }
    }

    pub(crate) fn toggle_pause(&mut self, now: Instant) -> bool {
        if self.outcome.is_some() || self.pending_finish.is_some() {
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
        if self.outcome.is_some() || self.paused || !self.viewport_supported || self.input_locked()
        {
            return;
        }

        let units = key_units(self.language, &character.to_string()).len() as u64;
        self.attempted_units = self.attempted_units.saturating_add(units);
        let was_empty = self.input.is_empty();
        self.input.push(character);
        if was_empty && self.target.is_none() {
            self.target = self.select_target();
        }
        if !self.input_is_valid() {
            if matches!(&self.pattern, BossPattern::NullArchon(_)) {
                self.combo = 0;
                if let BossPattern::NullArchon(state) = &mut self.pattern {
                    state.checksum_units.pop();
                }
                self.clear_input();
                self.start_cue(BattleCue::Hit, HIT_DURATION, false);
            } else if !self.input_error {
                self.combo = 0;
                self.input_error = true;
            }
            return;
        }
        self.correct_units = self.correct_units.saturating_add(units);

        let input = key_units(self.language, &self.input);
        let complete = self.target.and_then(|id| {
            self.prompts
                .iter()
                .find(|prompt| prompt.id == id)
                .map(|prompt| key_units(self.language, &prompt.text) == input)
        });
        if complete == Some(true) {
            self.complete_target();
        }
    }

    pub(crate) fn backspace(&mut self) -> bool {
        if self.outcome.is_some() || self.paused || !self.viewport_supported || self.input_locked()
        {
            return false;
        }
        let removed = self.input.pop().is_some();
        if self.input.is_empty() {
            self.target = None;
        }
        if self.input_is_valid() {
            self.input_error = false;
        }
        removed
    }

    pub(crate) fn submit_input(&mut self) {
        if self.outcome.is_some() || self.paused || !self.viewport_supported || self.input_locked()
        {
            return;
        }
        self.clear_input();
    }

    pub(crate) fn input_is_valid(&self) -> bool {
        if self.input.is_empty() {
            return true;
        }
        let Some(prompt) = self
            .target
            .and_then(|id| self.prompts.iter().find(|prompt| prompt.id == id))
        else {
            return false;
        };
        key_units(self.language, &prompt.text).starts_with(&key_units(self.language, &self.input))
    }

    fn input_locked(&self) -> bool {
        self.cue.is_some_and(|cue| cue.locking)
    }

    fn select_target(&self) -> Option<u64> {
        let input = key_units(self.language, &self.input);
        self.prompts
            .iter()
            .filter(|prompt| key_units(self.language, &prompt.text).starts_with(&input))
            .max_by(|left, right| {
                left.progress()
                    .total_cmp(&right.progress())
                    .then_with(|| right.id.cmp(&left.id))
            })
            .map(|prompt| prompt.id)
    }

    fn complete_target(&mut self) {
        let Some(index) = self
            .target
            .and_then(|id| self.prompts.iter().position(|prompt| prompt.id == id))
        else {
            return;
        };
        let units = unit_count(self.language, &self.prompts[index].text);
        let damage = match &mut self.pattern {
            BossPattern::Warden(state) => {
                let core_exposed = state.locks == 3;
                if !core_exposed {
                    state.locks = state.locks.saturating_add(1).min(3);
                }
                units.saturating_mul(if core_exposed { 2 } else { 1 })
            }
            BossPattern::Queen => units,
            BossPattern::NullArchon(state) => {
                state.checksum_units.push(units);
                let bonus = if state.checksum_units.len() == 3 {
                    let bonus = state.checksum_units.iter().sum();
                    state.checksum_units.clear();
                    state.canticle_units = 0;
                    state.canticle_elapsed = Duration::ZERO;
                    state.canticle_deadline =
                        archon_canticle(self.difficulty, self.phase, state.canticle_units);
                    bonus
                } else {
                    0
                };
                units.saturating_add(bonus)
            }
        };

        self.combo = self.combo.saturating_add(1);
        self.max_combo = self.max_combo.max(self.combo);
        self.health = self.health.saturating_sub(damage);
        self.prompts.remove(index);
        self.clear_input();

        if self.health == 0 {
            self.start_finish(true);
            return;
        }
        if self.phase == BossPhase::One
            && !self.phase_transitioning
            && self.health <= self.max_health / 2
        {
            self.phase_transitioning = true;
            self.start_cue(BattleCue::PhaseTransition, PHASE_DURATION, true);
        } else {
            self.start_cue(BattleCue::Hit, HIT_DURATION, false);
        }
        match self.boss {
            BossKind::IronWarden | BossKind::NullArchon => self.spawn_prompt(),
            BossKind::ThornQueen => self.fill_queen_lanes(),
        }
    }

    fn tick_warden(&mut self, elapsed: Duration) {
        enum Event {
            None,
            CoreClosed,
            Attack,
        }

        let event = match &mut self.pattern {
            BossPattern::Warden(state) if state.locks == 3 => {
                state.core_remaining = state.core_remaining.saturating_sub(elapsed);
                if state.core_remaining.is_zero() {
                    state.locks = 0;
                    state.cast_units = 0;
                    state.cast_elapsed = Duration::ZERO;
                    state.cast_deadline = warden_cast(self.difficulty, self.phase, 0);
                    Event::CoreClosed
                } else {
                    Event::None
                }
            }
            BossPattern::Warden(state) => {
                state.cast_elapsed = state.cast_elapsed.saturating_add(elapsed);
                if state.cast_elapsed >= state.cast_deadline {
                    state.locks = 0;
                    state.cast_units = 0;
                    state.cast_elapsed = Duration::ZERO;
                    state.cast_deadline = warden_cast(self.difficulty, self.phase, 0);
                    Event::Attack
                } else {
                    Event::None
                }
            }
            BossPattern::Queen | BossPattern::NullArchon(_) => {
                unreachable!("tick_warden requires Warden state")
            }
        };

        match event {
            Event::None => {}
            Event::CoreClosed => {
                self.reset_prompt_deadlines();
                self.restart_pattern_window();
            }
            Event::Attack => {
                self.hearts = self.hearts.saturating_sub(1);
                self.combo = 0;
                self.clear_input();
                self.reset_prompt_deadlines();
                self.restart_pattern_window();
                if self.hearts == 0 {
                    self.start_finish(false);
                } else {
                    self.start_cue(BattleCue::BossAttack, ATTACK_DURATION, true);
                }
            }
        }
    }

    fn tick_queen(&mut self) {
        let expired = self
            .prompts
            .iter()
            .filter(|prompt| prompt.remaining().is_zero())
            .max_by(|left, right| {
                left.progress()
                    .total_cmp(&right.progress())
                    .then_with(|| right.id.cmp(&left.id))
            })
            .map(|prompt| prompt.id);
        let Some(expired) = expired else {
            return;
        };
        if self.target == Some(expired) {
            self.clear_input();
        }
        self.prompts.retain(|prompt| prompt.id != expired);
        self.hearts = self.hearts.saturating_sub(1);
        self.combo = 0;
        self.fill_queen_lanes();
        if self.hearts == 0 {
            self.start_finish(false);
        } else {
            self.start_cue(BattleCue::BossAttack, ATTACK_DURATION, true);
        }
    }

    fn tick_archon(&mut self, elapsed: Duration) {
        let expired = match &mut self.pattern {
            BossPattern::NullArchon(state) => {
                state.canticle_elapsed = state.canticle_elapsed.saturating_add(elapsed);
                if state.canticle_elapsed >= state.canticle_deadline {
                    state.checksum_units.clear();
                    state.canticle_units = 0;
                    state.canticle_elapsed = Duration::ZERO;
                    state.canticle_deadline =
                        archon_canticle(self.difficulty, self.phase, state.canticle_units);
                    true
                } else {
                    false
                }
            }
            BossPattern::Warden(_) | BossPattern::Queen => {
                unreachable!("tick_archon requires Null Archon state")
            }
        };
        if !expired {
            return;
        }

        self.hearts = self.hearts.saturating_sub(1);
        self.combo = 0;
        self.clear_input();
        self.reset_prompt_deadlines();
        self.restart_pattern_window();
        if self.hearts == 0 {
            self.start_finish(false);
        } else {
            self.start_cue(BattleCue::BossAttack, ATTACK_DURATION, true);
        }
    }

    fn spawn_prompt(&mut self) {
        let text = self.words[self.rng.usize(..self.words.len())].clone();
        let units = unit_count(self.language, &text);
        let deadline = prompt_window(self.language, self.difficulty, &text);
        self.prompts.push(BossPrompt {
            id: self.next_prompt_id,
            text,
            deadline,
            elapsed: Duration::ZERO,
        });
        self.next_prompt_id = self.next_prompt_id.saturating_add(1);
        match &mut self.pattern {
            BossPattern::Warden(state) if state.locks == 3 => {
                if state.core_remaining.is_zero() {
                    state.core_remaining = warden_core(self.difficulty, self.phase, units);
                }
            }
            BossPattern::Warden(state) => {
                state.cast_units = state.cast_units.saturating_add(units);
                state.cast_deadline = warden_cast(self.difficulty, self.phase, state.cast_units);
            }
            BossPattern::NullArchon(state) => {
                state.canticle_units = state.canticle_units.saturating_add(units);
                state.canticle_deadline =
                    archon_canticle(self.difficulty, self.phase, state.canticle_units);
            }
            BossPattern::Queen => {}
        }
    }

    fn fill_queen_lanes(&mut self) {
        let desired = if self.phase == BossPhase::Two { 3 } else { 2 };
        while self.prompts.len() < desired {
            let active_initials = self
                .prompts
                .iter()
                .filter_map(|prompt| key_units(self.language, &prompt.text).first().copied())
                .collect::<Vec<_>>();
            let candidates = self
                .words
                .iter()
                .enumerate()
                .filter(|(_, word)| {
                    !self.prompts.iter().any(|prompt| prompt.text == **word)
                        && key_units(self.language, word)
                            .first()
                            .is_some_and(|initial| !active_initials.contains(initial))
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if candidates.is_empty() {
                break;
            }
            let index = candidates[self.rng.usize(..candidates.len())];
            let text = self.words[index].clone();
            let lane = self.prompts.len();
            let deadline = prompt_window(self.language, self.difficulty, &text)
                .mul_f64(desired as f64)
                .saturating_add(
                    QUEEN_STAGGER[difficulty_slot(self.difficulty)].mul_f64(lane as f64),
                );
            self.prompts.push(BossPrompt {
                id: self.next_prompt_id,
                text,
                deadline,
                elapsed: Duration::ZERO,
            });
            self.next_prompt_id = self.next_prompt_id.saturating_add(1);
        }
    }

    fn reset_prompt_deadlines(&mut self) {
        for prompt in &mut self.prompts {
            prompt.elapsed = Duration::ZERO;
        }
    }

    fn restart_pattern_window(&mut self) {
        let units = self
            .prompts
            .first()
            .map_or(0, |prompt| unit_count(self.language, &prompt.text));
        match &mut self.pattern {
            BossPattern::Warden(state) if state.locks < 3 => {
                state.cast_units = units;
                state.cast_deadline = warden_cast(self.difficulty, self.phase, state.cast_units);
            }
            BossPattern::NullArchon(state) => {
                state.canticle_units = units;
                state.canticle_deadline =
                    archon_canticle(self.difficulty, self.phase, state.canticle_units);
            }
            BossPattern::Warden(_) | BossPattern::Queen => {}
        }
    }

    fn clear_input(&mut self) {
        self.input.clear();
        self.target = None;
        self.input_error = false;
    }

    fn start_cue(&mut self, kind: BattleCue, duration: Duration, locking: bool) {
        self.cue = Some(CueState::new(kind, duration, locking));
    }

    fn start_finish(&mut self, victory: bool) {
        if self.pending_finish.is_some() || self.outcome.is_some() {
            return;
        }
        self.pending_finish = Some(victory);
        self.start_cue(
            if victory {
                BattleCue::Victory
            } else {
                BattleCue::Defeat
            },
            FINISH_DURATION,
            true,
        );
    }

    fn consume_locking_cue(&mut self, elapsed: Duration) -> Duration {
        let Some(cue) = self.cue.as_mut().filter(|cue| cue.locking) else {
            return elapsed;
        };
        let consumed = elapsed.min(cue.duration.saturating_sub(cue.elapsed));
        cue.elapsed = cue.elapsed.saturating_add(consumed);
        let finished = cue.elapsed >= cue.duration;
        if finished {
            self.finish_cue();
        }
        elapsed.saturating_sub(consumed)
    }

    fn advance_nonlocking_cue(&mut self, elapsed: Duration) {
        let Some(cue) = self.cue.as_mut().filter(|cue| !cue.locking) else {
            return;
        };
        cue.elapsed = cue.elapsed.saturating_add(elapsed);
        if cue.elapsed >= cue.duration {
            self.cue = None;
        }
    }

    fn finish_cue(&mut self) {
        let Some(cue) = self.cue.take() else {
            return;
        };
        match cue.kind {
            BattleCue::PhaseTransition => {
                self.phase = BossPhase::Two;
                self.phase_transitioning = false;
                let prompt_units = self
                    .prompts
                    .first()
                    .map_or(0, |prompt| unit_count(self.language, &prompt.text));
                match &mut self.pattern {
                    BossPattern::Warden(state) if state.locks == 3 => {
                        state.core_remaining = state.core_remaining.min(warden_core(
                            self.difficulty,
                            self.phase,
                            prompt_units,
                        ));
                    }
                    BossPattern::Warden(state) => {
                        state.cast_deadline =
                            warden_cast(self.difficulty, self.phase, state.cast_units);
                    }
                    BossPattern::Queen => self.fill_queen_lanes(),
                    BossPattern::NullArchon(state) => {
                        state.canticle_deadline =
                            archon_canticle(self.difficulty, self.phase, state.canticle_units);
                        state.canticle_elapsed =
                            state.canticle_elapsed.min(state.canticle_deadline);
                    }
                }
            }
            BattleCue::Victory | BattleCue::Defeat => self.publish_outcome(),
            BattleCue::Intro | BattleCue::Hit | BattleCue::BossAttack => {}
        }
    }

    fn publish_outcome(&mut self) {
        let Some(victory) = self.pending_finish.take() else {
            return;
        };
        let attempted_units = u128::from(self.attempted_units);
        let accuracy_basis_points = (u128::from(self.correct_units) * 10_000 + attempted_units / 2)
            .checked_div(attempted_units)
            .and_then(|basis_points| u64::try_from(basis_points).ok())
            .unwrap_or(10_000);
        let score = u64::from(victory).saturating_mul(10_000)
            + self.time_remaining().as_secs().saturating_mul(100)
            + u64::from(self.hearts).saturating_mul(1_000)
            + accuracy_basis_points
            + self.max_combo.saturating_mul(10);
        self.outcome = Some(BossBattleOutcome {
            victory,
            boss: self.boss,
            language: self.language,
            difficulty: self.difficulty,
            score,
            active_time: self.active_time,
            hearts: self.hearts,
            correct_units: self.correct_units,
            attempted_units: self.attempted_units,
            max_combo: self.max_combo,
        });
    }
}

fn difficulty_slot(difficulty: GameDifficulty) -> usize {
    difficulty.index()
}

fn prompt_window(language: Language, difficulty: GameDifficulty, word: &str) -> Duration {
    unit_window(difficulty, unit_count(language, word))
}

fn unit_window(difficulty: GameDifficulty, units: u64) -> Duration {
    const TARGET_KPM: [f64; 4] = [180.0, 300.0, 420.0, 540.0];
    const REACTION: [f64; 4] = [1.8, 1.4, 1.1, 0.9];
    const GRACE: [f64; 4] = [1.7, 2.0, 1.65, 1.55];
    let index = difficulty_slot(difficulty);
    let typing = units as f64 * 60.0 / TARGET_KPM[index];
    Duration::from_secs_f64(REACTION[index] + typing * GRACE[index])
}

fn pattern_window(
    floor: Duration,
    difficulty: GameDifficulty,
    phase: BossPhase,
    units: u64,
) -> Duration {
    floor
        .max(unit_window(difficulty, units))
        .mul_f64(if phase == BossPhase::Two { 0.8 } else { 1.0 })
}

fn warden_cast(difficulty: GameDifficulty, phase: BossPhase, units: u64) -> Duration {
    pattern_window(
        WARDEN_CAST[difficulty_slot(difficulty)],
        difficulty,
        phase,
        units,
    )
}

fn warden_core(difficulty: GameDifficulty, phase: BossPhase, units: u64) -> Duration {
    pattern_window(
        WARDEN_CORE[difficulty_slot(difficulty)],
        difficulty,
        phase,
        units,
    )
}

fn archon_canticle(difficulty: GameDifficulty, phase: BossPhase, units: u64) -> Duration {
    pattern_window(
        ARCHON_CANTICLE[difficulty_slot(difficulty)],
        difficulty,
        phase,
        units,
    )
}

fn archon_health(language: Language, difficulty: GameDifficulty) -> u64 {
    match language {
        Language::Ko => ARCHON_HEALTH_KO[difficulty_slot(difficulty)],
        Language::En => ARCHON_HEALTH_EN[difficulty_slot(difficulty)],
    }
}

fn duration_progress(elapsed: Duration, total: Duration) -> f64 {
    if total.is_zero() {
        1.0
    } else {
        (elapsed.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests;

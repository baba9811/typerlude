use super::{
    App, GameOptions, InputEvent, Key, KeyInput, KeyKind, KeyModifiers, ModeOptions,
    QUICK_COUNT_PRESETS, QUICK_TIME_PRESETS, QuickOptions, QuickSource, Screen,
    TEST_DURATION_PRESETS, key_stages,
};
use crate::{
    config::Settings,
    game::{GameKind, boss_battle::BossKind},
    i18n::{TextKey, text},
    model::{Difficulty, Language, PracticeKind},
    stats::Range,
};
use anyhow::Result;
use std::time::{Duration, Instant};

impl App {
    pub fn open(&mut self, screen: Screen) {
        if screen == self.screen {
            self.focus = 0;
            return;
        }
        self.remember_focus();

        if screen == Screen::Help {
            self.parent_before_help = Some(self.parent);
            self.parent = self.screen;
            self.screen = Screen::Help;
            self.focus = 0;
            return;
        }

        let prior = if self.screen == Screen::Help {
            self.parent
        } else {
            self.screen
        };
        self.parent = if prior == screen { Screen::Home } else { prior };
        self.parent_before_help = None;
        self.screen = screen;
        self.focus = 0;
    }

    pub fn handle_event(&mut self, event: InputEvent, now: Instant) -> Result<()> {
        let update_notice_was_visible = self.update_notice.is_some();
        let quit = matches!(
            &event,
            InputEvent::Key(key)
                if matches!(key.key, Key::Char('c' | 'C')) && key.modifiers.control
        );
        let was_practicing = self.screen == Screen::Practice;
        let was_playing = self.screen == Screen::Game;
        let tick = self.tick(now);
        if quit {
            self.quit = true;
            return tick;
        }
        tick?;
        if was_practicing && self.screen != Screen::Practice {
            return Ok(());
        }
        if was_playing && self.screen != Screen::Game {
            return Ok(());
        }
        match event {
            InputEvent::Paste if self.screen == Screen::Practice => {
                if let Some(active) = self.practice.as_mut()
                    && !(active.kind() == PracticeKind::Test && active.leave_confirmation())
                {
                    active.status = Some((
                        text(self.settings.ui_language, TextKey::PasteIgnored).into(),
                        now.checked_add(Duration::from_secs(3)).unwrap_or(now),
                    ));
                }
            }
            InputEvent::Key(key) => {
                self.handle_key(key, now, update_notice_was_visible)?;
            }
            _ => {}
        }
        self.tick(now)
    }

    fn handle_key(
        &mut self,
        key: KeyInput,
        now: Instant,
        update_notice_was_visible: bool,
    ) -> Result<()> {
        if self.screen == Screen::Practice {
            return self.handle_practice_key(key, now);
        }
        if self.screen == Screen::Game {
            return self.handle_game_key(key, now);
        }

        if key.is_plain_q_command() {
            self.escape();
            return Ok(());
        }
        if key.key == Key::Char('?')
            && matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT)
        {
            self.open(Screen::Help);
            return Ok(());
        }
        if matches!(self.screen, Screen::Home | Screen::Result)
            && key.kind == KeyKind::Press
            && key.modifiers == KeyModifiers::NONE
            && update_notice_was_visible
            && self.update_notice.is_some()
        {
            match key.key {
                Key::Char('l') => {
                    self.update_notice = None;
                    return Ok(());
                }
                Key::Char('s') => {
                    let latest = self
                        .update_notice
                        .as_ref()
                        .map(|notice| notice.latest.to_string())
                        .unwrap_or_default();
                    if self
                        .change_settings(|settings| settings.skipped_update_version = latest)
                        .is_ok()
                    {
                        self.update_notice = None;
                    }
                    return Ok(());
                }
                _ => {}
            }
        }

        match key.key {
            Key::Esc => self.escape(),
            Key::Tab if self.screen != Screen::Practice => self.move_tab_focus(1),
            Key::BackTab if self.screen != Screen::Practice => self.move_tab_focus(-1),
            Key::Down if self.screen != Screen::Practice => self.move_focus(1),
            Key::Up if self.screen != Screen::Practice => {
                self.move_focus(-1);
            }
            Key::Char('j')
                if self.screen != Screen::Practice && key.modifiers == KeyModifiers::NONE =>
            {
                self.move_focus(1);
            }
            Key::Char('k')
                if self.screen != Screen::Practice && key.modifiers == KeyModifiers::NONE =>
            {
                self.move_focus(-1);
            }
            Key::Left if key.modifiers == KeyModifiers::NONE => self.adjust(-1),
            Key::Right if key.modifiers == KeyModifiers::NONE => self.adjust(1),
            Key::Char('d')
                if self.screen == Screen::ContentDetail
                    && key.kind == KeyKind::Press
                    && key.modifiers == KeyModifiers::NONE =>
            {
                self.disable_selected_content();
            }
            Key::Enter if key.modifiers == KeyModifiers::NONE => self.enter(now)?,
            Key::Char('r')
                if self.screen == Screen::Result && key.modifiers == KeyModifiers::NONE =>
            {
                if let Some(request) = self.retry_request.clone() {
                    let stream = self.retry_stream.clone();
                    let long_metadata = self.retry_long_metadata.clone();
                    self.start_mode(request, now)?;
                    if let Some(stream) = stream {
                        if let Some(active) = self.practice.as_mut() {
                            active.stream = Some(stream.clone());
                        }
                        self.retry_stream = Some(stream);
                    }
                    if let Some(metadata) = long_metadata {
                        if let Some(active) = self.practice.as_mut() {
                            active.long_metadata = Some(metadata.clone());
                        }
                        self.retry_long_metadata = Some(metadata);
                    }
                }
            }
            Key::Char('n')
                if self.screen == Screen::Result && key.modifiers == KeyModifiers::NONE =>
            {
                self.start_next(now)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn escape(&mut self) {
        self.content_disable_confirmation = false;
        self.remember_focus();
        match self.screen {
            Screen::Home => self.quit = true,
            Screen::Result => self.return_home(),
            Screen::GameResult => self.return_to_games(),
            Screen::Help => {
                let destination = self.parent;
                let restored_parent = self.parent_before_help.take().unwrap_or(Screen::Home);
                self.parent = if restored_parent == destination {
                    Screen::Home
                } else {
                    restored_parent
                };
                self.restore_focus(destination);
            }
            _ => {
                let destination = if self.parent == self.screen {
                    Screen::Home
                } else {
                    self.parent
                };
                self.parent = Screen::Home;
                self.parent_before_help = None;
                self.restore_focus(destination);
            }
        }
    }

    pub(super) fn return_home(&mut self) {
        self.remember_focus();
        self.parent = Screen::Home;
        self.parent_before_help = None;
        self.restore_focus(Screen::Home);
    }

    pub(super) fn remember_focus(&mut self) {
        self.focus_memory.insert(self.screen, self.focus);
    }

    fn restore_focus(&mut self, screen: Screen) {
        self.screen = screen;
        self.focus = self
            .focus_memory
            .get(&screen)
            .copied()
            .unwrap_or(0)
            .min(self.focus_count().saturating_sub(1));
    }

    fn focus_count(&self) -> usize {
        match self.screen {
            Screen::Home => 11,
            Screen::Games => GameKind::ALL.len(),
            Screen::GameOptions => match self.game_options.kind {
                GameKind::WordRain => 3,
                GameKind::BossBattle => 6,
            },
            Screen::ModeOptions => match self.mode_options.kind {
                PracticeKind::Quick | PracticeKind::Key => 5,
                PracticeKind::Words => 3,
                PracticeKind::Sentence => 2,
                PracticeKind::Test => 4,
                PracticeKind::Long => self
                    .long_items(self.mode_options.language, None)
                    .len()
                    .saturating_add(1),
            },
            Screen::Stats => 5,
            Screen::History => 3,
            Screen::Goals => 4,
            Screen::Content => self.content_packs().len().max(1),
            Screen::ContentDetail => self
                .content_detail_pack()
                .map_or(1, |pack| pack.provenance.len().max(1)),
            Screen::Settings => 9,
            Screen::Themes => self.themes.ids().count().max(1),
            _ => 1,
        }
    }

    fn move_focus(&mut self, delta: isize) {
        let count = self.focus_count();
        self.focus = if delta < 0 {
            (self.focus + count - 1) % count
        } else {
            (self.focus + 1) % count
        };
        if self.screen == Screen::ModeOptions
            && self.mode_options.kind == PracticeKind::Long
            && self.focus != 0
        {
            self.mode_options.long_selection = self.focus - 1;
        }
        if self.screen == Screen::GameOptions
            && self.game_options.kind == GameKind::BossBattle
            && self.focus < BossKind::ALL.len()
        {
            self.game_options.boss = BossKind::ALL[self.focus];
            self.clamp_boss_difficulty();
            self.game_options.error = None;
        }
    }

    fn move_tab_focus(&mut self, delta: isize) {
        if self.screen == Screen::GameOptions && self.game_options.kind == GameKind::BossBattle {
            self.focus = if delta < 0 {
                match self.focus {
                    0..=2 => 5,
                    3 => self.game_options.boss.index(),
                    focus => focus - 1,
                }
            } else {
                match self.focus {
                    0..=2 => 3,
                    5 => self.game_options.boss.index(),
                    focus => focus + 1,
                }
            };
            return;
        }
        self.move_focus(delta);
    }

    fn adjust(&mut self, delta: isize) {
        if self.screen == Screen::ModeOptions {
            self.adjust_mode_options(delta);
            return;
        }
        if self.screen == Screen::GameOptions {
            self.game_options.error = None;
            match self.game_options.kind {
                GameKind::WordRain => match self.focus {
                    0 => self.game_options.language = other_language(self.game_options.language),
                    1 => {
                        self.game_options.difficulty =
                            cycle_game_difficulty(self.game_options.difficulty, delta);
                    }
                    _ => {}
                },
                GameKind::BossBattle => match self.focus {
                    3 => self.game_options.language = other_language(self.game_options.language),
                    4 => {
                        self.game_options.difficulty = cycle_boss_difficulty(
                            &self.settings,
                            self.game_options.boss,
                            self.game_options.difficulty,
                            delta,
                        );
                    }
                    _ => {}
                },
            }
            return;
        }
        match (self.screen, self.focus) {
            (Screen::Stats | Screen::History, 0) => {
                self.stats_range = cycle_range(self.stats_range, delta);
            }
            (Screen::Stats | Screen::History, 1) => {
                self.stats_language = other_language(self.stats_language);
            }
            (Screen::Stats | Screen::History, 2) => {
                self.stats_mode = cycle_mode(self.stats_mode, delta);
            }
            (Screen::Goals, 0) => {
                let value = adjusted(self.settings.target_kpm, delta, 10, 1, 5_000);
                let _ = self.set_target_kpm(value);
            }
            (Screen::Goals, 1) => {
                let value = adjusted(self.settings.target_wpm, delta, 5, 1, 5_000);
                let _ = self.set_target_wpm(value);
            }
            (Screen::Goals, 2) => {
                let value = adjusted_decimal(self.settings.target_accuracy, delta, 0.5, 1.0, 100.0);
                let _ = self.set_target_accuracy(value);
            }
            (Screen::Goals, 3) => {
                let value = adjusted(self.settings.daily_minutes, delta, 5, 1, 1_440);
                let _ = self.set_daily_minutes(value);
            }
            (Screen::Settings, _) => self.activate_setting(),
            _ => {}
        }
    }

    fn clamp_boss_difficulty(&mut self) {
        if self
            .settings
            .boss_difficulty_is_unlocked(self.game_options.boss, self.game_options.difficulty)
        {
            return;
        }
        self.game_options.difficulty = [Difficulty::Hard, Difficulty::Medium, Difficulty::Easy]
            .into_iter()
            .find(|difficulty| {
                self.settings
                    .boss_difficulty_is_unlocked(self.game_options.boss, *difficulty)
            })
            .unwrap_or(Difficulty::Easy);
    }

    fn adjust_mode_options(&mut self, delta: isize) {
        if self.focus == 0 {
            self.mode_options.language = other_language(self.mode_options.language);
            match self.mode_options.kind {
                PracticeKind::Key => {
                    self.mode_options.key_stage = self
                        .mode_options
                        .key_stage
                        .min(key_stages(self.mode_options.language).len() as u8);
                }
                PracticeKind::Long => {
                    let item_count = self.long_items(self.mode_options.language, None).len();
                    self.mode_options.long_selection = self
                        .mode_options
                        .long_selection
                        .min(item_count.saturating_sub(1));
                    self.focus = self.focus.min(item_count);
                }
                PracticeKind::Test => {
                    self.mode_options.test_selection = self
                        .mode_options
                        .test_selection
                        .min(self.long_items(self.mode_options.language, None).len());
                }
                _ => {}
            }
            return;
        }

        match (self.mode_options.kind, self.focus) {
            (PracticeKind::Quick, 1) => {
                self.mode_options.quick_source = match self.mode_options.quick_source {
                    QuickSource::Words => QuickSource::Quote,
                    QuickSource::Quote => QuickSource::Words,
                };
            }
            (PracticeKind::Quick, 2) => {
                self.mode_options.quick_items = !self.mode_options.quick_items;
            }
            (PracticeKind::Quick, 3) => {
                let presets = if self.mode_options.quick_items {
                    QUICK_COUNT_PRESETS.len()
                } else {
                    QUICK_TIME_PRESETS.len()
                };
                self.mode_options.quick_preset =
                    cycle_index(self.mode_options.quick_preset, presets, delta);
            }
            (PracticeKind::Key, 1) => {
                self.mode_options.key_stage = (cycle_index(
                    usize::from(self.mode_options.key_stage.saturating_sub(1)),
                    key_stages(self.mode_options.language).len(),
                    delta,
                ) + 1) as u8;
            }
            (PracticeKind::Key, 2) => self.mode_options.key_random = !self.mode_options.key_random,
            (PracticeKind::Key, 3) => {
                self.mode_options.key_weak_repeat = !self.mode_options.key_weak_repeat;
            }
            (PracticeKind::Words, 1) => {
                self.mode_options.word_difficulty =
                    cycle_difficulty(self.mode_options.word_difficulty, delta);
            }
            (PracticeKind::Test, 1) => {
                self.mode_options.test_preset = cycle_index(
                    self.mode_options.test_preset,
                    TEST_DURATION_PRESETS.len(),
                    delta,
                );
            }
            (PracticeKind::Test, 2) => {
                self.mode_options.test_selection = cycle_index(
                    self.mode_options.test_selection,
                    self.long_items(self.mode_options.language, None).len() + 1,
                    delta,
                );
            }
            _ => {}
        }
    }

    fn enter(&mut self, now: Instant) -> Result<()> {
        match self.screen {
            Screen::Home => {
                let kinds = [
                    PracticeKind::Quick,
                    PracticeKind::Key,
                    PracticeKind::Words,
                    PracticeKind::Sentence,
                    PracticeKind::Long,
                    PracticeKind::Test,
                ];
                match self.focus {
                    0..=5 => {
                        self.mode_options =
                            ModeOptions::new(kinds[self.focus], self.settings.language);
                        self.open(Screen::ModeOptions);
                    }
                    6 => self.open(Screen::Games),
                    7 => self.open(Screen::Stats),
                    8 => self.open(Screen::Goals),
                    9 => self.open(Screen::Content),
                    10 => self.open(Screen::Settings),
                    _ => {}
                }
            }
            Screen::ModeOptions => {
                let options = self.mode_options.clone();
                match (options.kind, self.focus) {
                    (PracticeKind::Quick, 4) => self.start_quick(
                        QuickOptions::new(
                            options.language,
                            options.quick_source,
                            options.quick_stop(),
                        )?,
                        fastrand::u64(..),
                        now,
                    )?,
                    (PracticeKind::Key, 4) => self.start_key(
                        options.language,
                        options.key_stage,
                        options.key_random,
                        options.key_weak_repeat,
                        fastrand::u64(..),
                        now,
                    )?,
                    (PracticeKind::Words, 2) => self.start_words(
                        options.language,
                        options.word_difficulty,
                        fastrand::u64(..),
                        now,
                    )?,
                    (PracticeKind::Sentence, 1) => {
                        self.start_sentence(options.language, fastrand::u64(..), now)?;
                    }
                    (PracticeKind::Long, focus) if focus != 0 => {
                        let item_id = self
                            .long_items(options.language, None)
                            .get(options.long_selection)
                            .map(|item| item.id.clone());
                        if let Some(item_id) = item_id {
                            self.start_long(&item_id, now)?;
                        }
                    }
                    (PracticeKind::Test, 3) => {
                        let items = self.long_items(options.language, None);
                        let item_id = options
                            .test_selection
                            .checked_sub(1)
                            .and_then(|index| items.get(index))
                            .map(|item| item.id.clone());
                        self.start_test(
                            options.language,
                            Some(TEST_DURATION_PRESETS[options.test_preset]),
                            item_id.as_deref(),
                            fastrand::u64(..),
                            now,
                        )?;
                    }
                    _ => self.adjust(1),
                }
            }
            Screen::Games => {
                if let Some(kind) = GameKind::ALL.get(self.focus).copied() {
                    self.game_options = GameOptions::new(kind, self.settings.language);
                    self.open(Screen::GameOptions);
                }
            }
            Screen::GameOptions => match (self.game_options.kind, self.focus) {
                (GameKind::WordRain, 2) => {
                    self.start_word_rain_with_seed(fastrand::u64(..), now)?;
                }
                (GameKind::WordRain, _) => self.adjust(1),
                (GameKind::BossBattle, 5) => {
                    self.start_boss_battle_with_seed(fastrand::u64(..), now)?;
                }
                (GameKind::BossBattle, 3 | 4) => self.adjust(1),
                (GameKind::BossBattle, _) => {}
            },
            Screen::GameResult => match self.game_options.kind {
                GameKind::WordRain => self.start_word_rain_with_seed(fastrand::u64(..), now)?,
                GameKind::BossBattle => {
                    self.start_boss_battle_with_seed(fastrand::u64(..), now)?;
                }
            },
            Screen::Stats => match self.focus {
                0..=2 => self.adjust(1),
                3 => self.open(Screen::History),
                4 => self.open(Screen::WeakKeys),
                _ => {}
            },
            Screen::History => self.adjust(1),
            Screen::Goals => self.adjust(1),
            Screen::Content => {
                if let Some(pack) = self.content_packs().get(self.focus) {
                    self.selected_content_pack = Some(pack.id.clone());
                    self.content_disable_confirmation = false;
                    self.open(Screen::ContentDetail);
                }
            }
            Screen::Settings => self.activate_setting(),
            Screen::Themes => {
                let id = self.themes.ids().nth(self.focus).map(str::to_owned);
                if let Some(id) = id
                    && self.select_theme(&id).is_ok()
                {
                    self.escape();
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn activate_setting(&mut self) {
        match self.focus {
            0 => {
                let language = other_language(self.settings.language);
                if self
                    .change_settings(|settings| settings.language = language)
                    .is_ok()
                {
                    self.stats_language = language;
                }
            }
            1 => {
                let language = other_language(self.settings.ui_language);
                let _ = self.change_settings(|settings| settings.ui_language = language);
            }
            2 => self.open(Screen::Themes),
            3 => {
                let value = !self.settings.show_keyboard;
                let _ = self.change_settings(|settings| settings.show_keyboard = value);
            }
            4 => {
                let value = !self.settings.show_finger_guide;
                let _ = self.change_settings(|settings| settings.show_finger_guide = value);
            }
            5 => {
                let value = !self.settings.show_live_speed;
                let _ = self.change_settings(|settings| settings.show_live_speed = value);
            }
            6 => {
                let value = !self.settings.show_accuracy;
                let _ = self.change_settings(|settings| settings.show_accuracy = value);
            }
            7 => {
                let value = !self.settings.adaptive;
                let _ = self.change_settings(|settings| settings.adaptive = value);
            }
            8 => {
                let value = !self.settings.check_updates;
                let _ = self.change_settings(|settings| settings.check_updates = value);
            }
            _ => {}
        }
    }
}

const fn other_language(language: Language) -> Language {
    match language {
        Language::Ko => Language::En,
        Language::En => Language::Ko,
    }
}

fn cycle_range(range: Range, delta: isize) -> Range {
    const VALUES: [Range; 4] = [Range::Days7, Range::Days30, Range::Days90, Range::All];
    let index = VALUES.iter().position(|value| *value == range).unwrap_or(0);
    VALUES[cycle_index(index, VALUES.len(), delta)]
}

fn cycle_mode(mode: Option<PracticeKind>, delta: isize) -> Option<PracticeKind> {
    const VALUES: [Option<PracticeKind>; 7] = [
        None,
        Some(PracticeKind::Quick),
        Some(PracticeKind::Key),
        Some(PracticeKind::Words),
        Some(PracticeKind::Sentence),
        Some(PracticeKind::Long),
        Some(PracticeKind::Test),
    ];
    let index = VALUES.iter().position(|value| *value == mode).unwrap_or(0);
    VALUES[cycle_index(index, VALUES.len(), delta)]
}

fn cycle_game_difficulty(difficulty: Difficulty, delta: isize) -> Difficulty {
    const VALUES: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard];
    let index = VALUES
        .iter()
        .position(|value| *value == difficulty)
        .unwrap_or(1);
    VALUES[cycle_index(index, VALUES.len(), delta)]
}

fn cycle_boss_difficulty(
    settings: &Settings,
    boss: BossKind,
    difficulty: Difficulty,
    delta: isize,
) -> Difficulty {
    let values = [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard]
        .into_iter()
        .filter(|value| settings.boss_difficulty_is_unlocked(boss, *value))
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Difficulty::Easy;
    }
    let index = values
        .iter()
        .position(|value| *value == difficulty)
        .unwrap_or_default();
    values[cycle_index(index, values.len(), delta)]
}

fn cycle_index(index: usize, len: usize, delta: isize) -> usize {
    if delta < 0 {
        (index + len - 1) % len
    } else {
        (index + 1) % len
    }
}

fn adjusted(value: u32, delta: isize, step: u32, minimum: u32, maximum: u32) -> u32 {
    let value = value.clamp(minimum, maximum);
    let next = if delta < 0 {
        if value.is_multiple_of(step) {
            value.saturating_sub(step)
        } else {
            value / step * step
        }
    } else if delta > 0 {
        (value / step * step).saturating_add(step)
    } else {
        value
    };
    next.clamp(minimum, maximum)
}

fn adjusted_decimal(value: f64, delta: isize, step: f64, minimum: f64, maximum: f64) -> f64 {
    let units = value / step;
    let next = if delta < 0 {
        units.ceil() - 1.0
    } else if delta > 0 {
        units.floor() + 1.0
    } else {
        units
    };
    (next * step).clamp(minimum, maximum)
}

fn cycle_difficulty(difficulty: Difficulty, delta: isize) -> Difficulty {
    let values = [
        Difficulty::Easy,
        Difficulty::Medium,
        Difficulty::Hard,
        Difficulty::Mixed,
    ];
    let index = values
        .iter()
        .position(|value| *value == difficulty)
        .unwrap_or_default();
    values[cycle_index(index, values.len(), delta)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Settings, content::ContentCatalog, game::boss_battle::BossKind, storage::AppPaths,
        theme::ThemeCatalog,
    };

    fn fixture() -> App {
        App::new(
            Settings::default(),
            AppPaths::from_override(std::env::temp_dir().join(format!(
                "typerlude-boss-navigation-{}-{}",
                std::process::id(),
                fastrand::u64(..)
            ))),
            ContentCatalog::load_builtins().unwrap(),
            ThemeCatalog::load_builtins().unwrap(),
            Vec::new(),
            Vec::new(),
        )
    }

    #[test]
    fn boss_options_use_six_positions_and_preview_locked_roster_rows() {
        let mut app = fixture();
        app.screen = Screen::Games;
        app.focus = 1;

        app.enter(Instant::now()).unwrap();
        assert_eq!(app.screen, Screen::GameOptions);
        assert_eq!(app.game_options.kind, GameKind::BossBattle);
        assert_eq!(app.focus_count(), 6);

        app.move_focus(1);
        assert_eq!(app.focus, 1);
        assert_eq!(app.game_options.boss, BossKind::ThornQueen);
        assert!(!app.settings.boss_is_unlocked(BossKind::ThornQueen));
    }

    #[test]
    fn boss_difficulty_navigation_skips_locked_values() {
        let mut app = fixture();
        app.screen = Screen::GameOptions;
        app.game_options = GameOptions::new(GameKind::BossBattle, Language::En);
        app.settings
            .record_boss_clear(BossKind::IronWarden, Language::En, Difficulty::Easy, 1);
        app.focus = 4;

        app.adjust(1);
        assert_eq!(app.game_options.difficulty, Difficulty::Medium);
        app.adjust(1);
        assert_eq!(app.game_options.difficulty, Difficulty::Easy);
    }

    #[test]
    fn tab_leaves_the_boss_roster_without_changing_the_preview() {
        let mut app = fixture();
        app.screen = Screen::GameOptions;
        app.game_options = GameOptions::new(GameKind::BossBattle, Language::En);
        app.game_options.boss = BossKind::ThornQueen;
        app.focus = 1;

        app.handle_event(
            InputEvent::Key(KeyInput {
                key: Key::Tab,
                modifiers: KeyModifiers::NONE,
                kind: KeyKind::Press,
            }),
            Instant::now(),
        )
        .unwrap();

        assert_eq!(app.focus, 3);
        assert_eq!(app.game_options.boss, BossKind::ThornQueen);
    }

    #[test]
    fn starting_a_locked_boss_stays_on_options_with_an_error() {
        let mut app = fixture();
        app.screen = Screen::GameOptions;
        app.game_options = GameOptions::new(GameKind::BossBattle, Language::En);
        app.game_options.boss = BossKind::NullArchon;
        app.focus = 5;

        app.enter(Instant::now()).unwrap();

        assert_eq!(app.screen, Screen::GameOptions);
        assert!(app.active_game.is_none());
        assert_eq!(app.game_options.error.as_deref(), Some("Boss locked"));
    }
}

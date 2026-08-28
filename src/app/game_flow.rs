use super::{
    ActiveBossBattle, ActiveGame, ActiveWordRain, App, BossBattleResult, Key, KeyInput, KeyKind,
    KeyModifiers, Screen, StoredGameResult, WordRainResult,
};
use crate::{
    content::ContentKind,
    game::{
        GameKind,
        boss_battle::{BossBattle, BossBattleOutcome},
        word_rain::{WordRain, WordRainOutcome},
    },
    i18n::{TextKey, text},
    typing::normalize_nfc,
};
use anyhow::Result;
use std::{collections::HashSet, time::Instant};
use unicode_width::UnicodeWidthStr;

enum FinishedGame {
    WordRain(WordRainOutcome),
    BossBattle(BossBattleOutcome),
}

impl ActiveGame {
    fn toggle_pause(&mut self, now: Instant) -> bool {
        match self {
            Self::WordRain(active) => active.game.toggle_pause(now),
            Self::BossBattle(active) => active.game.toggle_pause(now),
        }
    }

    fn is_paused(&self) -> bool {
        match self {
            Self::WordRain(active) => active.game.is_paused(),
            Self::BossBattle(active) => active.game.is_paused(),
        }
    }

    fn leave_confirmation(&self) -> bool {
        match self {
            Self::WordRain(active) => active.leave_confirmation,
            Self::BossBattle(active) => active.leave_confirmation,
        }
    }

    fn set_leave_confirmation(&mut self, confirmed: bool) {
        match self {
            Self::WordRain(active) => active.leave_confirmation = confirmed,
            Self::BossBattle(active) => active.leave_confirmation = confirmed,
        }
    }

    fn submit_input(&mut self) {
        match self {
            Self::WordRain(active) => active.game.submit_input(),
            Self::BossBattle(active) => active.game.submit_input(),
        }
    }

    fn backspace(&mut self) {
        match self {
            Self::WordRain(active) => {
                active.game.backspace();
            }
            Self::BossBattle(active) => {
                active.game.backspace();
            }
        }
    }

    fn input_char(&mut self, character: char) {
        match self {
            Self::WordRain(active) => active.game.input_char(character),
            Self::BossBattle(active) => active.game.input_char(character),
        }
    }

    fn tick(&mut self, now: Instant) {
        match self {
            Self::WordRain(active) => active.game.tick(now),
            Self::BossBattle(active) => active.game.tick(now),
        }
    }

    fn outcome(&self) -> Option<FinishedGame> {
        match self {
            Self::WordRain(active) => active.game.outcome().cloned().map(FinishedGame::WordRain),
            Self::BossBattle(active) => {
                active.game.outcome().cloned().map(FinishedGame::BossBattle)
            }
        }
    }

    fn set_viewport_supported(&mut self, supported: bool, now: Instant) {
        match self {
            Self::WordRain(active) => active.game.set_viewport_supported(supported, now),
            Self::BossBattle(active) => active.game.set_viewport_supported(supported, now),
        }
    }
}

impl App {
    pub(super) fn start_word_rain_with_seed(&mut self, seed: u64, now: Instant) -> Result<()> {
        let words = self.game_words();
        if words.is_empty() {
            self.game_options.error =
                Some(text(self.settings.ui_language, TextKey::NoPlayableWords).to_owned());
            return Ok(());
        }

        let game = WordRain::new(
            self.game_options.language,
            self.game_options.difficulty,
            words,
            seed,
            now,
        )?;
        self.enter_game(ActiveGame::WordRain(ActiveWordRain {
            game,
            leave_confirmation: false,
        }));
        Ok(())
    }

    pub(super) fn start_boss_battle_with_seed(&mut self, seed: u64, now: Instant) -> Result<()> {
        let boss = self.game_options.boss;
        if !self.settings.boss_is_unlocked(boss) {
            self.game_options.error =
                Some(text(self.settings.ui_language, TextKey::BossLocked).to_owned());
            return Ok(());
        }
        if !self
            .settings
            .boss_difficulty_is_unlocked(boss, self.game_options.difficulty)
        {
            self.game_options.error =
                Some(text(self.settings.ui_language, TextKey::DifficultyLocked).to_owned());
            return Ok(());
        }

        let words = self.game_words();
        if words.is_empty() {
            self.game_options.error =
                Some(text(self.settings.ui_language, TextKey::NoPlayableWords).to_owned());
            return Ok(());
        }

        let game = BossBattle::new(
            boss,
            self.game_options.language,
            self.game_options.difficulty,
            words,
            seed,
            now,
        )?;
        self.enter_game(ActiveGame::BossBattle(ActiveBossBattle {
            game,
            leave_confirmation: false,
        }));
        Ok(())
    }

    fn game_words(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        self.content
            .select(
                self.game_options.language,
                ContentKind::Word,
                self.game_options.difficulty.content_difficulty(),
            )
            .into_iter()
            .filter_map(|item| playable_word(&item.text))
            .filter(|word| seen.insert(word.clone()))
            .collect()
    }

    fn enter_game(&mut self, active: ActiveGame) {
        self.remember_focus();
        self.screen = Screen::Game;
        self.parent = Screen::Games;
        self.parent_before_help = None;
        self.focus = 0;
        self.game_options.error = None;
        self.active_game = Some(active);
        self.game_result = None;
    }

    pub(super) fn handle_game_key(&mut self, key: KeyInput, now: Instant) -> Result<()> {
        if key.kind == KeyKind::Press && key.key == Key::Esc {
            if let Some(active) = self.active_game.as_mut()
                && active.toggle_pause(now)
            {
                active.set_leave_confirmation(false);
            }
            return Ok(());
        }

        if self.active_game.as_ref().is_some_and(ActiveGame::is_paused) {
            if key.kind == KeyKind::Press && key.is_plain_q_command() {
                let confirmed = self
                    .active_game
                    .as_ref()
                    .is_some_and(ActiveGame::leave_confirmation);
                if confirmed {
                    match self.game_options.kind {
                        GameKind::WordRain => self.return_to_games(),
                        GameKind::BossBattle => self.return_to_boss_options(),
                    }
                } else if let Some(active) = self.active_game.as_mut() {
                    active.set_leave_confirmation(true);
                }
            }
            return Ok(());
        }

        match key.key {
            Key::Enter if key.kind == KeyKind::Press && key.modifiers == KeyModifiers::NONE => {
                if let Some(active) = self.active_game.as_mut() {
                    active.submit_input();
                }
            }
            Key::Backspace if matches!(key.kind, KeyKind::Press | KeyKind::Repeat) => {
                if let Some(active) = self.active_game.as_mut() {
                    active.backspace();
                }
            }
            Key::Char(character)
                if matches!(key.kind, KeyKind::Press | KeyKind::Repeat)
                    && matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) =>
            {
                if let Some(active) = self.active_game.as_mut() {
                    active.input_char(character);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn tick_game(&mut self, now: Instant) {
        if self.screen != Screen::Game {
            return;
        }
        if let Some(active) = self.active_game.as_mut() {
            active.tick(now);
        }
        let outcome = self.active_game.as_ref().and_then(ActiveGame::outcome);
        let Some(outcome) = outcome else {
            return;
        };

        self.active_game = None;
        self.game_result = Some(match outcome {
            FinishedGame::WordRain(outcome) => {
                let language = self.game_options.language;
                let difficulty = self.game_options.difficulty;
                let previous_best = self.settings.word_rain_high_score(language, difficulty);
                if outcome.score > previous_best {
                    let score = outcome.score;
                    let _ = self.change_settings(|settings| {
                        settings.set_word_rain_high_score(language, difficulty, score);
                    });
                }
                StoredGameResult::WordRain(WordRainResult {
                    outcome,
                    previous_best,
                })
            }
            FinishedGame::BossBattle(outcome) => {
                let previous_best = self.settings.boss_high_score(
                    outcome.boss,
                    outcome.language,
                    outcome.difficulty,
                );
                let previous_rank = self.settings.boss_clear_rank(outcome.boss);
                if outcome.victory {
                    let boss = outcome.boss;
                    let language = outcome.language;
                    let difficulty = outcome.difficulty;
                    let score = outcome.score;
                    let _ = self.change_settings(|settings| {
                        settings.record_boss_clear(boss, language, difficulty, score);
                    });
                }
                StoredGameResult::BossBattle(BossBattleResult {
                    new_rank: self.settings.boss_clear_rank(outcome.boss),
                    outcome,
                    previous_best,
                    previous_rank,
                })
            }
        });
        self.screen = Screen::GameResult;
        self.parent = Screen::Games;
        self.parent_before_help = None;
        self.focus = 0;
    }

    pub(crate) fn set_game_viewport_supported(&mut self, supported: bool, now: Instant) {
        if let Some(active) = self.active_game.as_mut() {
            active.set_viewport_supported(supported, now);
        }
    }

    pub(super) fn return_to_games(&mut self) {
        self.remember_focus();
        self.screen = Screen::Games;
        self.parent = Screen::Home;
        self.parent_before_help = None;
        self.focus = 0;
        self.active_game = None;
        self.game_result = None;
    }

    pub(super) fn return_to_boss_options(&mut self) {
        self.remember_focus();
        self.screen = Screen::GameOptions;
        self.parent = Screen::Games;
        self.parent_before_help = None;
        self.focus = self.game_options.boss.index();
        self.game_options.error = None;
        self.active_game = None;
        self.game_result = None;
    }
}

fn playable_word(text: &str) -> Option<String> {
    let word = normalize_nfc(text);
    (1..=24)
        .contains(&UnicodeWidthStr::width(word.as_str()))
        .then_some(word)
}

#[cfg(test)]
mod tests {
    use super::super::{
        App, GameOptions, InputEvent, Key, KeyInput, KeyKind, KeyModifiers, Screen,
    };
    use crate::{
        config::Settings,
        content::ContentCatalog,
        game::{GameDifficulty, GameKind, boss_battle::BossKind},
        model::Language,
        storage::AppPaths,
        theme::ThemeCatalog,
    };
    use std::time::{Duration, Instant};

    fn fixture(content: ContentCatalog) -> App {
        fixture_with_settings(Settings::default(), content)
    }

    fn fixture_with_settings(settings: Settings, content: ContentCatalog) -> App {
        App::new(
            settings,
            AppPaths::from_override(std::env::temp_dir().join(format!(
                "typerlude-word-rain-{}-{}",
                std::process::id(),
                fastrand::u64(..)
            ))),
            content,
            ThemeCatalog::load_builtins().unwrap(),
            Vec::new(),
            Vec::new(),
        )
    }

    fn key(key: Key) -> InputEvent {
        InputEvent::Key(KeyInput {
            key,
            modifiers: KeyModifiers::NONE,
            kind: KeyKind::Press,
        })
    }

    fn key_with(key: Key, modifiers: KeyModifiers, kind: KeyKind) -> InputEvent {
        InputEvent::Key(KeyInput {
            key,
            modifiers,
            kind,
        })
    }

    fn start(app: &mut App, language: Language, difficulty: GameDifficulty, now: Instant) {
        app.game_options = GameOptions::new(GameKind::WordRain, language);
        app.game_options.difficulty = difficulty;
        app.start_word_rain_with_seed(7, now).unwrap();
    }

    fn complete_first_word(app: &mut App, now: Instant) -> u64 {
        let word = app
            .active_word_rain()
            .unwrap()
            .game
            .active_words()
            .next()
            .unwrap()
            .text()
            .to_owned();
        for character in word.chars() {
            app.handle_event(key(Key::Char(character)), now).unwrap();
        }
        app.active_word_rain().unwrap().game.score()
    }

    fn finish_game(app: &mut App, now: Instant) {
        for step in 1..=100 {
            app.tick(now + Duration::from_millis(step * 250)).unwrap();
        }
        assert_eq!(app.screen, Screen::GameResult);
    }

    fn force_boss_victory(app: &mut App, now: &mut Instant) {
        for _ in 0..10_000 {
            if app.screen == Screen::GameResult {
                return;
            }
            let prompt = app
                .active_boss_battle()
                .and_then(|active| active.game.prompts().next())
                .map(|prompt| prompt.text().to_owned());
            if let Some(prompt) = prompt {
                for character in prompt.chars() {
                    app.handle_event(key(Key::Char(character)), *now).unwrap();
                }
            }
            *now += Duration::from_millis(250);
            app.tick(*now).unwrap();
        }
        panic!("boss battle did not finish");
    }

    #[test]
    fn playable_word_normalizes_nfc_and_enforces_display_width() {
        assert_eq!(super::playable_word("안녕"), Some("안녕".into()));
        assert_eq!(super::playable_word(""), None);
        assert_eq!(super::playable_word("\u{301}"), None);
        assert_eq!(super::playable_word(&"a".repeat(25)), None);
        assert_eq!(
            super::playable_word(&"界".repeat(12)),
            Some("界".repeat(12))
        );
        assert_eq!(super::playable_word(&"界".repeat(13)), None);
    }

    #[test]
    fn starting_uses_exact_language_and_difficulty_content_with_an_immediate_word() {
        let now = Instant::now();
        for (difficulty, expected_content_difficulty) in [
            (GameDifficulty::Easy, 1),
            (GameDifficulty::Medium, 2),
            (GameDifficulty::Hard, 3),
            (GameDifficulty::Hell, 3),
        ] {
            let mut app = fixture(ContentCatalog::load_builtins().unwrap());
            start(&mut app, Language::En, difficulty, now);

            let active = app.active_word_rain().unwrap();
            assert_eq!(app.screen, Screen::Game);
            assert_eq!(active.game.difficulty(), difficulty);
            assert_eq!(active.game.active_words().count(), 1);
            let active_word = active.game.active_words().next().unwrap().text();
            assert!(app.content.items().any(|item| {
                item.language == Language::En
                    && item.difficulty == Some(expected_content_difficulty)
                    && item.text == active_word
            }));
        }
    }

    #[test]
    fn no_playable_words_stays_on_options_with_no_partial_game() {
        let now = Instant::now();
        let mut app = fixture(ContentCatalog::default());
        app.screen = Screen::GameOptions;
        app.game_options = GameOptions::new(GameKind::WordRain, Language::En);

        app.start_word_rain_with_seed(7, now).unwrap();

        assert_eq!(app.screen, Screen::GameOptions);
        assert!(app.active_game.is_none());
        assert!(app.game_result.is_none());
        assert_eq!(app.game_options.error.as_deref(), Some("No playable words"));
    }

    #[test]
    fn active_text_and_backspace_are_game_input_but_paste_is_ignored() {
        let now = Instant::now();
        let mut app = fixture(ContentCatalog::load_builtins().unwrap());
        start(&mut app, Language::En, GameDifficulty::Easy, now);

        app.handle_event(InputEvent::Paste, now).unwrap();
        app.handle_event(key(Key::Char('q')), now).unwrap();
        assert_eq!(app.active_word_rain().unwrap().game.input(), "q");
        assert_eq!(app.screen, Screen::Game);

        app.handle_event(key(Key::Backspace), now).unwrap();
        assert_eq!(app.active_word_rain().unwrap().game.input(), "");
    }

    #[test]
    fn enter_submits_and_clears_invalid_game_input() {
        let now = Instant::now();
        let mut app = fixture(ContentCatalog::load_builtins().unwrap());
        start(&mut app, Language::En, GameDifficulty::Easy, now);
        let first = app
            .active_word_rain()
            .unwrap()
            .game
            .active_words()
            .next()
            .unwrap()
            .text()
            .chars()
            .next()
            .unwrap();

        app.handle_event(key(Key::Char(first)), now).unwrap();
        app.handle_event(key(Key::Char('~')), now).unwrap();
        let active = app.active_word_rain().unwrap();
        assert!(!active.game.input_is_valid());
        assert!(active.game.target_id().is_some());

        app.handle_event(key(Key::Enter), now).unwrap();
        let active = app.active_word_rain().unwrap();
        assert_eq!(active.game.input(), "");
        assert_eq!(active.game.target_id(), None);
    }

    #[test]
    fn pause_requires_two_plain_q_commands_to_leave_and_escape_resumes() {
        let now = Instant::now();
        let mut app = fixture(ContentCatalog::load_builtins().unwrap());
        start(&mut app, Language::En, GameDifficulty::Easy, now);

        app.handle_event(key(Key::Esc), now).unwrap();
        assert!(app.active_word_rain().unwrap().game.is_paused());

        app.handle_event(key(Key::Char('q')), now).unwrap();
        assert!(app.active_word_rain().unwrap().leave_confirmation);
        assert_eq!(app.screen, Screen::Game);

        app.handle_event(key(Key::Esc), now).unwrap();
        assert!(!app.active_word_rain().unwrap().game.is_paused());
        assert!(!app.active_word_rain().unwrap().leave_confirmation);

        app.handle_event(key(Key::Esc), now).unwrap();
        app.handle_event(key(Key::Char('ㅂ')), now).unwrap();
        app.handle_event(key(Key::Char('ㅂ')), now).unwrap();
        assert_eq!(app.screen, Screen::Games);
        assert!(app.active_game.is_none());
    }

    #[test]
    fn word_rain_result_enter_returns_to_games() {
        let now = Instant::now();
        let mut app = fixture(ContentCatalog::load_builtins().unwrap());
        start(&mut app, Language::Ko, GameDifficulty::Hard, now);

        for step in 1..=40 {
            app.tick(now + Duration::from_millis(step * 250)).unwrap();
        }
        assert_eq!(app.screen, Screen::GameResult);
        assert!(app.active_game.is_none());
        assert!(app.game_result.is_some());

        app.handle_event(key(Key::Enter), now + Duration::from_secs(11))
            .unwrap();

        assert_eq!(app.screen, Screen::Games);
        assert!(app.active_game.is_none());
        assert!(app.game_result.is_none());
    }

    #[test]
    fn word_rain_result_r_or_korean_giyeok_retries_the_same_options() {
        for (character, modifiers) in [
            ('r', KeyModifiers::NONE),
            ('R', KeyModifiers::SHIFT),
            ('ㄱ', KeyModifiers::NONE),
        ] {
            let now = Instant::now();
            let mut app = fixture(ContentCatalog::load_builtins().unwrap());
            start(&mut app, Language::Ko, GameDifficulty::Hell, now);
            finish_game(&mut app, now);

            app.handle_event(
                key_with(Key::Char(character), modifiers, KeyKind::Press),
                now + Duration::from_secs(26),
            )
            .unwrap();

            let active = app.active_word_rain().unwrap();
            assert_eq!(app.screen, Screen::Game);
            assert_eq!(app.game_options.language, Language::Ko);
            assert_eq!(active.game.difficulty(), GameDifficulty::Hell);
            assert!(app.game_result.is_none());
        }
    }

    #[test]
    fn word_rain_result_ignores_repeated_or_modified_r() {
        let now = Instant::now();
        let mut app = fixture(ContentCatalog::load_builtins().unwrap());
        start(&mut app, Language::En, GameDifficulty::Easy, now);
        finish_game(&mut app, now);

        for event in [
            key_with(Key::Char('r'), KeyModifiers::NONE, KeyKind::Repeat),
            key_with(Key::Char('r'), KeyModifiers::OTHER, KeyKind::Press),
        ] {
            app.handle_event(event, now + Duration::from_secs(26))
                .unwrap();
            assert_eq!(app.screen, Screen::GameResult);
            assert!(app.active_game.is_none());
            assert!(app.game_result.is_some());
        }
    }

    #[test]
    fn a_higher_score_updates_only_its_language_and_difficulty_and_survives_reload() {
        let now = Instant::now();
        let settings = Settings {
            word_rain_high_scores: [[11, 12, 13], [0, 22, 23]],
            ..Settings::default()
        };
        let mut app = fixture_with_settings(settings, ContentCatalog::load_builtins().unwrap());
        start(&mut app, Language::En, GameDifficulty::Easy, now);
        let score = complete_first_word(&mut app, now);

        finish_game(&mut app, now);

        let (result, previous_best) = app.word_rain_result().unwrap();
        assert_eq!(result.score, score);
        assert_eq!(previous_best, 0);
        assert_eq!(
            app.settings.word_rain_high_scores,
            [[11, 12, 13], [score, 22, 23]]
        );
        assert_eq!(
            Settings::load(&app.paths)
                .unwrap()
                .value
                .word_rain_high_scores,
            [[11, 12, 13], [score, 22, 23]]
        );
    }

    #[test]
    fn an_equal_score_does_not_update_the_personal_best() {
        let now = Instant::now();
        let mut app = fixture(ContentCatalog::load_builtins().unwrap());
        start(&mut app, Language::En, GameDifficulty::Easy, now);
        let score = complete_first_word(&mut app, now);
        finish_game(&mut app, now);

        start(&mut app, Language::En, GameDifficulty::Easy, now);
        assert_eq!(complete_first_word(&mut app, now), score);
        finish_game(&mut app, now);

        let (result, previous_best) = app.word_rain_result().unwrap();
        assert_eq!(result.score, score);
        assert_eq!(previous_best, score);
        assert_eq!(
            app.settings
                .word_rain_high_score(Language::En, GameDifficulty::Easy),
            score
        );
    }

    #[test]
    fn a_boss_victory_updates_shared_rank_and_language_scoped_score() {
        let mut now = Instant::now();
        let mut app = fixture(ContentCatalog::load_builtins().unwrap());
        app.game_options = GameOptions::new(GameKind::BossBattle, Language::En);
        app.game_options.boss = BossKind::IronWarden;
        app.game_options.difficulty = GameDifficulty::Easy;
        app.start_boss_battle_with_seed(7, now).unwrap();

        force_boss_victory(&mut app, &mut now);

        assert_eq!(app.screen, Screen::GameResult);
        assert_eq!(app.settings.boss_clear_rank(BossKind::IronWarden), 1);
        assert!(app.settings.boss_is_unlocked(BossKind::ThornQueen));
        assert_eq!(
            app.settings
                .boss_high_score(BossKind::IronWarden, Language::Ko, GameDifficulty::Easy,),
            0,
        );
    }

    #[test]
    fn boss_result_enter_and_escape_preserve_the_exact_boss_options() {
        for exit in [Key::Enter, Key::Esc] {
            let mut now = Instant::now();
            let mut app = fixture(ContentCatalog::load_builtins().unwrap());
            for (boss, difficulty) in [
                (BossKind::IronWarden, GameDifficulty::Easy),
                (BossKind::ThornQueen, GameDifficulty::Easy),
                (BossKind::NullArchon, GameDifficulty::Easy),
                (BossKind::NullArchon, GameDifficulty::Medium),
            ] {
                app.settings
                    .record_boss_clear(boss, Language::En, difficulty, 1);
            }
            app.game_options = GameOptions::new(GameKind::BossBattle, Language::Ko);
            app.game_options.boss = BossKind::NullArchon;
            app.game_options.difficulty = GameDifficulty::Hard;
            app.start_boss_battle_with_seed(7, now).unwrap();
            force_boss_victory(&mut app, &mut now);

            app.handle_event(key(exit), now).unwrap();

            assert_eq!(app.screen, Screen::GameOptions, "{exit:?}");
            assert_eq!(app.parent, Screen::Games, "{exit:?}");
            assert_eq!(app.focus, BossKind::NullArchon.index(), "{exit:?}");
            assert_eq!(app.game_options.kind, GameKind::BossBattle, "{exit:?}");
            assert_eq!(app.game_options.boss, BossKind::NullArchon, "{exit:?}");
            assert_eq!(app.game_options.language, Language::Ko, "{exit:?}");
            assert_eq!(
                app.game_options.difficulty,
                GameDifficulty::Hard,
                "{exit:?}"
            );
            assert!(app.game_options.error.is_none(), "{exit:?}");
            assert!(app.active_game.is_none(), "{exit:?}");
            assert!(app.game_result.is_none(), "{exit:?}");
        }
    }

    #[test]
    fn boss_result_korean_giyeok_retries_the_same_boss_options() {
        let mut now = Instant::now();
        let mut app = fixture(ContentCatalog::load_builtins().unwrap());
        app.game_options = GameOptions::new(GameKind::BossBattle, Language::Ko);
        app.game_options.boss = BossKind::IronWarden;
        app.game_options.difficulty = GameDifficulty::Easy;
        app.start_boss_battle_with_seed(7, now).unwrap();
        force_boss_victory(&mut app, &mut now);

        app.handle_event(key(Key::Char('ㄱ')), now).unwrap();

        let active = app.active_boss_battle().unwrap();
        assert_eq!(app.screen, Screen::Game);
        assert_eq!(app.game_options.boss, BossKind::IronWarden);
        assert_eq!(app.game_options.language, Language::Ko);
        assert_eq!(active.game.difficulty(), GameDifficulty::Easy);
        assert!(app.game_result.is_none());
    }
}

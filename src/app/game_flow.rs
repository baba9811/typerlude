use super::{ActiveWordRain, App, Key, KeyInput, KeyKind, KeyModifiers, Screen, WordRainResult};
use crate::{
    content::ContentKind,
    game::word_rain::WordRain,
    i18n::{TextKey, text},
    typing::normalize_nfc,
};
use anyhow::Result;
use std::{collections::HashSet, time::Instant};
use unicode_width::UnicodeWidthStr;

impl App {
    pub(super) fn start_word_rain_with_seed(&mut self, seed: u64, now: Instant) -> Result<()> {
        let mut seen = HashSet::new();
        let words = self
            .content
            .select(
                self.game_options.language,
                ContentKind::Word,
                self.game_options.difficulty,
            )
            .into_iter()
            .filter_map(|item| playable_word(&item.text))
            .filter(|word| seen.insert(word.clone()))
            .collect::<Vec<_>>();
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
        self.remember_focus();
        self.screen = Screen::Game;
        self.parent = Screen::Games;
        self.parent_before_help = None;
        self.focus = 0;
        self.game_options.error = None;
        self.active_game = Some(ActiveWordRain {
            game,
            leave_confirmation: false,
        });
        self.game_result = None;
        Ok(())
    }

    pub(super) fn handle_game_key(&mut self, key: KeyInput, now: Instant) -> Result<()> {
        if key.kind == KeyKind::Press && key.key == Key::Esc {
            if let Some(active) = self.active_game.as_mut()
                && active.game.toggle_pause(now)
            {
                active.leave_confirmation = false;
            }
            return Ok(());
        }

        if self
            .active_game
            .as_ref()
            .is_some_and(|active| active.game.is_paused())
        {
            if key.kind == KeyKind::Press && key.is_plain_q_command() {
                let confirmed = self
                    .active_game
                    .as_ref()
                    .is_some_and(|active| active.leave_confirmation);
                if confirmed {
                    self.return_to_games();
                } else if let Some(active) = self.active_game.as_mut() {
                    active.leave_confirmation = true;
                }
            }
            return Ok(());
        }

        match key.key {
            Key::Enter if key.kind == KeyKind::Press && key.modifiers == KeyModifiers::NONE => {
                if let Some(active) = self.active_game.as_mut() {
                    active.game.submit_input();
                }
            }
            Key::Backspace if matches!(key.kind, KeyKind::Press | KeyKind::Repeat) => {
                if let Some(active) = self.active_game.as_mut() {
                    active.game.backspace();
                }
            }
            Key::Char(character)
                if matches!(key.kind, KeyKind::Press | KeyKind::Repeat)
                    && matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) =>
            {
                if let Some(active) = self.active_game.as_mut() {
                    active.game.input_char(character);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn tick_word_rain(&mut self, now: Instant) {
        if self.screen != Screen::Game {
            return;
        }
        if let Some(active) = self.active_game.as_mut() {
            active.game.tick(now);
        }
        let outcome = self
            .active_game
            .as_ref()
            .and_then(|active| active.game.outcome().cloned());
        if let Some(outcome) = outcome {
            let language = self.game_options.language;
            let difficulty = self.game_options.difficulty;
            let previous_best = self.settings.word_rain_high_score(language, difficulty);
            if outcome.score > previous_best {
                let score = outcome.score;
                let _ = self.change_settings(|settings| {
                    settings.set_word_rain_high_score(language, difficulty, score);
                });
            }
            self.active_game = None;
            self.game_result = Some(WordRainResult {
                outcome,
                previous_best,
            });
            self.screen = Screen::GameResult;
            self.parent = Screen::Games;
            self.parent_before_help = None;
            self.focus = 0;
        }
    }

    pub(crate) fn set_game_viewport_supported(&mut self, supported: bool, now: Instant) {
        if let Some(active) = self.active_game.as_mut() {
            active.game.set_viewport_supported(supported, now);
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
        game::GameKind,
        model::{Difficulty, Language},
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

    fn start(app: &mut App, language: Language, difficulty: Difficulty, now: Instant) {
        app.game_options = GameOptions::new(GameKind::WordRain, language);
        app.game_options.difficulty = difficulty;
        app.start_word_rain_with_seed(7, now).unwrap();
    }

    fn complete_first_word(app: &mut App, now: Instant) -> u64 {
        let word = app
            .active_game
            .as_ref()
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
        app.active_game.as_ref().unwrap().game.score()
    }

    fn finish_game(app: &mut App, now: Instant) {
        for step in 1..=100 {
            app.tick(now + Duration::from_millis(step * 250)).unwrap();
        }
        assert_eq!(app.screen, Screen::GameResult);
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
            (Difficulty::Easy, 1),
            (Difficulty::Medium, 2),
            (Difficulty::Hard, 3),
        ] {
            let mut app = fixture(ContentCatalog::load_builtins().unwrap());
            start(&mut app, Language::En, difficulty, now);

            let active = app.active_game.as_ref().unwrap();
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
        start(&mut app, Language::En, Difficulty::Easy, now);

        app.handle_event(InputEvent::Paste, now).unwrap();
        app.handle_event(key(Key::Char('q')), now).unwrap();
        assert_eq!(app.active_game.as_ref().unwrap().game.input(), "q");
        assert_eq!(app.screen, Screen::Game);

        app.handle_event(key(Key::Backspace), now).unwrap();
        assert_eq!(app.active_game.as_ref().unwrap().game.input(), "");
    }

    #[test]
    fn enter_submits_and_clears_invalid_game_input() {
        let now = Instant::now();
        let mut app = fixture(ContentCatalog::load_builtins().unwrap());
        start(&mut app, Language::En, Difficulty::Easy, now);
        let first = app
            .active_game
            .as_ref()
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
        let active = app.active_game.as_ref().unwrap();
        assert!(!active.game.input_is_valid());
        assert!(active.game.target_id().is_some());

        app.handle_event(key(Key::Enter), now).unwrap();
        let active = app.active_game.as_ref().unwrap();
        assert_eq!(active.game.input(), "");
        assert_eq!(active.game.target_id(), None);
    }

    #[test]
    fn pause_requires_two_plain_q_commands_to_leave_and_escape_resumes() {
        let now = Instant::now();
        let mut app = fixture(ContentCatalog::load_builtins().unwrap());
        start(&mut app, Language::En, Difficulty::Easy, now);

        app.handle_event(key(Key::Esc), now).unwrap();
        assert!(app.active_game.as_ref().unwrap().game.is_paused());

        app.handle_event(key(Key::Char('q')), now).unwrap();
        assert!(app.active_game.as_ref().unwrap().leave_confirmation);
        assert_eq!(app.screen, Screen::Game);

        app.handle_event(key(Key::Esc), now).unwrap();
        assert!(!app.active_game.as_ref().unwrap().game.is_paused());
        assert!(!app.active_game.as_ref().unwrap().leave_confirmation);

        app.handle_event(key(Key::Esc), now).unwrap();
        app.handle_event(key(Key::Char('ㅂ')), now).unwrap();
        app.handle_event(key(Key::Char('ㅂ')), now).unwrap();
        assert_eq!(app.screen, Screen::Games);
        assert!(app.active_game.is_none());
    }

    #[test]
    fn collision_opens_result_and_enter_retries_the_same_options() {
        let now = Instant::now();
        let mut app = fixture(ContentCatalog::load_builtins().unwrap());
        start(&mut app, Language::Ko, Difficulty::Hard, now);

        for step in 1..=40 {
            app.tick(now + Duration::from_millis(step * 250)).unwrap();
        }
        assert_eq!(app.screen, Screen::GameResult);
        assert!(app.active_game.is_none());
        assert!(app.game_result.is_some());

        app.handle_event(key(Key::Enter), now + Duration::from_secs(11))
            .unwrap();
        let active = app.active_game.as_ref().unwrap();
        assert_eq!(app.screen, Screen::Game);
        assert_eq!(app.game_options.language, Language::Ko);
        assert_eq!(active.game.difficulty(), Difficulty::Hard);
        assert!(app.game_result.is_none());
    }

    #[test]
    fn a_higher_score_updates_only_its_language_and_difficulty_and_survives_reload() {
        let now = Instant::now();
        let settings = Settings {
            word_rain_high_scores: [[11, 12, 13], [0, 22, 23]],
            ..Settings::default()
        };
        let mut app = fixture_with_settings(settings, ContentCatalog::load_builtins().unwrap());
        start(&mut app, Language::En, Difficulty::Easy, now);
        let score = complete_first_word(&mut app, now);

        finish_game(&mut app, now);

        let result = app.game_result.as_ref().unwrap();
        assert_eq!(result.outcome.score, score);
        assert_eq!(result.previous_best, 0);
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
        start(&mut app, Language::En, Difficulty::Easy, now);
        let score = complete_first_word(&mut app, now);
        finish_game(&mut app, now);

        start(&mut app, Language::En, Difficulty::Easy, now);
        assert_eq!(complete_first_word(&mut app, now), score);
        finish_game(&mut app, now);

        let result = app.game_result.as_ref().unwrap();
        assert_eq!(result.outcome.score, score);
        assert_eq!(result.previous_best, score);
        assert_eq!(
            app.settings
                .word_rain_high_score(Language::En, Difficulty::Easy),
            score
        );
    }
}

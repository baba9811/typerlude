use crate::{
    game::{GameDifficulty, boss_battle::BossKind},
    i18n::initial_ui_language_os,
    model::Language,
    storage::{AppPaths, LoadWarning, atomic_write},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, fs, io::ErrorKind};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct BossProgress {
    pub clear_rank: u8,
    pub high_scores: [[u64; 3]; 2],
    #[serde(default)]
    pub hell_high_scores: [u64; 2],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u16,
    pub language: Language,
    pub ui_language: Language,
    pub theme: String,
    pub show_keyboard: bool,
    pub show_finger_guide: bool,
    pub show_live_speed: bool,
    pub show_accuracy: bool,
    pub target_kpm: u32,
    pub target_wpm: u32,
    pub target_accuracy: f64,
    pub daily_minutes: u32,
    pub adaptive: bool,
    pub check_updates: bool,
    pub skipped_update_version: String,
    pub word_rain_high_scores: [[u64; 3]; 2],
    #[serde(default)]
    pub word_rain_hell_high_scores: [u64; 2],
    pub boss_battle_progress: Vec<BossProgress>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            language: Language::En,
            ui_language: Language::En,
            theme: "default".into(),
            show_keyboard: true,
            show_finger_guide: true,
            show_live_speed: true,
            show_accuracy: true,
            target_kpm: 450,
            target_wpm: 80,
            target_accuracy: 98.0,
            daily_minutes: 15,
            adaptive: true,
            check_updates: true,
            skipped_update_version: String::new(),
            word_rain_high_scores: [[0; 3]; 2],
            word_rain_hell_high_scores: [0; 2],
            boss_battle_progress: vec![BossProgress::default(); BossKind::ALL.len()],
        }
    }
}

#[derive(Debug)]
pub struct ConfigLoad {
    pub value: Settings,
    pub warnings: Vec<LoadWarning>,
}

impl Settings {
    pub(crate) fn boss_clear_rank(&self, boss: BossKind) -> u8 {
        self.boss_battle_progress
            .get(boss.index())
            .map_or(0, |progress| progress.clear_rank)
    }

    pub(crate) fn boss_is_unlocked(&self, boss: BossKind) -> bool {
        boss.index() == 0 || self.boss_clear_rank(BossKind::ALL[boss.index() - 1]) >= 1
    }

    pub(crate) fn boss_difficulty_is_unlocked(
        &self,
        boss: BossKind,
        difficulty: GameDifficulty,
    ) -> bool {
        self.boss_is_unlocked(boss) && difficulty.index() <= usize::from(self.boss_clear_rank(boss))
    }

    pub(crate) fn boss_high_score(
        &self,
        boss: BossKind,
        language: Language,
        difficulty: GameDifficulty,
    ) -> u64 {
        self.boss_battle_progress
            .get(boss.index())
            .map_or(0, |progress| {
                if difficulty == GameDifficulty::Hell {
                    progress.hell_high_scores[language_slot(language)]
                } else {
                    progress.high_scores[language_slot(language)][difficulty.index()]
                }
            })
    }

    pub(crate) fn record_boss_clear(
        &mut self,
        boss: BossKind,
        language: Language,
        difficulty: GameDifficulty,
        score: u64,
    ) {
        let boss = boss.index();
        if self.boss_battle_progress.len() <= boss {
            self.boss_battle_progress
                .resize_with(boss + 1, BossProgress::default);
        }
        let progress = &mut self.boss_battle_progress[boss];
        progress.clear_rank = progress.clear_rank.max(difficulty.index() as u8 + 1);
        let best = if difficulty == GameDifficulty::Hell {
            &mut progress.hell_high_scores[language_slot(language)]
        } else {
            &mut progress.high_scores[language_slot(language)][difficulty.index()]
        };
        *best = (*best).max(score);
    }

    pub(crate) fn word_rain_high_score(
        &self,
        language: Language,
        difficulty: GameDifficulty,
    ) -> u64 {
        if difficulty == GameDifficulty::Hell {
            self.word_rain_hell_high_scores[language_slot(language)]
        } else {
            self.word_rain_high_scores[language_slot(language)][difficulty.index()]
        }
    }

    pub(crate) fn set_word_rain_high_score(
        &mut self,
        language: Language,
        difficulty: GameDifficulty,
        score: u64,
    ) {
        if difficulty == GameDifficulty::Hell {
            self.word_rain_hell_high_scores[language_slot(language)] = score;
        } else {
            self.word_rain_high_scores[language_slot(language)][difficulty.index()] = score;
        }
    }

    pub fn load(paths: &AppPaths) -> Result<ConfigLoad> {
        let lc_all = std::env::var_os("LC_ALL");
        let lang = std::env::var_os("LANG");
        Self::load_with_locale(paths, lc_all.as_deref(), lang.as_deref())
    }

    fn load_with_locale(
        paths: &AppPaths,
        lc_all: Option<&OsStr>,
        lang: Option<&OsStr>,
    ) -> Result<ConfigLoad> {
        let bytes = match fs::read(&paths.config) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                match fs::symlink_metadata(&paths.config) {
                    Err(metadata_error) if metadata_error.kind() == ErrorKind::NotFound => {
                        let language = initial_ui_language_os(lc_all, lang);
                        return Ok(ConfigLoad {
                            value: Self {
                                language,
                                ui_language: language,
                                ..Self::default()
                            },
                            warnings: Vec::new(),
                        });
                    }
                    Ok(_) => {}
                    Err(metadata_error) => {
                        return Err(metadata_error).with_context(|| {
                            format!("failed to inspect {}", paths.config.display())
                        });
                    }
                }
                return Err(error)
                    .with_context(|| format!("failed to read {}", paths.config.display()));
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {}", paths.config.display()));
            }
        };

        match std::str::from_utf8(&bytes)
            .context("config is not valid UTF-8")
            .and_then(|source| toml::from_str::<Self>(source).context("invalid config TOML"))
            .and_then(|settings| {
                settings.validate()?;
                Ok(settings)
            }) {
            Ok(value) => Ok(ConfigLoad {
                value,
                warnings: Vec::new(),
            }),
            Err(error) => Ok(ConfigLoad {
                value: Self::default(),
                warnings: vec![LoadWarning {
                    path: paths.config.clone(),
                    message: error.to_string(),
                }],
            }),
        }
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        self.validate()?;
        let source = toml::to_string_pretty(self).context("failed to serialize settings")?;
        atomic_write(&paths.config, source.as_bytes())
            .with_context(|| format!("failed to save {}", paths.config.display()))
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            bail!("unsupported config schema version {}", self.schema_version);
        }
        if self.theme.trim().is_empty() {
            bail!("theme must not be empty");
        }
        if !(1..=5_000).contains(&self.target_kpm) {
            bail!("target_kpm must be between 1 and 5000");
        }
        if !(1..=5_000).contains(&self.target_wpm) {
            bail!("target_wpm must be between 1 and 5000");
        }
        if !self.target_accuracy.is_finite() || !(1.0..=100.0).contains(&self.target_accuracy) {
            bail!("target_accuracy must be between 1 and 100");
        }
        if !(1..=1_440).contains(&self.daily_minutes) {
            bail!("daily_minutes must be between 1 and 1440");
        }
        if self
            .boss_battle_progress
            .iter()
            .any(|progress| progress.clear_rank > 4)
        {
            bail!("boss clear rank must be between 0 and 4");
        }
        Ok(())
    }
}

fn language_slot(language: Language) -> usize {
    match language {
        Language::Ko => 0,
        Language::En => 1,
    }
}

#[cfg(test)]
mod locale_tests {
    use super::Settings;
    use crate::{
        game::{GameDifficulty, boss_battle::BossKind},
        model::Language,
        storage::AppPaths,
    };
    use std::{ffi::OsStr, fs, path::PathBuf};

    #[test]
    fn locale_applies_only_to_a_genuinely_missing_config() {
        let root = std::env::temp_dir().join(format!(
            "typerlude-config-locale-{}-{}",
            std::process::id(),
            fastrand::u64(..)
        ));
        let paths = AppPaths::from_override(PathBuf::from(&root));

        let missing = Settings::load_with_locale(
            &paths,
            Some(OsStr::new("ko_KR.UTF-8")),
            Some(OsStr::new("en")),
        )
        .unwrap();
        assert_eq!(missing.value.language, Language::Ko);
        assert_eq!(missing.value.ui_language, Language::Ko);
        assert!(missing.warnings.is_empty());
        assert!(!paths.config.exists());

        let missing_en =
            Settings::load_with_locale(&paths, Some(OsStr::new("en_US.UTF-8")), None).unwrap();
        assert_eq!(missing_en.value.language, Language::En);
        assert_eq!(missing_en.value.ui_language, Language::En);

        fs::create_dir_all(&root).unwrap();
        fs::write(&paths.config, b"schema_version = 1\nui_language = \"en\"\n").unwrap();
        let saved = Settings::load_with_locale(&paths, Some(OsStr::new("ko")), None).unwrap();
        assert_eq!(saved.value.language, Language::En);
        assert_eq!(saved.value.ui_language, Language::En);
        assert!(saved.warnings.is_empty());

        let corrupt = b"schema_version = [";
        fs::write(&paths.config, corrupt).unwrap();
        let loaded = Settings::load_with_locale(&paths, Some(OsStr::new("ko")), None).unwrap();
        assert_eq!(loaded.value, Settings::default());
        assert_eq!(loaded.warnings.len(), 1);
        assert_eq!(loaded.warnings[0].path, paths.config);
        assert_eq!(fs::read(&paths.config).unwrap(), corrupt);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn word_rain_high_scores_are_scoped_by_language_and_difficulty() {
        let settings = Settings {
            word_rain_high_scores: [[11, 12, 13], [21, 22, 23]],
            ..Settings::default()
        };

        for (language, difficulty, expected) in [
            (Language::Ko, GameDifficulty::Easy, 11),
            (Language::Ko, GameDifficulty::Medium, 12),
            (Language::Ko, GameDifficulty::Hard, 13),
            (Language::En, GameDifficulty::Easy, 21),
            (Language::En, GameDifficulty::Medium, 22),
            (Language::En, GameDifficulty::Hard, 23),
        ] {
            assert_eq!(
                settings.word_rain_high_score(language, difficulty),
                expected
            );
        }
    }

    #[test]
    fn boss_progress_requires_prior_clears_and_easy_opens_the_next_boss() {
        let mut settings = Settings::default();
        assert!(settings.boss_is_unlocked(BossKind::IronWarden));
        assert!(!settings.boss_is_unlocked(BossKind::ThornQueen));
        assert!(settings.boss_difficulty_is_unlocked(BossKind::IronWarden, GameDifficulty::Easy,));
        assert!(
            !settings.boss_difficulty_is_unlocked(BossKind::IronWarden, GameDifficulty::Medium,)
        );

        settings.record_boss_clear(
            BossKind::IronWarden,
            Language::En,
            GameDifficulty::Easy,
            12_345,
        );

        assert_eq!(settings.boss_clear_rank(BossKind::IronWarden), 1);
        assert!(settings.boss_is_unlocked(BossKind::ThornQueen));
        assert!(
            settings.boss_difficulty_is_unlocked(BossKind::IronWarden, GameDifficulty::Medium,)
        );
        assert!(!settings.boss_difficulty_is_unlocked(BossKind::IronWarden, GameDifficulty::Hard,));
        assert_eq!(
            settings.boss_high_score(BossKind::IronWarden, Language::En, GameDifficulty::Easy,),
            12_345,
        );
        assert_eq!(
            settings.boss_high_score(BossKind::IronWarden, Language::Ko, GameDifficulty::Easy,),
            0,
        );
    }

    #[test]
    fn recording_a_lower_clear_never_regresses_rank_or_best_score() {
        let mut settings = Settings::default();
        settings.record_boss_clear(
            BossKind::IronWarden,
            Language::En,
            GameDifficulty::Hard,
            20_000,
        );
        settings.record_boss_clear(
            BossKind::IronWarden,
            Language::En,
            GameDifficulty::Easy,
            10_000,
        );
        settings.record_boss_clear(
            BossKind::IronWarden,
            Language::En,
            GameDifficulty::Hard,
            19_000,
        );

        assert_eq!(settings.boss_clear_rank(BossKind::IronWarden), 3);
        assert_eq!(
            settings.boss_high_score(BossKind::IronWarden, Language::En, GameDifficulty::Hard,),
            20_000,
        );
    }

    #[test]
    fn recording_an_appended_boss_extends_short_progress() {
        let mut settings = Settings {
            boss_battle_progress: Vec::new(),
            ..Settings::default()
        };

        settings.record_boss_clear(
            BossKind::NullArchon,
            Language::Ko,
            GameDifficulty::Easy,
            9_000,
        );

        assert_eq!(settings.boss_battle_progress.len(), 3);
        assert_eq!(settings.boss_clear_rank(BossKind::NullArchon), 1);
        assert_eq!(
            settings.boss_high_score(BossKind::NullArchon, Language::Ko, GameDifficulty::Easy,),
            9_000,
        );
    }

    #[test]
    fn hell_scores_are_separate_and_four_clears_never_regress() {
        let mut settings = Settings::default();

        settings.set_word_rain_high_score(Language::Ko, GameDifficulty::Hard, 300);
        settings.set_word_rain_high_score(Language::Ko, GameDifficulty::Hell, 400);
        assert_eq!(
            settings.word_rain_high_score(Language::Ko, GameDifficulty::Hard),
            300
        );
        assert_eq!(
            settings.word_rain_high_score(Language::Ko, GameDifficulty::Hell),
            400
        );

        for difficulty in GameDifficulty::ALL {
            assert_eq!(
                settings.boss_difficulty_is_unlocked(BossKind::IronWarden, difficulty),
                difficulty.index() <= usize::from(settings.boss_clear_rank(BossKind::IronWarden)),
            );
            if settings.boss_difficulty_is_unlocked(BossKind::IronWarden, difficulty) {
                settings.record_boss_clear(
                    BossKind::IronWarden,
                    Language::En,
                    difficulty,
                    10_000 + difficulty.index() as u64,
                );
            }
        }

        assert_eq!(settings.boss_clear_rank(BossKind::IronWarden), 4);
        assert_eq!(
            settings.boss_high_score(BossKind::IronWarden, Language::En, GameDifficulty::Hell,),
            10_003,
        );
        settings.record_boss_clear(BossKind::IronWarden, Language::En, GameDifficulty::Hard, 1);
        assert_eq!(settings.boss_clear_rank(BossKind::IronWarden), 4);
    }

    #[test]
    fn boss_clear_rank_above_four_is_rejected() {
        let mut settings = Settings::default();
        settings.boss_battle_progress[0].clear_rank = 5;

        let error = settings.validate().unwrap_err();

        assert!(
            error
                .to_string()
                .contains("boss clear rank must be between 0 and 4"),
            "{error:#}",
        );
    }
}

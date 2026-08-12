use crate::{
    i18n::initial_ui_language_os,
    model::Language,
    storage::{AppPaths, LoadWarning, atomic_write},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, fs, io::ErrorKind};

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
        }
    }
}

#[derive(Debug)]
pub struct ConfigLoad {
    pub value: Settings,
    pub warnings: Vec<LoadWarning>,
}

impl Settings {
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
                        return Ok(ConfigLoad {
                            value: Self {
                                ui_language: initial_ui_language_os(lc_all, lang),
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
        Ok(())
    }
}

#[cfg(test)]
mod locale_tests {
    use super::Settings;
    use crate::{model::Language, storage::AppPaths};
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
        assert_eq!(missing.value.ui_language, Language::Ko);
        assert!(missing.warnings.is_empty());
        assert!(!paths.config.exists());

        fs::create_dir_all(&root).unwrap();
        fs::write(&paths.config, b"schema_version = 1\nui_language = \"en\"\n").unwrap();
        let saved = Settings::load_with_locale(&paths, Some(OsStr::new("ko")), None).unwrap();
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
}

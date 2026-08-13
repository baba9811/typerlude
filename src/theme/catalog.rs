use super::{ThemeCatalog, ThemeLoad, ThemeSpec, parse_theme};
use crate::storage::LoadWarning;
use anyhow::{Context, Result, bail};
use include_dir::{Dir, include_dir};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Read,
    path::Path,
};

const MAX_THEME_BYTES: usize = 8 * 1024 * 1024;
static BUILTIN: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/themes");

impl ThemeCatalog {
    pub fn load_builtins() -> Result<Self> {
        let mut themes = Vec::new();
        collect_builtins(&BUILTIN, &mut themes)?;
        themes.sort_unstable_by(|left, right| left.0.cmp(&right.0));

        let mut catalog = Self {
            themes: Vec::new(),
            ids: HashSet::new(),
        };
        for (path, theme) in themes {
            catalog
                .insert(theme)
                .with_context(|| format!("invalid built-in theme {path}"))?;
        }
        Ok(catalog)
    }

    pub fn load(user_dir: &Path) -> Result<ThemeLoad> {
        let mut catalog = Self::load_builtins()?;
        let mut warnings = Vec::new();
        let mut entries = match fs::read_dir(user_dir) {
            Ok(entries) => entries.collect::<std::io::Result<Vec<_>>>()?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {}", user_dir.display()));
            }
        };
        entries.sort_unstable_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
                continue;
            }
            let result = entry
                .file_type()
                .with_context(|| format!("failed to inspect {}", path.display()))
                .and_then(|file_type| {
                    if !file_type.is_file() {
                        bail!("{} is not a regular file", path.display());
                    }
                    read_theme(&path)
                })
                .and_then(|source| parse_theme(&source))
                .and_then(|theme| catalog.insert(theme));
            if let Err(error) = result {
                warnings.push(LoadWarning {
                    path,
                    message: error.to_string(),
                });
            }
        }

        Ok(ThemeLoad { catalog, warnings })
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.themes.iter().map(|theme| theme.id.as_str())
    }

    pub fn get(&self, id: &str) -> Option<&ThemeSpec> {
        self.themes.iter().find(|theme| theme.id == id)
    }

    fn insert(&mut self, theme: ThemeSpec) -> Result<()> {
        if !self.ids.insert(theme.id.clone()) {
            bail!("duplicate theme ID {:?}", theme.id);
        }
        self.themes.push(theme);
        Ok(())
    }
}

fn collect_builtins(dir: &Dir<'_>, themes: &mut Vec<(String, ThemeSpec)>) -> Result<()> {
    for file in dir.files() {
        if file
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("toml")
        {
            continue;
        }
        let path = file.path().to_string_lossy().into_owned();
        let source = file
            .contents_utf8()
            .with_context(|| format!("built-in theme {path} is not valid UTF-8"))?;
        let theme =
            parse_theme(source).with_context(|| format!("invalid built-in theme {path}"))?;
        themes.push((path, theme));
    }
    for child in dir.dirs() {
        collect_builtins(child, themes)?;
    }
    Ok(())
}

fn read_theme(path: &Path) -> Result<String> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_THEME_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if bytes.len() > MAX_THEME_BYTES {
        bail!(
            "theme exceeds the {} MiB limit",
            MAX_THEME_BYTES / 1024 / 1024
        );
    }
    String::from_utf8(bytes).context("theme is not valid UTF-8")
}

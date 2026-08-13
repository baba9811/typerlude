use crate::storage::LoadWarning;
use anyhow::{Context, Result, bail};
use include_dir::{Dir, include_dir};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Read,
    path::Path,
};

const MAX_THEME_BYTES: usize = 8 * 1024 * 1024;
static BUILTIN: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/themes");

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeSpec {
    pub schema_version: u16,
    pub id: String,
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub correct: String,
    pub error: String,
    pub cursor: String,
    pub dim: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeColor {
    Reset,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThemePalette {
    pub background: ThemeColor,
    pub foreground: ThemeColor,
    pub accent: ThemeColor,
    pub correct: ThemeColor,
    pub error: ThemeColor,
    pub cursor: ThemeColor,
    pub dim: ThemeColor,
}

pub struct ThemeCatalog {
    themes: Vec<ThemeSpec>,
    ids: HashSet<String>,
}

pub struct ThemeLoad {
    pub catalog: ThemeCatalog,
    pub warnings: Vec<LoadWarning>,
}

pub fn parse_theme(source: &str) -> Result<ThemeSpec> {
    let theme = toml::from_str::<ThemeSpec>(source).context("invalid theme TOML")?;
    theme.validate()?;
    Ok(theme)
}

impl ThemeSpec {
    pub fn palette(&self) -> Result<ThemePalette> {
        self.validate()?;
        Ok(ThemePalette {
            background: parse_color(&self.background)?,
            foreground: parse_color(&self.foreground)?,
            accent: parse_color(&self.accent)?,
            correct: parse_color(&self.correct)?,
            error: parse_color(&self.error)?,
            cursor: parse_color(&self.cursor)?,
            dim: parse_color(&self.dim)?,
        })
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            bail!("schema_version must be 1");
        }
        if self.id.trim().is_empty() {
            bail!("id must not be empty");
        }
        if self.id.chars().any(char::is_control) {
            bail!("id contains a disallowed control character");
        }
        for (field, value) in [
            ("background", self.background.as_str()),
            ("foreground", self.foreground.as_str()),
            ("accent", self.accent.as_str()),
            ("correct", self.correct.as_str()),
            ("error", self.error.as_str()),
            ("cursor", self.cursor.as_str()),
            ("dim", self.dim.as_str()),
        ] {
            parse_color(value).with_context(|| format!("invalid {field} color {value:?}"))?;
        }
        Ok(())
    }
}

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

fn parse_color(value: &str) -> Result<ThemeColor> {
    let named = match value.to_ascii_lowercase().as_str() {
        "reset" => Some(ThemeColor::Reset),
        "black" => Some(ThemeColor::Black),
        "red" => Some(ThemeColor::Red),
        "green" => Some(ThemeColor::Green),
        "yellow" => Some(ThemeColor::Yellow),
        "blue" => Some(ThemeColor::Blue),
        "magenta" => Some(ThemeColor::Magenta),
        "cyan" => Some(ThemeColor::Cyan),
        "gray" => Some(ThemeColor::Gray),
        "dark_gray" | "darkgray" => Some(ThemeColor::DarkGray),
        "light_red" | "lightred" => Some(ThemeColor::LightRed),
        "light_green" | "lightgreen" => Some(ThemeColor::LightGreen),
        "light_yellow" | "lightyellow" => Some(ThemeColor::LightYellow),
        "light_blue" | "lightblue" => Some(ThemeColor::LightBlue),
        "light_magenta" | "lightmagenta" => Some(ThemeColor::LightMagenta),
        "light_cyan" | "lightcyan" => Some(ThemeColor::LightCyan),
        "white" => Some(ThemeColor::White),
        _ => None,
    };
    if let Some(color) = named {
        return Ok(color);
    }
    let Some(rgb) = value
        .strip_prefix('#')
        .filter(|rgb| rgb.len() == 6 && rgb.is_ascii())
    else {
        bail!("unknown color");
    };
    let component = |range| {
        u8::from_str_radix(&rgb[range], 16).map_err(|_| anyhow::anyhow!("invalid RGB color"))
    };
    Ok(ThemeColor::Rgb(
        component(0..2)?,
        component(2..4)?,
        component(4..6)?,
    ))
}

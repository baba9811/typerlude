use crate::storage::LoadWarning;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashSet;

mod catalog;

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

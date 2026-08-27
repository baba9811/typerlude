use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use typerlude::{
    i18n::{TextKey, initial_ui_language, text},
    model::Language,
    theme::{ThemeCatalog, ThemeColor, ThemeSpec, parse_theme},
};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "typerlude-ui-foundation-{name}-{}-{}",
            std::process::id(),
            NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn theme_source(id: &str) -> String {
    format!(
        r#"schema_version = 1
id = "{id}"
background = "reset"
foreground = "white"
accent = "cyan"
correct = "green"
error = "red"
cursor = "yellow"
dim = "dark_gray"
"#
    )
}

#[test]
fn every_translation_key_has_distinct_nonempty_korean_and_english_text() {
    assert_eq!(TextKey::ALL.len(), 88);
    assert_eq!(
        TextKey::ALL.iter().copied().collect::<HashSet<_>>().len(),
        TextKey::ALL.len()
    );

    for &key in TextKey::ALL {
        let korean = text(Language::Ko, key);
        let english = text(Language::En, key);
        if key == TextKey::AppTitle {
            assert_eq!(korean, "Typerlude");
            assert_eq!(english, "Typerlude");
            continue;
        }
        assert!(!korean.trim().is_empty(), "Korean {key:?}");
        assert!(!english.trim().is_empty(), "English {key:?}");
        assert_ne!(korean, english, "{key:?}");
    }
}

#[test]
fn locale_default_uses_lc_all_then_lang_and_only_the_language_prefix() {
    for (lc_all, lang, expected) in [
        (Some("ko_KR.UTF-8"), Some("en_US.UTF-8"), Language::Ko),
        (Some("KO-kr"), Some("en"), Language::Ko),
        (Some("ko.UTF-8"), None, Language::Ko),
        (None, Some("kO_KR.UTF-8"), Language::Ko),
        (Some("en_US.UTF-8"), Some("ko_KR.UTF-8"), Language::En),
        (Some("C"), Some("ko"), Language::En),
        (Some(""), Some("ko"), Language::En),
        (None, Some("kok_KR"), Language::En),
        (None, None, Language::En),
    ] {
        assert_eq!(initial_ui_language(lc_all, lang), expected);
    }
}

#[test]
fn every_supported_named_color_and_exact_rgb_boundary_builds_a_palette() {
    let named = [
        ("reset", ThemeColor::Reset),
        ("black", ThemeColor::Black),
        ("red", ThemeColor::Red),
        ("green", ThemeColor::Green),
        ("yellow", ThemeColor::Yellow),
        ("blue", ThemeColor::Blue),
        ("magenta", ThemeColor::Magenta),
        ("cyan", ThemeColor::Cyan),
        ("gray", ThemeColor::Gray),
        ("dark_gray", ThemeColor::DarkGray),
        ("light_red", ThemeColor::LightRed),
        ("light_green", ThemeColor::LightGreen),
        ("light_yellow", ThemeColor::LightYellow),
        ("light_blue", ThemeColor::LightBlue),
        ("light_magenta", ThemeColor::LightMagenta),
        ("light_cyan", ThemeColor::LightCyan),
        ("white", ThemeColor::White),
    ];

    for (name, expected) in named {
        for value in [name.to_owned(), name.to_ascii_uppercase()] {
            let source = theme_source("colors").replace(
                "background = \"reset\"",
                &format!("background = \"{value}\""),
            );
            let theme = parse_theme(&source).unwrap();
            assert_eq!(theme.palette().unwrap().background, expected, "{value}");
        }
    }

    for (value, expected) in [
        ("#000000", ThemeColor::Rgb(0, 0, 0)),
        ("#FFFFFF", ThemeColor::Rgb(255, 255, 255)),
        ("#aBcDeF", ThemeColor::Rgb(0xab, 0xcd, 0xef)),
    ] {
        let source = theme_source("rgb").replace(
            "background = \"reset\"",
            &format!("background = \"{value}\""),
        );
        assert_eq!(
            parse_theme(&source).unwrap().palette().unwrap().background,
            expected
        );
    }
}

#[test]
fn invalid_colors_from_direct_deserialization_or_mutation_return_errors() {
    let directly_deserialized: ThemeSpec = toml::from_str(
        &theme_source("direct-invalid")
            .replace("background = \"reset\"", "background = \"not-a-color\""),
    )
    .unwrap();
    let error = directly_deserialized.palette().unwrap_err();
    assert!(error.to_string().contains("background"), "{error:#}");

    let mut mutated = parse_theme(&theme_source("mutated-invalid")).unwrap();
    mutated.cursor = "not-a-color".into();
    let error = mutated.palette().unwrap_err();
    assert!(error.to_string().contains("cursor"), "{error:#}");
}

#[test]
fn exact_theme_schema_rejects_every_invalid_role_and_malformed_shape() {
    for field in [
        "background",
        "foreground",
        "accent",
        "correct",
        "error",
        "cursor",
        "dim",
    ] {
        let source = theme_source("bad-color").replace(
            &format!(
                "{field} = \"{}\"",
                if field == "dim" {
                    "dark_gray"
                } else if field == "background" {
                    "reset"
                } else if field == "foreground" {
                    "white"
                } else if field == "accent" {
                    "cyan"
                } else if field == "correct" {
                    "green"
                } else if field == "error" {
                    "red"
                } else {
                    "yellow"
                }
            ),
            &format!("{field} = \"not-a-color\""),
        );
        let error = parse_theme(&source).unwrap_err();
        assert!(error.to_string().contains(field), "{field}: {error:#}");
    }

    for invalid in [
        "000000", "#00000", "#0000000", "#GG0000", "#aéaaa", " #000000", "#000000 ",
    ] {
        let source = theme_source("bad-rgb").replace("reset", invalid);
        assert!(parse_theme(&source).is_err(), "{invalid}");
    }

    let bad_schema = theme_source("schema").replace("schema_version = 1", "schema_version = 2");
    assert!(
        parse_theme(&bad_schema)
            .unwrap_err()
            .to_string()
            .contains("schema_version")
    );
    assert!(
        parse_theme(&theme_source("   "))
            .unwrap_err()
            .to_string()
            .contains("id")
    );
    assert!(parse_theme(&(theme_source("duplicate-field") + "background = \"red\"\n")).is_err());
    assert!(parse_theme(&(theme_source("unknown-field") + "future = true\n")).is_err());
    assert!(parse_theme("schema_version = [").is_err());
}

#[test]
fn five_builtins_have_exact_deterministic_ids_and_role_palettes() {
    let catalog = ThemeCatalog::load_builtins().unwrap();
    assert_eq!(
        catalog.ids().collect::<Vec<_>>(),
        ["default", "matrix", "minimal", "monochrome", "nord"]
    );

    let expected = [
        (
            "default",
            [
                "black",
                "white",
                "cyan",
                "green",
                "red",
                "yellow",
                "dark_gray",
            ],
            [
                ThemeColor::Black,
                ThemeColor::White,
                ThemeColor::Cyan,
                ThemeColor::Green,
                ThemeColor::Red,
                ThemeColor::Yellow,
                ThemeColor::DarkGray,
            ],
        ),
        (
            "matrix",
            [
                "black",
                "light_green",
                "green",
                "light_green",
                "light_red",
                "white",
                "green",
            ],
            [
                ThemeColor::Black,
                ThemeColor::LightGreen,
                ThemeColor::Green,
                ThemeColor::LightGreen,
                ThemeColor::LightRed,
                ThemeColor::White,
                ThemeColor::Green,
            ],
        ),
        (
            "minimal",
            ["black", "white", "white", "green", "red", "white", "gray"],
            [
                ThemeColor::Black,
                ThemeColor::White,
                ThemeColor::White,
                ThemeColor::Green,
                ThemeColor::Red,
                ThemeColor::White,
                ThemeColor::Gray,
            ],
        ),
        (
            "monochrome",
            ["black", "white", "gray", "white", "gray", "white", "gray"],
            [
                ThemeColor::Black,
                ThemeColor::White,
                ThemeColor::Gray,
                ThemeColor::White,
                ThemeColor::Gray,
                ThemeColor::White,
                ThemeColor::Gray,
            ],
        ),
        (
            "nord",
            [
                "#2e3440", "#d8dee9", "#88c0d0", "#a3be8c", "#bf616a", "#ebcb8b", "#81a1c1",
            ],
            [
                ThemeColor::Rgb(0x2e, 0x34, 0x40),
                ThemeColor::Rgb(0xd8, 0xde, 0xe9),
                ThemeColor::Rgb(0x88, 0xc0, 0xd0),
                ThemeColor::Rgb(0xa3, 0xbe, 0x8c),
                ThemeColor::Rgb(0xbf, 0x61, 0x6a),
                ThemeColor::Rgb(0xeb, 0xcb, 0x8b),
                ThemeColor::Rgb(0x81, 0xa1, 0xc1),
            ],
        ),
    ];

    for (id, fields, colors) in expected {
        let theme = catalog.get(id).unwrap();
        assert_eq!(theme.schema_version, 1);
        assert_eq!(theme.id, id);
        assert_eq!(
            [
                theme.background.as_str(),
                theme.foreground.as_str(),
                theme.accent.as_str(),
                theme.correct.as_str(),
                theme.error.as_str(),
                theme.cursor.as_str(),
                theme.dim.as_str(),
            ],
            fields,
            "{id}"
        );
        let palette = theme.palette().unwrap();
        assert_eq!(
            [
                palette.background,
                palette.foreground,
                palette.accent,
                palette.correct,
                palette.error,
                palette.cursor,
                palette.dim,
            ],
            colors,
            "{id}"
        );
    }
}

fn relative_luminance(color: ThemeColor) -> f64 {
    let ThemeColor::Rgb(red, green, blue) = color else {
        panic!("contrast regression expects an RGB color, got {color:?}");
    };
    let linear = |component: u8| {
        let value = f64::from(component) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
}

fn contrast_ratio(left: ThemeColor, right: ThemeColor) -> f64 {
    let left = relative_luminance(left);
    let right = relative_luminance(right);
    (left.max(right) + 0.05) / (left.min(right) + 0.05)
}

#[test]
fn builtins_avoid_reset_with_explicit_white_and_nord_dim_meets_contrast_floor() {
    let catalog = ThemeCatalog::load_builtins().unwrap();
    for id in ["default", "minimal"] {
        let theme = catalog.get(id).unwrap();
        assert_eq!(theme.background, "black", "{id}");
        assert_eq!(theme.foreground, "white", "{id}");
    }

    let nord = catalog.get("nord").unwrap();
    assert_eq!(nord.dim, "#81a1c1");
    let palette = nord.palette().unwrap();
    assert!(
        contrast_ratio(palette.background, palette.dim) >= 4.5,
        "Nord dim text must meet WCAG AA contrast"
    );
}

#[test]
fn sorted_user_themes_cannot_shadow_builtins_or_earlier_valid_ids() {
    let root = TestDir::new("user-themes");
    fs::write(root.path().join("a-first.toml"), theme_source("z-user")).unwrap();
    fs::write(
        root.path().join("b-builtin-shadow.toml"),
        theme_source("default"),
    )
    .unwrap();
    fs::write(
        root.path().join("c-user-shadow.toml"),
        theme_source("z-user").replace("accent = \"cyan\"", "accent = \"red\""),
    )
    .unwrap();
    fs::write(root.path().join("d-malformed.toml"), b"schema_version = [").unwrap();
    fs::write(root.path().join("e-second.toml"), theme_source("a-user")).unwrap();

    let loaded = ThemeCatalog::load(root.path()).unwrap();

    assert_eq!(
        loaded.catalog.ids().collect::<Vec<_>>(),
        [
            "default",
            "matrix",
            "minimal",
            "monochrome",
            "nord",
            "z-user",
            "a-user"
        ]
    );
    assert_eq!(loaded.catalog.get("z-user").unwrap().accent, "cyan");
    assert_eq!(
        loaded
            .warnings
            .iter()
            .map(|warning| warning.path.as_path())
            .collect::<Vec<_>>(),
        [
            root.path().join("b-builtin-shadow.toml"),
            root.path().join("c-user-shadow.toml"),
            root.path().join("d-malformed.toml"),
        ]
    );
    assert!(
        loaded
            .warnings
            .iter()
            .all(|warning| !warning.message.is_empty())
    );
}

#[test]
fn invalid_utf8_nonfiles_and_oversized_user_themes_are_path_warnings() {
    let root = TestDir::new("unsafe-user-themes");
    let invalid_utf8 = root.path().join("a-invalid-utf8.toml");
    fs::write(&invalid_utf8, [0xff]).unwrap();
    let directory = root.path().join("b-directory.toml");
    fs::create_dir(&directory).unwrap();
    let oversized = root.path().join("c-oversized.toml");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();

    let loaded = ThemeCatalog::load(root.path()).unwrap();
    assert_eq!(
        loaded
            .warnings
            .iter()
            .map(|warning| warning.path.as_path())
            .collect::<Vec<_>>(),
        [
            invalid_utf8.as_path(),
            directory.as_path(),
            oversized.as_path()
        ]
    );
    assert!(loaded.warnings[2].message.contains("8 MiB"));
}

#[cfg(unix)]
#[test]
fn user_theme_symlinks_are_warnings_and_never_loaded() {
    use std::os::unix::fs::symlink;

    let root = TestDir::new("symlink-theme");
    let outside = root.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let target = outside.join("target");
    fs::write(&target, theme_source("linked-theme")).unwrap();
    let linked = root.path().join("linked.toml");
    symlink(&target, &linked).unwrap();

    let loaded = ThemeCatalog::load(root.path()).unwrap();
    assert!(loaded.catalog.get("linked-theme").is_none());
    assert_eq!(loaded.warnings.len(), 1);
    assert_eq!(loaded.warnings[0].path, linked);
}

const OFFICIAL_NORD_LICENSE: &[u8] = b"MIT License (MIT)\n\nCopyright (c) 2016-present Sven Greb <development@svengreb.de> (https://www.svengreb.de)\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so, subject to the following conditions:\n\nThe above copyright notice and this permission notice shall be included in all\ncopies or substantial portions of the Software.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR\nIMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,\nFITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\nAUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER\nLIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,\nOUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE\nSOFTWARE.\n";

#[test]
fn nord_offline_license_notice_lf_and_package_closure_are_pinned() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let license = fs::read(root.join("assets/licenses/NORD-MIT.txt")).unwrap();
    assert_eq!(license, OFFICIAL_NORD_LICENSE);
    assert_eq!(license.len(), 1_132);

    let notice = fs::read_to_string(root.join("THIRD_PARTY_NOTICES.md")).unwrap();
    for required in [
        "https://github.com/nordtheme/nord",
        "1cef71605416a222e57225b544540ce0fcec18d4",
        "Copyright (c) 2016-present Sven Greb <development@svengreb.de> (https://www.svengreb.de)",
        "MIT",
        "assets/licenses/NORD-MIT.txt",
        "https://raw.githubusercontent.com/nordtheme/nord/1cef71605416a222e57225b544540ce0fcec18d4/src/nord.css",
        "b931ac3732582b2066b2d6cadec02d9820ba7081e6e3e404c31cb62d9315a962",
        "25ac8188d670bd2ad2ce2f4f55ab88573010ee9f7a4502543cb1eea1e2274f8a",
        "unchanged hex values",
        "`#81a1c1` dim",
    ] {
        assert!(
            notice.contains(required),
            "missing Nord notice fact: {required}"
        );
    }

    let attributes = fs::read_to_string(root.join(".gitattributes")).unwrap();
    for required in [
        "assets/themes/*.toml text eol=lf",
        "assets/licenses/*.txt text eol=lf",
    ] {
        assert!(
            attributes.lines().any(|line| line == required),
            "{required}"
        );
    }
    for path in [
        "assets/themes/default.toml",
        "assets/themes/matrix.toml",
        "assets/themes/minimal.toml",
        "assets/themes/monochrome.toml",
        "assets/themes/nord.toml",
        "assets/licenses/NORD-MIT.txt",
    ] {
        let bytes = fs::read(root.join(path)).unwrap();
        assert!(!bytes.contains(&b'\r'), "{path}");
        assert!(bytes.ends_with(b"\n"), "{path}");
    }

    let output = Command::new(env!("CARGO"))
        .args([
            "package",
            "--list",
            "--allow-dirty",
            "--locked",
            "--offline",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packaged = String::from_utf8(output.stdout).unwrap();
    for path in [
        "THIRD_PARTY_NOTICES.md",
        "assets/licenses/NORD-MIT.txt",
        "assets/themes/default.toml",
        "assets/themes/matrix.toml",
        "assets/themes/minimal.toml",
        "assets/themes/monochrome.toml",
        "assets/themes/nord.toml",
    ] {
        assert!(
            packaged.lines().any(|line| line == path),
            "package omitted {path}"
        );
    }
}

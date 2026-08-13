mod args;
mod content;
mod startup;

pub use args::parse_args;
pub use startup::{prepare_app, stdin_command};

use crate::{
    VERSION,
    app::CustomTextSource,
    content::MAX_CONTENT_BYTES,
    diagnostic::terminal_safe,
    model::{Language, PracticeKind},
    storage::AppPaths,
    update::foreground_check,
    user_error,
};
use anyhow::Result;
use std::path::PathBuf;

const HELP: &str = r#"Usage:
  typerlude
  typerlude quick [--lang ko|en] [--time 15|30|60|120]
  typerlude keys|words|sentence|long [--lang ko|en]
  typerlude test [--lang ko|en] [--time 60|180|300|600]
  typerlude FILE | typerlude practice FILE
  typerlude stats | history | themes
  typerlude content list
  typerlude content add PACK.toml
  typerlude content validate [PACK.toml]
  typerlude content disable PACK_ID
  typerlude paths | licenses | update
  typerlude --help | --version | --smoke"#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Home,
    Practice(PracticeArgs),
    File(PathBuf),
    Stdin(String),
    Stats,
    History,
    Content(ContentCommand),
    Themes,
    Paths,
    Licenses,
    Update,
    Version,
    Help,
    Smoke,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentCommand {
    List,
    Add(PathBuf),
    Validate(Option<PathBuf>),
    Disable(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PracticeArgs {
    pub kind: PracticeKind,
    pub language: Option<Language>,
    pub seconds: Option<u64>,
    pub file: Option<PathBuf>,
}

impl PracticeArgs {
    pub const fn new(kind: PracticeKind) -> Self {
        Self {
            kind,
            language: None,
            seconds: None,
            file: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Startup {
    Home,
    Practice(PracticeArgs),
    CustomText {
        source: CustomTextSource,
        name: String,
        text: String,
    },
    Stats,
    History,
    Themes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Exit {
    Done,
    Launch(Startup),
}

fn input_error(message: impl Into<String>) -> anyhow::Error {
    user_error::input_error(message)
}

pub fn is_input_error(error: &anyhow::Error) -> bool {
    user_error::is_input_error(error)
}

pub fn run(command: Command) -> Result<Exit> {
    match command {
        Command::Home => Ok(Exit::Launch(Startup::Home)),
        Command::Practice(args) => {
            if let Some(path) = &args.file {
                startup::custom_file(path)
            } else {
                Ok(Exit::Launch(Startup::Practice(args)))
            }
        }
        Command::File(path) => startup::custom_file(&path),
        Command::Stdin(text) => {
            if text.len() > MAX_CONTENT_BYTES {
                return Err(input_error("stdin exceeds the 8 MiB limit"));
            }
            let text = startup::validate_text(text.into_bytes(), "stdin")?;
            Ok(Exit::Launch(Startup::CustomText {
                source: CustomTextSource::Stdin,
                name: "stdin".into(),
                text,
            }))
        }
        Command::Stats => Ok(Exit::Launch(Startup::Stats)),
        Command::History => Ok(Exit::Launch(Startup::History)),
        Command::Themes => Ok(Exit::Launch(Startup::Themes)),
        Command::Content(command) => {
            content::run(command)?;
            Ok(Exit::Done)
        }
        Command::Paths => {
            print_paths(&AppPaths::discover()?);
            Ok(Exit::Done)
        }
        Command::Licenses => {
            print_licenses();
            Ok(Exit::Done)
        }
        Command::Update => {
            let paths = AppPaths::discover()?;
            let (method, current, latest) = foreground_check(&paths)?;
            println!("current: {current}");
            if let Some(latest) = latest {
                println!("latest: {latest}");
                if latest > current {
                    println!("update: {}", method.instructions());
                } else {
                    println!("Typerlude is up to date.");
                }
            } else {
                println!("latest: see {}", method.instructions());
            }
            println!("Typerlude never installs updates automatically.");
            Ok(Exit::Done)
        }
        Command::Version => {
            println!("typerlude {VERSION}");
            Ok(Exit::Done)
        }
        Command::Help => {
            println!("{HELP}");
            Ok(Exit::Done)
        }
        Command::Smoke => {
            smoke()?;
            Ok(Exit::Done)
        }
    }
}

fn print_paths(paths: &AppPaths) {
    println!("config: {}", terminal_safe(&paths.config.to_string_lossy()));
    println!(
        "sessions: {}",
        terminal_safe(&paths.sessions.to_string_lossy())
    );
    println!(
        "content: {}",
        terminal_safe(&paths.content.to_string_lossy())
    );
    println!("themes: {}", terminal_safe(&paths.themes.to_string_lossy()));
    println!(
        "update-cache: {}",
        terminal_safe(&paths.update_cache.to_string_lossy())
    );
}

fn print_licenses() {
    println!("Typerlude software: MIT");
    println!("Project-authored data: CC0-1.0");
    println!("Tatoeba Korean sentences: CC-BY-2.0-FR (Attribution 2.0 France)");
    println!("Aegukga lyrics: CC-BY-4.0");
    println!("Korean Wikisource editions: CC-BY-SA-4.0");
    println!("Other bundled data: see THIRD_PARTY_NOTICES.md below");
    println!(
        "\n===== LICENSE =====\n{}",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/LICENSE"))
    );
    println!(
        "\n===== THIRD_PARTY_NOTICES.md =====\n{}",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/THIRD_PARTY_NOTICES.md"
        ))
    );
    println!(
        "\n===== assets/licenses/CC0-1.0.txt =====\n{}",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/licenses/CC0-1.0.txt"
        ))
    );
    println!(
        "\n===== assets/licenses/CC-BY-2.0-FR.txt =====\n{}",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/licenses/CC-BY-2.0-FR.txt"
        ))
    );
    println!(
        "\n===== assets/licenses/CC-BY-4.0.txt =====\n{}",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/licenses/CC-BY-4.0.txt"
        ))
    );
    println!(
        "\n===== assets/licenses/CC-BY-SA-4.0.txt =====\n{}",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/licenses/CC-BY-SA-4.0.txt"
        ))
    );
    println!(
        "\n===== assets/licenses/NORD-MIT.txt =====\n{}",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/licenses/NORD-MIT.txt"
        ))
    );
}

fn smoke() -> Result<()> {
    let paths = AppPaths::discover()?;
    let app = startup::build_app(Startup::Home, paths)?;
    for warning in &app.warnings {
        eprintln!("warning: {}", terminal_safe(warning));
    }
    println!(
        "smoke ok: {} content items, {} sessions",
        app.content.items().count(),
        app.sessions.len()
    );
    Ok(())
}

use super::{Command, Exit, Startup, input_error};
use crate::{
    app::{App, CustomTextSource, Screen},
    config::Settings,
    content::{ContentCatalog, MAX_CONTENT_BYTES},
    diagnostic::{format_content_error, terminal_safe},
    storage::{AppPaths, LoadWarning, load_sessions},
    theme::ThemeCatalog,
    update::start_background_check,
};
use anyhow::{Context, Result, bail};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
    time::Instant,
};

pub fn stdin_command() -> Result<Command> {
    let bytes = read_limited(io::stdin().lock(), "stdin")
        .map_err(|error| input_error(error.to_string()))?;
    Ok(Command::Stdin(validate_text(bytes, "stdin")?))
}

pub fn prepare_app(startup: Startup, paths: AppPaths) -> Result<App> {
    let mut app = build_app(startup, paths)?;
    if let Some(receiver) = start_background_check(&app.settings, &app.paths) {
        app.set_update_receiver(receiver);
    }
    Ok(app)
}

pub(super) fn build_app(startup: Startup, paths: AppPaths) -> Result<App> {
    let settings = Settings::load(&paths)?;
    let content = ContentCatalog::load(&paths.content)?;
    let themes = ThemeCatalog::load(&paths.themes)?;
    let sessions = load_sessions(&paths)?;
    let mut warnings = settings
        .warnings
        .iter()
        .map(|warning| format_load_warning("config", warning))
        .collect::<Vec<_>>();
    warnings.extend(
        content
            .warnings
            .iter()
            .map(|warning| format!("content: {}", format_content_error(warning))),
    );
    warnings.extend(
        themes
            .warnings
            .iter()
            .map(|warning| format_load_warning("theme", warning)),
    );
    warnings.extend(
        sessions
            .warnings
            .iter()
            .map(|warning| format_load_warning("session", warning)),
    );

    let mut app = App::new(
        settings.value,
        paths,
        content.catalog,
        themes.catalog,
        sessions.values,
        warnings,
    );
    let now = Instant::now();
    match startup {
        Startup::Home => {}
        Startup::Practice(args) => {
            let language = args.language.unwrap_or(app.settings.language);
            app.start_default(args.kind, language, args.seconds, fastrand::u64(..), now)?;
        }
        Startup::CustomText { source, name, text } => {
            app.start_custom_text(source, &name, &text, now)?
        }
        Startup::Stats => app.open(Screen::Stats),
        Startup::History => app.open(Screen::History),
        Startup::Themes => app.open(Screen::Themes),
    }
    Ok(app)
}

fn format_load_warning(scope: &str, warning: &LoadWarning) -> String {
    format!(
        "{scope}: {}: {}",
        warning.path.to_string_lossy(),
        warning.message
    )
}

pub(super) fn custom_file(path: &Path) -> Result<Exit> {
    let file = File::open(path)
        .map_err(|error| input_error(format!("failed to open {}: {error}", path.display())))?;
    let bytes = read_limited(file, &path.display().to_string())
        .map_err(|error| input_error(error.to_string()))?;
    let text = validate_text(bytes, &path.display().to_string())?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    Ok(Exit::Launch(Startup::CustomText {
        source: CustomTextSource::File,
        name: terminal_safe(&name),
        text,
    }))
}

fn read_limited(reader: impl Read, name: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_CONTENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {name}"))?;
    if bytes.len() > MAX_CONTENT_BYTES {
        bail!("{name} exceeds the 8 MiB limit");
    }
    Ok(bytes)
}

pub(super) fn validate_text(bytes: Vec<u8>, name: &str) -> Result<String> {
    let text = String::from_utf8(bytes)
        .map_err(|error| input_error(format!("{name} is not valid UTF-8: {error}")))?;
    let text = text.replace("\r\n", "\n");
    if text
        .chars()
        .any(|character| character != '\n' && character.is_control())
    {
        return Err(input_error(format!(
            "{name} contains a disallowed control character"
        )));
    }
    if text.trim().is_empty() {
        return Err(input_error(format!("{name} must not be empty")));
    }
    Ok(text)
}

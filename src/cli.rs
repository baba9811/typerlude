use crate::{
    VERSION,
    app::{App, CustomTextSource, Screen},
    config::Settings,
    content::{
        ContentCatalog, ContentError, ContentPack, MAX_CONTENT_BYTES, parse_pack, read_pack_bytes,
        validate_pack,
    },
    diagnostic::{self, format_content_error},
    model::{Language, PracticeKind},
    storage::{AppPaths, LoadWarning, atomic_write_new, load_sessions, rename_no_replace},
    theme::ThemeCatalog,
    update::{foreground_check, start_background_check},
    user_error,
};
use anyhow::{Context, Result, anyhow, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Read},
    path::{Path, PathBuf},
    time::Instant,
};

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

pub fn terminal_safe(value: &str) -> String {
    diagnostic::terminal_safe(value)
}

pub fn parse_args(args: Vec<OsString>) -> Result<Command> {
    if args.is_empty() {
        return Ok(Command::Home);
    }
    let first = args[0].to_str();
    match first {
        Some("quick") => parse_practice(&args, PracticeKind::Quick),
        Some("key" | "keys") => parse_practice(&args, PracticeKind::Key),
        Some("word" | "words") => parse_practice(&args, PracticeKind::Words),
        Some("sentence" | "sentences") => parse_practice(&args, PracticeKind::Sentence),
        Some("long") => parse_practice(&args, PracticeKind::Long),
        Some("test") => parse_practice(&args, PracticeKind::Test),
        Some("practice") => {
            if args.len() != 2 {
                bail!("practice requires exactly one file path");
            }
            let mut parsed = PracticeArgs::new(PracticeKind::Long);
            parsed.file = Some(PathBuf::from(&args[1]));
            Ok(Command::Practice(parsed))
        }
        Some("stats") => exact(&args, Command::Stats),
        Some("history") => exact(&args, Command::History),
        Some("themes") => exact(&args, Command::Themes),
        Some("paths") => exact(&args, Command::Paths),
        Some("licenses") => exact(&args, Command::Licenses),
        Some("update") => exact(&args, Command::Update),
        Some("content") => parse_content(&args),
        Some("--version" | "-V" | "version") => exact(&args, Command::Version),
        Some("--help" | "-h" | "help") => exact(&args, Command::Help),
        Some("--smoke") => exact(&args, Command::Smoke),
        Some(value) if value.starts_with('-') => bail!("unknown option {value:?}"),
        _ => {
            if args.len() != 1 {
                bail!("a direct file path cannot have trailing operands");
            }
            Ok(Command::File(PathBuf::from(&args[0])))
        }
    }
}

fn exact(args: &[OsString], command: Command) -> Result<Command> {
    if args.len() != 1 {
        bail!("unexpected trailing operand");
    }
    Ok(command)
}

fn parse_practice(args: &[OsString], kind: PracticeKind) -> Result<Command> {
    let mut parsed = PracticeArgs::new(kind);
    let mut index = 1;
    while index < args.len() {
        let option = unicode(&args[index], "option")?;
        match option {
            "--lang" => {
                if parsed.language.is_some() {
                    bail!("--lang may be supplied only once");
                }
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("--lang requires ko or en"))?;
                parsed.language = Some(match unicode(value, "--lang value")? {
                    "ko" => Language::Ko,
                    "en" => Language::En,
                    value => bail!("invalid language {value:?}; expected ko or en"),
                });
            }
            "--time" => {
                if !matches!(kind, PracticeKind::Quick | PracticeKind::Test) {
                    bail!("--time is not valid for {kind:?}");
                }
                if parsed.seconds.is_some() {
                    bail!("--time may be supplied only once");
                }
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("--time requires a duration"))?;
                let value = unicode(value, "--time value")?;
                let seconds = value
                    .parse::<u64>()
                    .with_context(|| format!("invalid duration {value:?}"))?;
                let allowed = match kind {
                    PracticeKind::Quick => [15, 30, 60, 120].contains(&seconds),
                    PracticeKind::Test => [60, 180, 300, 600].contains(&seconds),
                    _ => false,
                };
                if !allowed {
                    bail!("invalid duration {seconds} for {kind:?}");
                }
                parsed.seconds = Some(seconds);
            }
            value => bail!("unknown or trailing option {value:?}"),
        }
        index += 1;
    }
    Ok(Command::Practice(parsed))
}

fn parse_content(args: &[OsString]) -> Result<Command> {
    let action = args
        .get(1)
        .ok_or_else(|| anyhow!("content requires list, add, validate, or disable"))?;
    match unicode(action, "content action")? {
        "list" if args.len() == 2 => Ok(Command::Content(ContentCommand::List)),
        "add" if args.len() == 3 => Ok(Command::Content(ContentCommand::Add(PathBuf::from(
            &args[2],
        )))),
        "validate" if args.len() == 2 => Ok(Command::Content(ContentCommand::Validate(None))),
        "validate" if args.len() == 3 => Ok(Command::Content(ContentCommand::Validate(Some(
            PathBuf::from(&args[2]),
        )))),
        "disable" if args.len() == 3 => Ok(Command::Content(ContentCommand::Disable(
            unicode(&args[2], "pack ID")?.to_owned(),
        ))),
        action => bail!("invalid content command or operands: {action}"),
    }
}

fn unicode<'a>(value: &'a OsString, name: &str) -> Result<&'a str> {
    value
        .to_str()
        .ok_or_else(|| anyhow!("{name} must be valid Unicode"))
}

pub fn stdin_command() -> Result<Command> {
    let bytes = read_limited(io::stdin().lock(), "stdin")
        .map_err(|error| input_error(error.to_string()))?;
    Ok(Command::Stdin(validate_text(bytes, "stdin")?))
}

pub fn run(command: Command) -> Result<Exit> {
    match command {
        Command::Home => Ok(Exit::Launch(Startup::Home)),
        Command::Practice(args) => {
            if let Some(path) = &args.file {
                custom_file(path)
            } else {
                Ok(Exit::Launch(Startup::Practice(args)))
            }
        }
        Command::File(path) => custom_file(&path),
        Command::Stdin(text) => {
            if text.len() > MAX_CONTENT_BYTES {
                return Err(input_error("stdin exceeds the 8 MiB limit"));
            }
            let text = validate_text(text.into_bytes(), "stdin")?;
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
            run_content(command)?;
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

pub fn prepare_app(startup: Startup, paths: AppPaths) -> Result<App> {
    let mut app = build_app(startup, paths)?;
    if let Some(receiver) = start_background_check(&app.settings, &app.paths) {
        app.set_update_receiver(receiver);
    }
    Ok(app)
}

fn build_app(startup: Startup, paths: AppPaths) -> Result<App> {
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

fn custom_file(path: &Path) -> Result<Exit> {
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

fn validate_text(bytes: Vec<u8>, name: &str) -> Result<String> {
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

fn run_content(command: ContentCommand) -> Result<()> {
    let paths = AppPaths::discover()?;
    match command {
        ContentCommand::List => list_content(&paths),
        ContentCommand::Validate(path) => validate_content(&paths, path.as_deref()),
        ContentCommand::Add(path) => add_content(&paths, &path),
        ContentCommand::Disable(id) => disable_content(&paths, &id),
    }
}

fn load_catalog(paths: &AppPaths) -> Result<ContentCatalog> {
    let loaded = ContentCatalog::load(&paths.content)?;
    print_content_warnings(&loaded.warnings);
    Ok(loaded.catalog)
}

fn list_content(paths: &AppPaths) -> Result<()> {
    struct Summary {
        language: Language,
        items: usize,
        licenses: BTreeSet<String>,
        sources: BTreeSet<String>,
    }

    let catalog = load_catalog(paths)?;
    let mut packs = BTreeMap::<String, Summary>::new();
    for item in catalog.items() {
        let summary = packs
            .entry(item.pack_id.clone())
            .or_insert_with(|| Summary {
                language: item.language,
                items: 0,
                licenses: BTreeSet::new(),
                sources: BTreeSet::new(),
            });
        summary.items += 1;
        summary.licenses.insert(item.source.license.clone());
        summary.sources.insert(item.source.source_url.clone());
    }
    for (id, summary) in packs {
        println!(
            "{id}\tlanguage={}\titems={}\tlicense={}\tsource={}",
            language_name(summary.language),
            summary.items,
            summary.licenses.into_iter().collect::<Vec<_>>().join(","),
            summary.sources.into_iter().collect::<Vec<_>>().join(",")
        );
    }
    Ok(())
}

fn validate_content(paths: &AppPaths, path: Option<&Path>) -> Result<()> {
    if let Some(path) = path {
        let (pack, _) = candidate(path)?;
        let loaded = ContentCatalog::load_excluding(&paths.content, Some(path))?;
        print_content_warnings(&loaded.warnings);
        reject_content_errors(loaded.catalog.validate_candidate(&pack))?;
        println!("valid content pack: {}", pack.id);
    } else {
        let loaded = ContentCatalog::load(&paths.content)?;
        print_content_warnings(&loaded.warnings);
        if !loaded.warnings.is_empty() {
            return Err(input_error(format!(
                "{} active content pack warning(s)",
                loaded.warnings.len()
            )));
        }
        println!("active content is valid");
    }
    Ok(())
}

fn add_content(paths: &AppPaths, path: &Path) -> Result<()> {
    let (pack, bytes) = candidate(path)?;
    validate_add_id(&pack.id)?;
    reject_content_errors(validate_pack(&pack))?;

    fs::create_dir_all(&paths.content)
        .with_context(|| format!("failed to create {}", paths.content.display()))?;
    let _lock = ContentLock::acquire(&paths.content)?;
    let catalog = load_catalog(paths)?;
    reject_content_errors(catalog.validate_candidate(&pack))?;
    let destination = paths.content.join(format!("{}.toml", pack.id));
    match atomic_write_new(&destination, &bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            return Err(input_error(format!(
                "content destination already exists: {}",
                destination.display()
            )));
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to add {}", destination.display()));
        }
    }
    println!("added content pack {}", pack.id);
    Ok(())
}

fn candidate(path: &Path) -> Result<(ContentPack, Vec<u8>)> {
    let bytes = read_pack_bytes(path).map_err(|error| input_error(format!("{error:#}")))?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| input_error(format!("{} is not valid UTF-8: {error}", path.display())))?;
    let pack = parse_pack(source)
        .map_err(|error| input_error(format!("invalid {}: {error:#}", path.display())))?;
    Ok((pack, bytes))
}

fn disable_content(paths: &AppPaths, id: &str) -> Result<()> {
    let mut warnings = Vec::new();
    let result = disable_user_pack(paths, id, &mut warnings);
    print_content_warnings(&warnings);
    let _catalog = result?;
    println!("disabled content pack {id}");
    Ok(())
}

pub(crate) fn disable_user_pack(
    paths: &AppPaths,
    id: &str,
    warnings: &mut Vec<ContentError>,
) -> Result<ContentCatalog> {
    validate_disable_id(id)?;
    if ContentCatalog::load_builtins()?.contains_pack(id) {
        return Err(input_error(format!(
            "built-in content pack {id:?} cannot be disabled"
        )));
    }
    fs::create_dir_all(&paths.content)
        .with_context(|| format!("failed to create {}", paths.content.display()))?;
    let _lock = ContentLock::acquire(&paths.content)?;
    let loaded = ContentCatalog::load(&paths.content)?;
    warnings.extend(loaded.warnings.iter().cloned());
    let mut catalog = loaded.catalog;
    let source = catalog
        .active_user_path(id)
        .map(Path::to_path_buf)
        .ok_or_else(|| input_error(format!("enabled user content pack {id:?} was not found")))?;
    if !fs::symlink_metadata(&source)
        .with_context(|| format!("failed to inspect {}", source.display()))?
        .file_type()
        .is_file()
    {
        bail!("active content source is no longer a regular file");
    }
    let disabled = ensure_disabled_dir(&paths.content)?;
    let file_name = source.file_name().context("content file has no filename")?;
    let destination = disabled.join(file_name);
    match rename_no_replace(&source, &destination) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            return Err(input_error(format!(
                "disabled content destination already exists: {}",
                destination.display()
            )));
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to rename {} to {}",
                    source.display(),
                    destination.display()
                )
            });
        }
    }
    catalog.remove_pack(id);
    Ok(catalog)
}

fn ensure_disabled_dir(content: &Path) -> Result<PathBuf> {
    let content = fs::canonicalize(content)
        .with_context(|| format!("failed to resolve {}", content.display()))?;
    let disabled = content.join("disabled");
    match fs::create_dir(&disabled) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(error).with_context(|| format!("failed to create {}", disabled.display()));
        }
    }
    let metadata = fs::symlink_metadata(&disabled)
        .with_context(|| format!("failed to inspect {}", disabled.display()))?;
    if !metadata.file_type().is_dir() {
        bail!("{} must be a real directory", disabled.display());
    }
    let disabled = fs::canonicalize(&disabled)
        .with_context(|| format!("failed to resolve {}", disabled.display()))?;
    if disabled.parent() != Some(content.as_path()) {
        bail!("disabled content directory is outside the content root");
    }
    Ok(disabled)
}

fn validate_add_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || is_windows_device_name(id)
    {
        return Err(input_error(format!(
            "pack ID {id:?} is not a portable filename"
        )));
    }
    Ok(())
}

fn is_windows_device_name(id: &str) -> bool {
    let upper = id.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            upper.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
        })
}

fn validate_disable_id(id: &str) -> Result<()> {
    if id.is_empty()
        || matches!(id, "." | "..")
        || id
            .chars()
            .any(|character| matches!(character, '/' | '\\') || character.is_control())
    {
        return Err(input_error(format!("invalid pack ID {id:?}")));
    }
    Ok(())
}

fn reject_content_errors(errors: Vec<ContentError>) -> Result<()> {
    if errors.is_empty() {
        return Ok(());
    }
    Err(input_error(
        errors
            .iter()
            .map(format_content_error)
            .collect::<Vec<_>>()
            .join("; "),
    ))
}

fn print_content_warnings(warnings: &[ContentError]) {
    for warning in warnings {
        eprintln!("warning: {}", format_content_error(warning));
    }
}

fn language_name(language: Language) -> &'static str {
    match language {
        Language::Ko => "ko",
        Language::En => "en",
    }
}

struct ContentLock {
    _file: File,
}

impl ContentLock {
    fn acquire(content: &Path) -> Result<Self> {
        let path = content.join(".typerlude-content.lock");
        let file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if !fs::symlink_metadata(&path)
                    .with_context(|| format!("failed to inspect {}", path.display()))?
                    .file_type()
                    .is_file()
                {
                    bail!("{} must be a regular lock file", path.display());
                }
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path)
                    .with_context(|| format!("failed to open {}", path.display()))?
            }
            Err(error) => {
                return Err(error).with_context(|| format!("failed to create {}", path.display()));
            }
        };
        match fs4::FileExt::try_lock(&file) {
            Ok(()) => Ok(Self { _file: file }),
            Err(fs4::TryLockError::WouldBlock) => bail!("content is already being changed"),
            Err(fs4::TryLockError::Error(error)) => {
                Err(error).context("failed to lock content mutations")
            }
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
    println!("\n===== LICENSE =====\n{}", include_str!("../LICENSE"));
    println!(
        "\n===== THIRD_PARTY_NOTICES.md =====\n{}",
        include_str!("../THIRD_PARTY_NOTICES.md")
    );
    println!(
        "\n===== assets/licenses/CC0-1.0.txt =====\n{}",
        include_str!("../assets/licenses/CC0-1.0.txt")
    );
    println!(
        "\n===== assets/licenses/CC-BY-2.0-FR.txt =====\n{}",
        include_str!("../assets/licenses/CC-BY-2.0-FR.txt")
    );
    println!(
        "\n===== assets/licenses/CC-BY-4.0.txt =====\n{}",
        include_str!("../assets/licenses/CC-BY-4.0.txt")
    );
    println!(
        "\n===== assets/licenses/CC-BY-SA-4.0.txt =====\n{}",
        include_str!("../assets/licenses/CC-BY-SA-4.0.txt")
    );
    println!(
        "\n===== assets/licenses/NORD-MIT.txt =====\n{}",
        include_str!("../assets/licenses/NORD-MIT.txt")
    );
}

fn smoke() -> Result<()> {
    let paths = AppPaths::discover()?;
    let app = build_app(Startup::Home, paths)?;
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

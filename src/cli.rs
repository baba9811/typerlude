use crate::{
    VERSION,
    config::Settings,
    content::{
        ContentCatalog, ContentError, ContentPack, MAX_CONTENT_BYTES, parse_pack, read_pack_bytes,
        validate_pack,
    },
    model::{Language, PracticeKind},
    storage::{AppPaths, atomic_write, load_sessions},
};
use anyhow::{Context, Result, anyhow, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    ffi::OsString,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
};

const HELP: &str = r#"Usage:
  typeul
  typeul quick [--lang ko|en] [--time 15|30|60|120]
  typeul keys|words|sentence|long [--lang ko|en]
  typeul test [--lang ko|en] [--time 60|180|300|600]
  typeul FILE | typeul practice FILE
  typeul stats | history | themes
  typeul content list
  typeul content add PACK.toml
  typeul content validate [PACK.toml]
  typeul content disable PACK_ID
  typeul paths | licenses | update
  typeul --help | --version | --smoke"#;

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
    pub word_count: Option<usize>,
    pub file: Option<PathBuf>,
}

impl PracticeArgs {
    pub const fn new(kind: PracticeKind) -> Self {
        Self {
            kind,
            language: None,
            seconds: None,
            word_count: None,
            file: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Startup {
    Home,
    Practice(PracticeArgs),
    CustomText { name: String, text: String },
    Stats,
    History,
    Themes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Exit {
    Done,
    Launch(Startup),
}

#[derive(Debug)]
struct InputError(String);

impl fmt::Display for InputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for InputError {}

fn input_error(message: impl Into<String>) -> anyhow::Error {
    anyhow!(InputError(message.into()))
}

pub fn is_input_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<InputError>().is_some()
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
            if text.trim().is_empty() {
                return Err(input_error("stdin must not be empty"));
            }
            Ok(Exit::Launch(Startup::CustomText {
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
            println!("typeul {VERSION}");
            println!("Releases: https://github.com/baba9811/typeul/releases");
            println!("Typeul never installs updates automatically.");
            Ok(Exit::Done)
        }
        Command::Version => {
            println!("typeul {VERSION}");
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
    Ok(Exit::Launch(Startup::CustomText { name, text }))
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
    let loaded = ContentCatalog::load(&paths.content)?;
    print_content_warnings(&loaded.warnings);
    if path.is_none() && !loaded.warnings.is_empty() {
        return Err(input_error(format!(
            "{} active content pack warning(s)",
            loaded.warnings.len()
        )));
    }
    let catalog = loaded.catalog;
    if let Some(path) = path {
        let (pack, _) = candidate(path)?;
        validate_safe_id(&pack.id)?;
        reject_content_errors(catalog.validate_candidate(&pack))?;
        println!("valid content pack: {}", pack.id);
    } else {
        println!("active content is valid");
    }
    Ok(())
}

fn add_content(paths: &AppPaths, path: &Path) -> Result<()> {
    let (pack, bytes) = candidate(path)?;
    validate_safe_id(&pack.id)?;
    reject_content_errors(validate_pack(&pack))?;

    fs::create_dir_all(&paths.content)
        .with_context(|| format!("failed to create {}", paths.content.display()))?;
    let _lock = ContentLock::acquire(&paths.content)?;
    let catalog = load_catalog(paths)?;
    reject_content_errors(catalog.validate_candidate(&pack))?;
    let destination = paths.content.join(format!("{}.toml", pack.id));
    match destination.try_exists() {
        Ok(true) => {
            return Err(input_error(format!(
                "content destination already exists: {}",
                destination.display()
            )));
        }
        Ok(false) => {}
        Err(error) => return Err(error).context("failed to inspect content destination"),
    }
    atomic_write(&destination, &bytes)
        .with_context(|| format!("failed to add {}", destination.display()))?;
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
    validate_safe_id(id)?;
    if ContentCatalog::load_builtins()?.contains_pack(id) {
        return Err(input_error(format!(
            "built-in content pack {id:?} cannot be disabled"
        )));
    }
    fs::create_dir_all(&paths.content)
        .with_context(|| format!("failed to create {}", paths.content.display()))?;
    let _lock = ContentLock::acquire(&paths.content)?;
    let mut matches = Vec::new();
    let mut entries = fs::read_dir(&paths.content)
        .with_context(|| format!("failed to read {}", paths.content.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_unstable_by_key(|entry| entry.file_name());
    for entry in entries {
        if !entry.file_type()?.is_file()
            || entry.path().extension().and_then(|ext| ext.to_str()) != Some("toml")
        {
            continue;
        }
        let path = entry.path();
        let bytes = match read_pack_bytes(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("warning: {}: {error:#}", path.display());
                continue;
            }
        };
        let pack = std::str::from_utf8(&bytes)
            .ok()
            .and_then(|source| parse_pack(source).ok());
        if pack.as_ref().is_some_and(|pack| pack.id == id) {
            matches.push(path);
        }
    }
    let source = match matches.as_slice() {
        [] => {
            return Err(input_error(format!(
                "enabled user content pack {id:?} was not found"
            )));
        }
        [source] => source,
        _ => {
            return Err(input_error(format!(
                "multiple enabled files declare pack ID {id:?}"
            )));
        }
    };
    let disabled = paths.content.join("disabled");
    let file_name = source.file_name().context("content file has no filename")?;
    let destination = disabled.join(file_name);
    match destination.try_exists() {
        Ok(true) => {
            return Err(input_error(format!(
                "disabled content destination already exists: {}",
                destination.display()
            )));
        }
        Ok(false) => {}
        Err(error) => return Err(error).context("failed to inspect disabled destination"),
    }
    fs::create_dir_all(&disabled)
        .with_context(|| format!("failed to create {}", disabled.display()))?;
    fs::rename(source, &destination).with_context(|| {
        format!(
            "failed to move {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    println!("disabled content pack {id}");
    Ok(())
}

fn validate_safe_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(input_error(format!(
            "pack ID {id:?} must use 1-128 ASCII letters, digits, '-' or '_'"
        )));
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

fn format_content_error(error: &ContentError) -> String {
    let item = error
        .item_id
        .as_deref()
        .map_or(String::new(), |item| format!(" item={item}"));
    format!(
        "pack={}{} field={}: {}",
        error.pack_id, item, error.field, error.message
    )
}

fn language_name(language: Language) -> &'static str {
    match language {
        Language::Ko => "ko",
        Language::En => "en",
    }
}

struct ContentLock(PathBuf);

impl ContentLock {
    fn acquire(content: &Path) -> Result<Self> {
        let path = content.join(".typeul-content.lock");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .context("content is already being changed")?;
        Ok(Self(path))
    }
}

impl Drop for ContentLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn print_paths(paths: &AppPaths) {
    println!("config: {}", paths.config.display());
    println!("sessions: {}", paths.sessions.display());
    println!("content: {}", paths.content.display());
    println!("themes: {}", paths.themes.display());
    println!("update-cache: {}", paths.update_cache.display());
}

fn print_licenses() {
    println!("Typeul software: MIT");
    println!("Project-authored data: CC0-1.0");
    println!("Tatoeba Korean sentences: CC-BY-2.0-FR (Attribution 2.0 France)");
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
}

fn smoke() -> Result<()> {
    let paths = AppPaths::discover()?;
    let settings = Settings::load(&paths)?;
    for warning in settings.warnings {
        eprintln!("warning: {}: {}", warning.path.display(), warning.message);
    }
    let content = ContentCatalog::load(&paths.content)?;
    print_content_warnings(&content.warnings);
    let sessions = load_sessions(&paths)?;
    for warning in sessions.warnings {
        eprintln!("warning: {}: {}", warning.path.display(), warning.message);
    }
    println!(
        "smoke ok: {} content items, {} sessions",
        content.catalog.items().count(),
        sessions.values.len()
    );
    Ok(())
}

use super::{ContentCommand, input_error};
use crate::{
    content::{
        ContentCatalog, ContentError, ContentPack, MutationLock, disable_user_pack, parse_pack,
        read_pack_bytes, validate_pack,
    },
    diagnostic::format_content_error,
    model::Language,
    storage::{AppPaths, atomic_write_new},
};
use anyhow::{Context, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::ErrorKind,
    path::Path,
};

pub(super) fn run(command: ContentCommand) -> Result<()> {
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
    let _lock = MutationLock::acquire(&paths.content)?;
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

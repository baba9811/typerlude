use super::{ContentCatalog, ContentError};
use crate::{
    storage::{AppPaths, rename_no_replace},
    user_error::input_error,
};
use anyhow::{Context, Result, bail};
use std::{
    fs::{self, File, OpenOptions},
    io::ErrorKind,
    path::{Path, PathBuf},
};

pub(crate) struct MutationLock {
    _file: File,
}

impl MutationLock {
    pub(crate) fn acquire(content: &Path) -> Result<Self> {
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
    let _lock = MutationLock::acquire(&paths.content)?;
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

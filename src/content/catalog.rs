use super::{
    CatalogLoad, ContentCatalog, ContentError, ContentItem, ContentKind, ContentPack,
    MAX_CONTENT_BYTES, ResolvedItem, SourceMeta, parse_pack,
    validation::{
        error, file_error, format_errors, validate_builtin_typeability, validate_builtin_words,
        validate_pack,
    },
};
use crate::{
    model::{Difficulty, Language},
    typing::normalize_nfc,
};
use anyhow::{Context, Result, bail};
use include_dir::{Dir, File, include_dir};
use std::{
    fs::{self, File as FsFile},
    io::Read,
    path::{Path, PathBuf},
};
use unicode_segmentation::UnicodeSegmentation;

static BUILTIN: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/content");

pub(super) fn builtin_pack_source<'file, 'data>(
    file: &'file File<'data>,
) -> Result<Option<(&'data str, &'file str)>> {
    if file.path().extension().and_then(|ext| ext.to_str()) != Some("toml") {
        return Ok(None);
    }
    let path = file.path().to_str().expect("include_dir paths are UTF-8");
    let contents = file
        .contents_utf8()
        .with_context(|| format!("built-in pack {path} is not valid UTF-8"))?;
    Ok(Some((path, contents)))
}

fn builtin_pack_sources() -> impl Iterator<Item = Result<(&'static str, &'static str)>> {
    BUILTIN
        .files()
        .filter_map(|file| builtin_pack_source(file).transpose())
}

impl ContentPack {
    pub fn resolve_items(&self) -> Result<Vec<ResolvedItem>> {
        Ok(self
            .items
            .iter()
            .map(|item| ResolvedItem {
                pack_id: self.id.clone(),
                id: item.id.clone(),
                language: self.language,
                kind: item.kind,
                text: item.text.clone(),
                difficulty: item
                    .difficulty
                    .or_else(|| fallback_difficulty(self.language, item)),
                tags: item.tags.clone(),
                title: item.title.clone(),
                source: item.source.clone().unwrap_or_else(|| self.source.clone()),
            })
            .collect())
    }
}

impl ContentCatalog {
    pub fn load_builtins() -> Result<Self> {
        let mut catalog = Self::default();
        let mut sources: Vec<_> = builtin_pack_sources().collect::<Result<_>>()?;
        sources.sort_unstable_by_key(|(path, _)| *path);

        for (path, source) in sources {
            let pack =
                parse_pack(source).with_context(|| format!("invalid built-in pack {path}"))?;
            let mut errors = validate_pack(&pack);
            errors.extend(validate_builtin_words(&pack));
            errors.extend(validate_builtin_typeability(&pack));
            errors.extend(catalog.conflicts(&pack));
            if !errors.is_empty() {
                bail!("invalid built-in pack {path}: {}", format_errors(&errors));
            }
            catalog.insert(pack)?;
        }
        Ok(catalog)
    }

    pub fn load(user_dir: &Path) -> Result<CatalogLoad> {
        Self::load_excluding(user_dir, None)
    }

    pub(crate) fn load_excluding(
        user_dir: &Path,
        excluded_path: Option<&Path>,
    ) -> Result<CatalogLoad> {
        let mut catalog = Self::load_builtins()?;
        let mut warnings = Vec::new();
        let excluded_path = excluded_path.and_then(canonical_regular_file);
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
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            let fallback_id = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_owned();
            match entry.file_type() {
                Ok(file_type) if file_type.is_file() => {}
                Ok(_) => {
                    warnings.push(file_error(
                        &fallback_id,
                        format!("{} is not a regular file", path.display()),
                    ));
                    continue;
                }
                Err(error) => {
                    warnings.push(file_error(
                        &fallback_id,
                        format!("failed to inspect {}: {error}", path.display()),
                    ));
                    continue;
                }
            }
            if excluded_path
                .as_ref()
                .is_some_and(|excluded| canonical_regular_file(&path).as_ref() == Some(excluded))
            {
                continue;
            }
            let bytes = match read_pack_bytes(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    warnings.push(file_error(&fallback_id, error.to_string()));
                    continue;
                }
            };
            let source = match String::from_utf8(bytes) {
                Ok(source) => source,
                Err(error) => {
                    warnings.push(file_error(&fallback_id, error.to_string()));
                    continue;
                }
            };
            let pack = match parse_pack(&source) {
                Ok(pack) => pack,
                Err(error) => {
                    warnings.push(file_error(&fallback_id, error.to_string()));
                    continue;
                }
            };
            let mut errors = validate_pack(&pack);
            errors.extend(catalog.conflicts(&pack));
            if errors.is_empty() {
                let pack_id = pack.id.clone();
                catalog.insert(pack)?;
                catalog.user_pack_paths.insert(pack_id, path);
            } else {
                warnings.extend(errors);
            }
        }

        Ok(CatalogLoad { catalog, warnings })
    }

    pub fn items(&self) -> impl Iterator<Item = &ResolvedItem> {
        self.items.iter()
    }

    pub fn validate_candidate(&self, pack: &ContentPack) -> Vec<ContentError> {
        let mut errors = validate_pack(pack);
        errors.extend(self.conflicts(pack));
        errors
    }

    pub fn contains_pack(&self, id: &str) -> bool {
        self.pack_ids.contains(id)
    }

    pub(crate) fn active_user_path(&self, id: &str) -> Option<&Path> {
        self.user_pack_paths.get(id).map(PathBuf::as_path)
    }

    pub(crate) fn pack_source(&self, id: &str) -> Option<&SourceMeta> {
        self.pack_sources.get(id)
    }

    pub(crate) fn remove_pack(&mut self, id: &str) {
        let mut retained = Vec::with_capacity(self.items.len());
        for item in self.items.drain(..) {
            if item.pack_id == id {
                self.item_ids.remove(&item.id);
                self.normalized_texts
                    .remove(&(item.language, item.kind, item.text.clone()));
            } else {
                retained.push(item);
            }
        }
        self.items = retained;
        self.pack_ids.remove(id);
        self.pack_sources.remove(id);
        self.user_pack_paths.remove(id);
    }

    pub fn count(&self, language: Language, kind: ContentKind) -> usize {
        self.items
            .iter()
            .filter(|item| item.language == language && item.kind == kind)
            .count()
    }

    pub fn count_any(&self, language: Language, kinds: &[ContentKind]) -> usize {
        self.items
            .iter()
            .filter(|item| item.language == language && kinds.contains(&item.kind))
            .count()
    }

    pub fn select(
        &self,
        language: Language,
        kind: ContentKind,
        difficulty: Difficulty,
    ) -> Vec<&ResolvedItem> {
        let difficulty = match difficulty {
            Difficulty::Easy => Some(1),
            Difficulty::Medium => Some(2),
            Difficulty::Hard => Some(3),
            Difficulty::Mixed => None,
        };
        self.items
            .iter()
            .filter(|item| {
                item.language == language
                    && item.kind == kind
                    && difficulty.is_none_or(|difficulty| item.difficulty == Some(difficulty))
            })
            .collect()
    }

    pub(super) fn conflicts(&self, pack: &ContentPack) -> Vec<ContentError> {
        let mut errors = Vec::new();
        if self.pack_ids.contains(&pack.id) {
            errors.push(error(pack, None, "id", "duplicate pack ID"));
        }
        for item in &pack.items {
            if self.item_ids.contains(&item.id) {
                errors.push(error(
                    pack,
                    Some(&item.id),
                    "items.id",
                    "duplicate catalog item ID",
                ));
            }
            if self.normalized_texts.contains(&(
                pack.language,
                item.kind,
                normalize_nfc(&item.text),
            )) {
                errors.push(error(
                    pack,
                    Some(&item.id),
                    "items.text",
                    "duplicate normalized catalog text",
                ));
            }
        }
        errors
    }

    pub(super) fn insert(&mut self, pack: ContentPack) -> Result<()> {
        let items = pack.resolve_items()?;
        self.pack_sources.insert(pack.id.clone(), pack.source);
        self.pack_ids.insert(pack.id);
        for item in &items {
            self.item_ids.insert(item.id.clone());
            self.normalized_texts
                .insert((item.language, item.kind, normalize_nfc(&item.text)));
        }
        self.items.extend(items);
        Ok(())
    }
}

fn canonical_regular_file(path: &Path) -> Option<PathBuf> {
    fs::symlink_metadata(path)
        .ok()
        .filter(|metadata| metadata.file_type().is_file())
        .and_then(|_| fs::canonicalize(path).ok())
}

pub(crate) fn read_pack_bytes(path: &Path) -> Result<Vec<u8>> {
    let file = FsFile::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_CONTENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if bytes.len() > MAX_CONTENT_BYTES {
        bail!(
            "content pack exceeds the {} MiB limit",
            MAX_CONTENT_BYTES / 1024 / 1024
        );
    }
    Ok(bytes)
}

fn fallback_difficulty(language: Language, item: &ContentItem) -> Option<u8> {
    if item.kind != ContentKind::Word {
        return None;
    }
    let length = item.text.graphemes(true).count();
    Some(match language {
        Language::Ko if length <= 2 => 1,
        Language::Ko if length <= 4 => 2,
        Language::Ko => 3,
        Language::En if length <= 4 => 1,
        Language::En if length <= 8 => 2,
        Language::En => 3,
    })
}

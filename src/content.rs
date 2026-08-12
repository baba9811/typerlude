use crate::{
    model::{Difficulty, Language},
    typing::normalize_nfc,
};
use anyhow::{Context, Result, bail};
use include_dir::{Dir, File, include_dir};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File as FsFile},
    io::Read,
    path::{Path, PathBuf},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    Word,
    Sentence,
    Quote,
    Text,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceMeta {
    pub author: String,
    pub source_id: String,
    pub source_url: String,
    pub license: String,
    pub license_url: String,
    pub modified: bool,
    pub retrieved_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContentItem {
    pub id: String,
    pub kind: ContentKind,
    pub text: String,
    pub difficulty: Option<u8>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub title: Option<String>,
    pub source: Option<SourceMeta>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContentPack {
    pub schema_version: u16,
    pub id: String,
    pub title: String,
    pub language: Language,
    pub source: SourceMeta,
    pub items: Vec<ContentItem>,
}

#[derive(Clone, Debug)]
pub struct ResolvedItem {
    pub pack_id: String,
    pub id: String,
    pub language: Language,
    pub kind: ContentKind,
    pub text: String,
    pub difficulty: Option<u8>,
    pub tags: Vec<String>,
    pub title: Option<String>,
    pub source: SourceMeta,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentError {
    pub pack_id: String,
    pub item_id: Option<String>,
    pub field: String,
    pub message: String,
}

#[derive(Default)]
pub struct ContentCatalog {
    items: Vec<ResolvedItem>,
    pack_ids: HashSet<String>,
    item_ids: HashSet<String>,
    normalized_texts: HashSet<(Language, ContentKind, String)>,
    pack_sources: HashMap<String, SourceMeta>,
    user_pack_paths: HashMap<String, PathBuf>,
}

pub struct CatalogLoad {
    pub catalog: ContentCatalog,
    pub warnings: Vec<ContentError>,
}

const ALLOWED_LICENSES: &[&str] = &[
    "CC0-1.0",
    "CC-BY-2.0-FR",
    "CC-BY-4.0",
    "KOGL-0",
    "KOGL-1.0",
    "LicenseRef-Public-Domain",
];
pub(crate) const MAX_CONTENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_METADATA_COLUMNS: usize = 320;
const MAX_METADATA_BYTES: usize = 1024;

static BUILTIN: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/content");

fn builtin_pack_source<'file, 'data>(
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

pub fn parse_pack(source: &str) -> Result<ContentPack> {
    toml::from_str(source).context("invalid content pack TOML")
}

pub fn validate_pack(pack: &ContentPack) -> Vec<ContentError> {
    let mut errors = Vec::new();
    let mut ids = HashSet::new();
    let mut texts = HashMap::new();

    if pack.schema_version != 1 {
        errors.push(error(pack, None, "schema_version", "must be 1"));
    }
    if pack.id.trim().is_empty() {
        errors.push(error(pack, None, "id", "must not be empty"));
    }
    validate_visible(pack, None, "id", &pack.id, &mut errors);
    validate_visible(pack, None, "title", &pack.title, &mut errors);
    if pack.items.is_empty() {
        errors.push(error(pack, None, "items", "must not be empty"));
    }
    validate_source(pack, None, &pack.source, &mut errors);

    for item in &pack.items {
        let item_id = Some(item.id.as_str());
        if item.id.trim().is_empty() {
            errors.push(error(pack, item_id, "id", "must not be empty"));
        } else if !ids.insert(item.id.as_str()) {
            errors.push(error(pack, item_id, "id", "duplicate item ID"));
        }
        validate_visible(pack, item_id, "id", &item.id, &mut errors);
        if let Some(title) = &item.title {
            validate_visible(pack, item_id, "title", title, &mut errors);
        }
        for tag in &item.tags {
            validate_visible(pack, item_id, "tags", tag, &mut errors);
        }
        if item.text.trim().is_empty() {
            errors.push(error(pack, item_id, "text", "must not be empty"));
        }
        if item.text.chars().any(|ch| ch != '\n' && ch.is_control()) {
            errors.push(error(
                pack,
                item_id,
                "text",
                "contains a disallowed control character",
            ));
        }
        let normalized = normalize_nfc(&item.text);
        if normalized != item.text {
            errors.push(error(pack, item_id, "text", "must be NFC normalized"));
        }
        if let Some(previous) = texts.insert((item.kind, normalized), item.id.as_str()) {
            errors.push(error(
                pack,
                item_id,
                "text",
                &format!("duplicate normalized text from item {previous}"),
            ));
        }
        if item
            .difficulty
            .is_some_and(|value| !(1..=3).contains(&value))
        {
            errors.push(error(
                pack,
                item_id,
                "difficulty",
                "must be between 1 and 3",
            ));
        }
        if let Some(source) = &item.source {
            validate_source(pack, item_id, source, &mut errors);
        }
    }

    errors
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

    fn conflicts(&self, pack: &ContentPack) -> Vec<ContentError> {
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

    fn insert(&mut self, pack: ContentPack) -> Result<()> {
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

fn validate_builtin_words(pack: &ContentPack) -> Vec<ContentError> {
    pack.items
        .iter()
        .filter(|item| item.kind == ContentKind::Word && item.difficulty.is_none())
        .map(|item| {
            error(
                pack,
                Some(&item.id),
                "difficulty",
                "built-in words must declare difficulty",
            )
        })
        .collect()
}

fn validate_builtin_typeability(pack: &ContentPack) -> Vec<ContentError> {
    pack.items
        .iter()
        .filter_map(|item| {
            item.text
                .chars()
                .find(|character| {
                    !(*character == '\n'
                        || *character == ' '
                        || character.is_ascii_graphic()
                        || (pack.language == Language::Ko && ('가'..='힣').contains(character)))
                })
                .map(|character| {
                    error(
                        pack,
                        Some(&item.id),
                        "text",
                        &format!(
                            "contains U+{:04X} {character}, which is not directly typable",
                            character as u32
                        ),
                    )
                })
        })
        .collect()
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

fn validate_source(
    pack: &ContentPack,
    item_id: Option<&str>,
    source: &SourceMeta,
    errors: &mut Vec<ContentError>,
) {
    if !ALLOWED_LICENSES.contains(&source.license.as_str()) {
        errors.push(error(
            pack,
            item_id,
            "source.license",
            "unsupported license",
        ));
    }
    for (field, value) in [
        ("source.author", source.author.as_str()),
        ("source.source_id", source.source_id.as_str()),
        ("source.source_url", source.source_url.as_str()),
        ("source.license", source.license.as_str()),
        ("source.license_url", source.license_url.as_str()),
        ("source.retrieved_at", source.retrieved_at.as_str()),
    ] {
        if value.trim().is_empty() {
            errors.push(error(pack, item_id, field, "must not be empty"));
        }
        validate_visible(pack, item_id, field, value, errors);
    }
    let values = [
        source.author.as_str(),
        source.source_id.as_str(),
        source.source_url.as_str(),
        source.license.as_str(),
        source.license_url.as_str(),
        source.retrieved_at.as_str(),
    ];
    let width = values
        .iter()
        .map(|value| UnicodeWidthStr::width(*value))
        .sum::<usize>();
    let bytes = values.iter().map(|value| value.len()).sum::<usize>();
    if width > MAX_METADATA_COLUMNS || bytes > MAX_METADATA_BYTES {
        errors.push(error(
            pack,
            item_id,
            "source",
            "must be at most 320 terminal columns and 1024 bytes",
        ));
    }
}

fn validate_visible(
    pack: &ContentPack,
    item_id: Option<&str>,
    field: &str,
    value: &str,
    errors: &mut Vec<ContentError>,
) {
    if value.chars().any(char::is_control) {
        errors.push(error(
            pack,
            item_id,
            field,
            "contains a disallowed control character",
        ));
    }
    if UnicodeWidthStr::width(value) > MAX_METADATA_COLUMNS || value.len() > MAX_METADATA_BYTES {
        errors.push(error(
            pack,
            item_id,
            field,
            "must be at most 320 terminal columns and 1024 bytes",
        ));
    }
}

fn error(pack: &ContentPack, item_id: Option<&str>, field: &str, message: &str) -> ContentError {
    ContentError {
        pack_id: pack.id.clone(),
        item_id: item_id.map(str::to_owned),
        field: field.to_owned(),
        message: message.to_owned(),
    }
}

fn file_error(pack_id: &str, message: String) -> ContentError {
    ContentError {
        pack_id: pack_id.to_owned(),
        item_id: None,
        field: "file".to_owned(),
        message,
    }
}

fn format_errors(errors: &[ContentError]) -> String {
    errors
        .iter()
        .map(|error| format!("{}: {}", error.field, error.message))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::{
        ContentCatalog, ContentKind, parse_pack, validate_builtin_typeability,
        validate_builtin_words, validate_pack,
    };
    use crate::model::{Difficulty, Language};
    use include_dir::File;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn fixture_pack() -> super::ContentPack {
        parse_pack(
            r#"
schema_version = 1
id = "fixture"
title = "Fixture"
language = "en"

[source]
author = "Example Author"
source_id = "example"
source_url = "https://example.com/source"
license = "CC-BY-4.0"
license_url = "https://creativecommons.org/licenses/by/4.0/"
modified = false
retrieved_at = "2026-08-07"

[[items]]
id = "fixture-1"
kind = "word"
text = "hello"
difficulty = 2
"#,
        )
        .unwrap()
    }

    #[test]
    fn invalid_utf8_builtin_toml_is_an_error() {
        let file = File::new("invalid.toml", b"\xff");
        let error = super::builtin_pack_source(&file).unwrap_err();
        assert!(error.to_string().contains("invalid.toml"));
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("typerlude-{name}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn valid_attributed_pack_resolves_source_defaults() {
        let pack = parse_pack(include_str!("../assets/content/ko-sentences.toml")).unwrap();
        assert!(validate_pack(&pack).is_empty());
        let item = pack.resolve_items().unwrap().remove(0);
        assert_eq!(item.source.license, "CC-BY-2.0-FR");
        assert_eq!(item.text, "지난 주말에 산으로 소풍을 갔다.");
    }

    #[test]
    fn disallowed_or_incomplete_licenses_fail() {
        let mut pack = fixture_pack();
        pack.source.license = "CC-BY-NC-4.0".into();
        assert!(
            validate_pack(&pack)
                .iter()
                .any(|e| e.field == "source.license")
        );

        pack.source.license = "CC-BY-4.0".into();
        pack.source.author.clear();
        let error = validate_pack(&pack)
            .into_iter()
            .find(|e| e.field == "source.author")
            .unwrap();
        assert_eq!(error.pack_id, "fixture");
        assert_eq!(error.item_id, None);
        assert!(!error.message.is_empty());
    }

    #[test]
    fn schema_version_and_exact_license_allowlist_are_enforced() {
        let mut pack = fixture_pack();
        pack.schema_version = 2;
        assert!(
            validate_pack(&pack)
                .iter()
                .any(|error| error.field == "schema_version")
        );

        pack.schema_version = 1;
        for license in [
            "CC0-1.0",
            "CC-BY-2.0-FR",
            "CC-BY-4.0",
            "KOGL-0",
            "KOGL-1.0",
            "LicenseRef-Public-Domain",
        ] {
            pack.source.license = license.into();
            assert!(validate_pack(&pack).is_empty(), "{license}");
        }
    }

    #[test]
    fn invalid_items_report_stable_fields_and_normalized_duplicates() {
        let mut pack = fixture_pack();
        let mut duplicate = pack.items[0].clone();
        duplicate.id = "fixture-1".into();
        duplicate.text = "he\u{301}llo".into();
        duplicate.difficulty = Some(4);
        pack.items.push(duplicate);

        let errors = validate_pack(&pack);
        assert!(
            errors
                .iter()
                .any(|e| e.item_id.as_deref() == Some("fixture-1") && e.field == "id")
        );
        assert!(
            errors
                .iter()
                .any(|e| e.field == "text" && e.message.contains("NFC"))
        );
        assert!(errors.iter().any(|e| e.field == "difficulty"));

        pack.items[1].text = "hello".into();
        assert!(
            validate_pack(&pack)
                .iter()
                .any(|e| e.field == "text" && e.message.contains("duplicate"))
        );
    }

    #[test]
    fn controls_empty_values_and_missing_modification_statement_fail() {
        let mut pack = fixture_pack();
        pack.id.clear();
        pack.items[0].id.clear();
        pack.items[0].text = "bad\ttext".into();
        pack.source.source_url.clear();
        pack.source.license_url.clear();
        let errors = validate_pack(&pack);
        assert!(
            errors
                .iter()
                .any(|e| e.field == "id" && e.item_id.is_none())
        );
        assert!(
            errors
                .iter()
                .any(|e| e.field == "id" && e.item_id.as_deref() == Some(""))
        );
        assert!(
            errors
                .iter()
                .any(|e| e.field == "text" && e.message.contains("control"))
        );
        assert!(errors.iter().any(|e| e.field == "source.source_url"));
        assert!(errors.iter().any(|e| e.field == "source.license_url"));

        let missing_modified = r#"
schema_version = 1
id = "missing-modified"
title = "Missing modified"
language = "en"
items = []
[source]
author = "Author"
source_id = "source"
source_url = "https://example.com"
license = "CC-BY-4.0"
license_url = "https://creativecommons.org/licenses/by/4.0/"
retrieved_at = "2026-08-07"
"#;
        assert!(format!("{:#}", parse_pack(missing_modified).unwrap_err()).contains("modified"));
        assert!(validate_pack(&fixture_pack()).is_empty());

        let mut newline = fixture_pack();
        newline.items[0].text = "line one\nline two".into();
        assert!(validate_pack(&newline).is_empty());
    }

    #[test]
    fn source_metadata_has_a_bounded_terminal_width() {
        let mut pack = fixture_pack();
        pack.source.author = "x".repeat(400);

        assert!(
            validate_pack(&pack)
                .iter()
                .any(|error| error.field == "source" && error.message.contains("320"))
        );
    }

    #[test]
    fn zero_width_source_metadata_has_a_bounded_byte_length() {
        let mut pack = fixture_pack();
        pack.source.author = "\u{301}".repeat(600);

        assert!(validate_pack(&pack).iter().any(|error| {
            error.field == "source.author" && error.message.contains("1024 bytes")
        }));
    }

    #[test]
    fn removing_a_pack_reconciles_every_catalog_index() {
        let pack = fixture_pack();
        let mut catalog = ContentCatalog::default();
        catalog.insert(pack.clone()).unwrap();
        assert!(catalog.contains_pack("fixture"));

        catalog.remove_pack("fixture");

        assert!(!catalog.contains_pack("fixture"));
        assert!(catalog.items().all(|item| item.pack_id != "fixture"));
        assert!(catalog.pack_source("fixture").is_none());
        assert!(catalog.validate_candidate(&pack).is_empty());
    }

    #[test]
    fn every_terminal_visible_string_rejects_c0_and_c1_controls() {
        let cases = [
            ("pack.id", "id"),
            ("pack.title", "title"),
            ("item.id", "id"),
            ("item.text", "text"),
            ("item.title", "title"),
            ("item.tags.0", "tags"),
            ("item.tags.1", "tags"),
            ("pack.source.author", "source.author"),
            ("pack.source.source_id", "source.source_id"),
            ("pack.source.source_url", "source.source_url"),
            ("pack.source.license", "source.license"),
            ("pack.source.license_url", "source.license_url"),
            ("pack.source.retrieved_at", "source.retrieved_at"),
            ("item.source.author", "source.author"),
            ("item.source.source_id", "source.source_id"),
            ("item.source.source_url", "source.source_url"),
            ("item.source.license", "source.license"),
            ("item.source.license_url", "source.license_url"),
            ("item.source.retrieved_at", "source.retrieved_at"),
        ];

        for control in ['\u{1b}', '\u{9b}'] {
            for (case, expected_field) in cases {
                let mut pack = fixture_pack();
                pack.items[0].title = Some("Item title".into());
                pack.items[0].tags = vec!["first".into(), "second".into()];
                pack.items[0].source = Some(pack.source.clone());
                let value = format!("safe{control}value");
                match case {
                    "pack.id" => pack.id = value,
                    "pack.title" => pack.title = value,
                    "item.id" => pack.items[0].id = value,
                    "item.text" => pack.items[0].text = value,
                    "item.title" => pack.items[0].title = Some(value),
                    "item.tags.0" => pack.items[0].tags[0] = value,
                    "item.tags.1" => pack.items[0].tags[1] = value,
                    "pack.source.author" => pack.source.author = value,
                    "pack.source.source_id" => pack.source.source_id = value,
                    "pack.source.source_url" => pack.source.source_url = value,
                    "pack.source.license" => pack.source.license = value,
                    "pack.source.license_url" => pack.source.license_url = value,
                    "pack.source.retrieved_at" => pack.source.retrieved_at = value,
                    "item.source.author" => pack.items[0].source.as_mut().unwrap().author = value,
                    "item.source.source_id" => {
                        pack.items[0].source.as_mut().unwrap().source_id = value;
                    }
                    "item.source.source_url" => {
                        pack.items[0].source.as_mut().unwrap().source_url = value;
                    }
                    "item.source.license" => {
                        pack.items[0].source.as_mut().unwrap().license = value;
                    }
                    "item.source.license_url" => {
                        pack.items[0].source.as_mut().unwrap().license_url = value;
                    }
                    "item.source.retrieved_at" => {
                        pack.items[0].source.as_mut().unwrap().retrieved_at = value;
                    }
                    _ => unreachable!(),
                }

                let errors = validate_pack(&pack);
                assert!(
                    errors.iter().any(|error| {
                        error.field == expected_field && error.message.contains("control")
                    }),
                    "{case} did not reject U+{:04X}: {errors:?}",
                    control as u32
                );
            }
        }

        let mut newline = fixture_pack();
        newline.items[0].text = "line one\nline two".into();
        assert!(validate_pack(&newline).is_empty());
    }

    #[test]
    fn item_source_is_complete_override_and_user_words_resolve_difficulty() {
        let pack = parse_pack(
            r#"
schema_version = 1
id = "fallback"
title = "Fallback"
language = "ko"
[source]
author = "Pack Author"
source_id = "pack-source"
source_url = "https://example.com/pack"
license = "CC-BY-4.0"
license_url = "https://creativecommons.org/licenses/by/4.0/"
modified = false
retrieved_at = "2026-08-07"
[[items]]
id = "short"
kind = "word"
text = "한글"
[items.source]
author = "Item Author"
source_id = "item-source"
source_url = "https://example.com/item"
license = "KOGL-1.0"
license_url = "https://www.kogl.or.kr/info/licenseType1.do"
modified = true
retrieved_at = "2026-08-06"
"#,
        )
        .unwrap();

        let item = pack.resolve_items().unwrap().remove(0);
        assert_eq!(item.difficulty, Some(1));
        assert_eq!(item.source.author, "Item Author");
        assert_eq!(item.source.source_id, "item-source");
        assert_eq!(item.source.retrieved_at, "2026-08-06");
    }

    #[test]
    fn language_specific_word_thresholds_and_non_words_are_resolved_once() {
        let mut pack = fixture_pack();
        pack.items[0].difficulty = None;
        pack.items[0].text = "ninechars".into();
        let item = pack.resolve_items().unwrap().remove(0);
        assert_eq!(item.difficulty, Some(3));

        pack.items[0].kind = ContentKind::Sentence;
        let item = pack.resolve_items().unwrap().remove(0);
        assert_eq!(item.difficulty, None);
    }

    #[test]
    fn every_word_fallback_boundary_is_grapheme_based() {
        let mut pack = fixture_pack();
        pack.items[0].difficulty = None;
        for (language, text, expected) in [
            (Language::Ko, "한글", 1),
            (Language::Ko, "타자연습", 2),
            (Language::Ko, "정확한타자", 3),
            (Language::En, "type", 1),
            (Language::En, "practice", 2),
            (Language::En, "keystrokes", 3),
        ] {
            pack.language = language;
            pack.items[0].text = text.into();
            assert_eq!(
                pack.resolve_items().unwrap()[0].difficulty,
                Some(expected),
                "{language:?} {text}"
            );
        }
    }

    #[test]
    fn built_in_words_must_declare_difficulty() {
        let mut pack = fixture_pack();
        pack.items[0].difficulty = None;
        let error = validate_builtin_words(&pack).remove(0);
        assert_eq!(error.pack_id, "fixture");
        assert_eq!(error.item_id.as_deref(), Some("fixture-1"));
        assert_eq!(error.field, "difficulty");
    }

    #[test]
    fn built_in_practice_text_requires_direct_keyboard_characters() {
        let mut pack = fixture_pack();
        pack.items[0].text = "plain ASCII".into();
        assert!(validate_builtin_typeability(&pack).is_empty());

        pack.language = Language::Ko;
        pack.items[0].text = "한글과 ASCII 123!?\n다음 줄".into();
        assert!(validate_builtin_typeability(&pack).is_empty());

        for text in ["①항목", "곡선 ‘따옴표’", "한자 漢"] {
            pack.items[0].text = text.into();
            let error = validate_builtin_typeability(&pack).remove(0);
            assert_eq!(error.item_id.as_deref(), Some("fixture-1"));
            assert_eq!(error.field, "text");
            assert!(error.message.contains("directly typable"));
        }
    }

    #[test]
    fn normalized_text_conflicts_are_scoped_by_language_and_kind() {
        let mut within_pack = fixture_pack();
        let mut cross_kind = within_pack.items[0].clone();
        cross_kind.id = "sentence-item".into();
        cross_kind.kind = ContentKind::Sentence;
        within_pack.items.push(cross_kind);
        assert!(
            !validate_pack(&within_pack)
                .iter()
                .any(|error| error.field == "text" && error.message.contains("duplicate"))
        );

        let mut same_kind = within_pack.items[0].clone();
        same_kind.id = "word-item".into();
        within_pack.items.push(same_kind);
        assert!(
            validate_pack(&within_pack)
                .iter()
                .any(|error| error.item_id.as_deref() == Some("word-item")
                    && error.field == "text"
                    && error.message.contains("duplicate"))
        );

        let base = fixture_pack();
        let mut catalog = ContentCatalog::default();
        catalog.insert(base.clone()).unwrap();

        let candidate =
            |pack_id: &str, item_id: &str, language: Language, kind: ContentKind, text: &str| {
                let mut pack = base.clone();
                pack.id = pack_id.into();
                pack.language = language;
                pack.items[0].id = item_id.into();
                pack.items[0].kind = kind;
                pack.items[0].text = text.into();
                pack
            };

        for allowed in [
            candidate(
                "other-language",
                "other-language-item",
                Language::Ko,
                ContentKind::Word,
                "hello",
            ),
            candidate(
                "other-kind",
                "other-kind-item",
                Language::En,
                ContentKind::Sentence,
                "hello",
            ),
        ] {
            assert!(
                !catalog
                    .conflicts(&allowed)
                    .iter()
                    .any(|error| error.field == "items.text"),
                "{}",
                allowed.id
            );
        }

        let same_scope = candidate(
            "same-scope",
            "same-scope-item",
            Language::En,
            ContentKind::Word,
            "hello",
        );
        assert!(
            catalog
                .conflicts(&same_scope)
                .iter()
                .any(|error| error.field == "items.text")
        );

        let duplicate_item = candidate(
            "duplicate-item",
            "fixture-1",
            Language::Ko,
            ContentKind::Quote,
            "unique text",
        );
        assert!(
            catalog
                .conflicts(&duplicate_item)
                .iter()
                .any(|error| error.field == "items.id")
        );

        let duplicate_pack = candidate(
            "fixture",
            "unique-item",
            Language::Ko,
            ContentKind::Quote,
            "other unique text",
        );
        assert!(
            catalog
                .conflicts(&duplicate_pack)
                .iter()
                .any(|error| error.field == "id")
        );
    }

    #[test]
    fn catalog_loads_builtins_skips_conflicting_users_and_ignores_disabled() {
        let builtins = ContentCatalog::load_builtins().unwrap();
        let builtin_en_words = builtins.count(Language::En, ContentKind::Word);
        let builtin_ko_lines =
            builtins.count_any(Language::Ko, &[ContentKind::Sentence, ContentKind::Quote]);
        let builtin_en_easy_words = builtins
            .select(Language::En, ContentKind::Word, Difficulty::Easy)
            .len();
        let dir = temp_dir("catalog");
        fs::write(
            dir.join("a-valid.toml"),
            r#"
schema_version = 1
id = "user-pack"
title = "User Pack"
language = "en"
[source]
author = "User"
source_id = "user"
source_url = "https://example.com/user"
license = "CC0-1.0"
license_url = "https://creativecommons.org/publicdomain/zero/1.0/"
modified = false
retrieved_at = "2026-08-07"
[[items]]
id = "user-word"
kind = "word"
text = "cat"
"#,
        )
        .unwrap();
        let conflict = r#"
schema_version = 1
id = "ko-sentences"
title = "Conflict"
language = "en"
items = []
[source]
author = "User"
source_id = "conflict"
source_url = "https://example.com/conflict"
license = "CC0-1.0"
license_url = "https://creativecommons.org/publicdomain/zero/1.0/"
modified = false
retrieved_at = "2026-08-07"
"#;
        fs::write(dir.join("b-conflict.toml"), conflict).unwrap();
        fs::create_dir(dir.join("disabled")).unwrap();
        fs::write(
            dir.join("disabled/hidden.toml"),
            conflict.replace("ko-sentences", "hidden"),
        )
        .unwrap();

        let loaded = ContentCatalog::load(&dir).unwrap();
        assert_eq!(
            loaded.catalog.count(Language::En, ContentKind::Word),
            builtin_en_words + 1
        );
        assert_eq!(
            loaded
                .catalog
                .count_any(Language::Ko, &[ContentKind::Sentence, ContentKind::Quote]),
            builtin_ko_lines
        );
        assert_eq!(
            loaded
                .catalog
                .select(Language::En, ContentKind::Word, Difficulty::Easy)
                .len(),
            builtin_en_easy_words + 1
        );
        assert!(loaded.catalog.items().all(|item| item.pack_id != "hidden"));
        assert!(
            loaded
                .warnings
                .iter()
                .any(|e| e.pack_id == "ko-sentences" && e.field == "id")
        );
        assert_eq!(
            fs::read_to_string(dir.join("b-conflict.toml")).unwrap(),
            conflict
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_user_pack_conflicting_on_an_item_id_is_skipped_whole() {
        let builtin_item_count = ContentCatalog::load_builtins().unwrap().items().count();
        let dir = temp_dir("item-conflict");
        fs::write(
            dir.join("conflict.toml"),
            include_str!("../assets/content/ko-sentences.toml")
                .replace("id = \"ko-sentences\"", "id = \"other-pack\"")
                .replace("title = \"Korean Sentences\"", "title = \"Other Pack\""),
        )
        .unwrap();

        let loaded = ContentCatalog::load(&dir).unwrap();
        assert_eq!(loaded.catalog.items().count(), builtin_item_count);
        assert!(
            loaded
                .warnings
                .iter()
                .any(|e| e.pack_id == "other-pack" && e.field == "items.id")
        );
        fs::remove_dir_all(dir).unwrap();
    }
}

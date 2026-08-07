use crate::{
    model::{Difficulty, Language},
    typing::normalize_nfc,
};
use anyhow::{Context, Result, bail};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    Word,
    Sentence,
    Quote,
    Text,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    normalized_texts: HashSet<String>,
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

static BUILTIN: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/content");

fn builtin_pack_sources() -> impl Iterator<Item = (&'static str, &'static str)> {
    BUILTIN.files().filter_map(|file| {
        if file.path().extension()?.to_str()? != "toml" {
            return None;
        }
        Some((file.path().to_str()?, file.contents_utf8()?))
    })
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
    validate_source(pack, None, &pack.source, &mut errors);

    for item in &pack.items {
        let item_id = Some(item.id.as_str());
        if item.id.trim().is_empty() {
            errors.push(error(pack, item_id, "id", "must not be empty"));
        } else if !ids.insert(item.id.as_str()) {
            errors.push(error(pack, item_id, "id", "duplicate item ID"));
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
        if let Some(previous) = texts.insert(normalized, item.id.as_str()) {
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
        let mut sources: Vec<_> = builtin_pack_sources().collect();
        sources.sort_unstable_by_key(|(path, _)| *path);

        for (path, source) in sources {
            let pack =
                parse_pack(source).with_context(|| format!("invalid built-in pack {path}"))?;
            let mut errors = validate_pack(&pack);
            errors.extend(validate_builtin_words(&pack));
            errors.extend(catalog.conflicts(&pack));
            if !errors.is_empty() {
                bail!("invalid built-in pack {path}: {}", format_errors(&errors));
            }
            catalog.insert(pack)?;
        }
        Ok(catalog)
    }

    pub fn load(user_dir: &Path) -> Result<CatalogLoad> {
        let mut catalog = Self::load_builtins()?;
        let mut warnings = Vec::new();
        let mut paths = match fs::read_dir(user_dir) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "toml"))
                .collect::<Vec<_>>(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {}", user_dir.display()));
            }
        };
        paths.sort();

        for path in paths {
            let fallback_id = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_owned();
            let source = match fs::read_to_string(&path) {
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
                catalog.insert(pack)?;
            } else {
                warnings.extend(errors);
            }
        }

        Ok(CatalogLoad { catalog, warnings })
    }

    pub fn items(&self) -> impl Iterator<Item = &ResolvedItem> {
        self.items.iter()
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
            if self.normalized_texts.contains(&normalize_nfc(&item.text)) {
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
        self.pack_ids.insert(pack.id);
        for item in &items {
            self.item_ids.insert(item.id.clone());
            self.normalized_texts.insert(normalize_nfc(&item.text));
        }
        self.items.extend(items);
        Ok(())
    }
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
        ("source.license_url", source.license_url.as_str()),
        ("source.retrieved_at", source.retrieved_at.as_str()),
    ] {
        if value.trim().is_empty() {
            errors.push(error(pack, item_id, field, "must not be empty"));
        }
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
    use super::{ContentCatalog, ContentKind, parse_pack, validate_builtin_words, validate_pack};
    use crate::model::{Difficulty, Language};
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

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("typeul-{name}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn valid_attributed_pack_resolves_source_defaults() {
        let pack = parse_pack(include_str!("../assets/content/project-smoke.toml")).unwrap();
        assert!(validate_pack(&pack).is_empty());
        let item = pack.resolve_items().unwrap().remove(0);
        assert_eq!(item.source.license, "CC0-1.0");
        assert_eq!(item.text, "정확하게 입력합니다.");
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
    fn catalog_conflicts_include_normalized_text_across_packs() {
        let catalog = ContentCatalog::load_builtins().unwrap();
        let mut pack = fixture_pack();
        pack.id = "other-pack".into();
        pack.items[0].id = "other-item".into();
        pack.items[0].text = "정확하게 입력합니다.".into();

        let error = catalog
            .conflicts(&pack)
            .into_iter()
            .find(|error| error.field == "items.text")
            .unwrap();
        assert_eq!(error.pack_id, "other-pack");
        assert_eq!(error.item_id.as_deref(), Some("other-item"));
    }

    #[test]
    fn catalog_loads_builtins_skips_conflicting_users_and_ignores_disabled() {
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
id = "project-smoke"
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
            conflict.replace("project-smoke", "hidden"),
        )
        .unwrap();

        let loaded = ContentCatalog::load(&dir).unwrap();
        assert_eq!(loaded.catalog.count(Language::En, ContentKind::Word), 1);
        assert_eq!(
            loaded
                .catalog
                .count_any(Language::Ko, &[ContentKind::Sentence, ContentKind::Quote]),
            1
        );
        assert_eq!(
            loaded
                .catalog
                .select(Language::En, ContentKind::Word, Difficulty::Easy)
                .len(),
            1
        );
        assert!(loaded.catalog.items().all(|item| item.pack_id != "hidden"));
        assert!(
            loaded
                .warnings
                .iter()
                .any(|e| e.pack_id == "project-smoke" && e.field == "id")
        );
        assert_eq!(
            fs::read_to_string(dir.join("b-conflict.toml")).unwrap(),
            conflict
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_user_pack_conflicting_on_an_item_id_is_skipped_whole() {
        let dir = temp_dir("item-conflict");
        fs::write(
            dir.join("conflict.toml"),
            include_str!("../assets/content/project-smoke.toml")
                .replace("id = \"project-smoke\"", "id = \"other-pack\"")
                .replace("title = \"Project Smoke\"", "title = \"Other Pack\""),
        )
        .unwrap();

        let loaded = ContentCatalog::load(&dir).unwrap();
        assert_eq!(loaded.catalog.items().count(), 1);
        assert!(
            loaded
                .warnings
                .iter()
                .any(|e| e.pack_id == "other-pack" && e.field == "items.id")
        );
        fs::remove_dir_all(dir).unwrap();
    }
}

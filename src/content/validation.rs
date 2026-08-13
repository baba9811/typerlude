use super::{ContentError, ContentKind, ContentPack, SourceMeta};
use crate::{model::Language, typing::normalize_nfc};
use std::collections::{HashMap, HashSet};
use unicode_width::UnicodeWidthStr;

const ALLOWED_LICENSES: &[&str] = &[
    "CC0-1.0",
    "CC-BY-2.0-FR",
    "CC-BY-4.0",
    "CC-BY-SA-4.0",
    "KOGL-0",
    "KOGL-1.0",
    "LicenseRef-Public-Domain",
];
const MAX_METADATA_COLUMNS: usize = 320;
const MAX_METADATA_BYTES: usize = 1024;

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

pub(super) fn validate_builtin_words(pack: &ContentPack) -> Vec<ContentError> {
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

pub(super) fn validate_builtin_typeability(pack: &ContentPack) -> Vec<ContentError> {
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

pub(super) fn error(
    pack: &ContentPack,
    item_id: Option<&str>,
    field: &str,
    message: &str,
) -> ContentError {
    ContentError {
        pack_id: pack.id.clone(),
        item_id: item_id.map(str::to_owned),
        field: field.to_owned(),
        message: message.to_owned(),
    }
}

pub(super) fn file_error(pack_id: &str, message: String) -> ContentError {
    ContentError {
        pack_id: pack_id.to_owned(),
        item_id: None,
        field: "file".to_owned(),
        message,
    }
}

pub(super) fn format_errors(errors: &[ContentError]) -> String {
    errors
        .iter()
        .map(|error| format!("{}: {}", error.field, error.message))
        .collect::<Vec<_>>()
        .join("; ")
}

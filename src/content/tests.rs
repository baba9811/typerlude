use super::validation::{validate_builtin_typeability, validate_builtin_words};
use super::{ContentCatalog, ContentKind, parse_pack, validate_pack};
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
    let error = super::catalog::builtin_pack_source(&file).unwrap_err();
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
    let pack = parse_pack(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/content/ko-sentences.toml"
    )))
    .unwrap();
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
        "CC-BY-SA-4.0",
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

    let mut consecutive_newlines = fixture_pack();
    consecutive_newlines.items[0].text = "line one\n\nline two".into();
    assert!(
        validate_pack(&consecutive_newlines).iter().any(|error| {
            error.field == "text" && error.message.contains("consecutive newlines")
        })
    );
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

    assert!(
        validate_pack(&pack).iter().any(|error| {
            error.field == "source.author" && error.message.contains("1024 bytes")
        })
    );
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

    for text in [
        "①항목",
        "봄·봄",
        "말줄임표…",
        "긴—대시",
        "곡선 ‘따옴표’",
        "한자 漢",
    ] {
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
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/content/ko-sentences.toml"
        ))
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

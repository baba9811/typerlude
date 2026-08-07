use std::{collections::HashSet, fs, path::Path};
use typeul::{
    content::{ContentCatalog, ContentKind, ContentPack, parse_pack, validate_pack},
    model::Language,
};
use unicode_segmentation::UnicodeSegmentation;

fn load_word_pack(file_name: &str) -> ContentPack {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/content")
        .join(file_name);
    parse_pack(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn bundled_word_packs_match_the_reviewed_5b1_contract() {
    let catalog = ContentCatalog::load_builtins().unwrap();

    for (language, pack_id, file_name) in [
        (Language::Ko, "ko-words", "ko-words.toml"),
        (Language::En, "en-words", "en-words.toml"),
    ] {
        let items: Vec<_> = catalog
            .items()
            .filter(|item| item.language == language && item.pack_id == pack_id)
            .collect();
        assert_eq!(items.len(), 300, "{pack_id}");
        assert!(items.iter().all(|item| item.kind == ContentKind::Word));

        let pack = load_word_pack(file_name);
        assert!(validate_pack(&pack).is_empty(), "{pack_id}");
        assert_eq!(pack.id, pack_id);
        assert_eq!(pack.language, language);
        assert_eq!(pack.items.len(), 300, "{pack_id}");
        assert!(pack.items.iter().all(|item| item.difficulty.is_some()));
        let item_prefix = pack_id.strip_suffix('s').unwrap();
        for (index, item) in pack.items.iter().enumerate() {
            assert_eq!(item.id, format!("{item_prefix}-{:03}", index + 1));
            assert!(
                item.tags.iter().all(|tag| !tag.trim().is_empty()),
                "{}",
                item.id
            );
        }

        for difficulty in 1..=3 {
            assert_eq!(
                items
                    .iter()
                    .filter(|item| item.difficulty == Some(difficulty))
                    .count(),
                100,
                "{pack_id} difficulty {difficulty}"
            );
        }

        for item in items {
            assert!(
                !item.text.chars().any(char::is_whitespace),
                "{} is not a single token",
                item.id
            );
            assert!(
                item.tags.iter().any(|tag| tag == "vocabulary"),
                "{}",
                item.id
            );
            assert!(
                item.tags.iter().any(|tag| tag != "vocabulary"),
                "{}",
                item.id
            );
            let graphemes = item.text.graphemes(true).count();
            let matches_threshold = match (language, item.difficulty) {
                (Language::Ko, Some(1)) => graphemes <= 2,
                (Language::Ko, Some(2)) => (3..=4).contains(&graphemes),
                (Language::Ko, Some(3)) => graphemes >= 5,
                (Language::En, Some(1)) => graphemes <= 4,
                (Language::En, Some(2)) => (5..=8).contains(&graphemes),
                (Language::En, Some(3)) => graphemes >= 9,
                _ => false,
            };
            assert!(matches_threshold, "{} has {graphemes} graphemes", item.id);
        }
    }
}

#[test]
fn word_packs_have_exact_project_cc0_provenance_and_unique_catalog_keys() {
    for (file_name, pack_id, source_id) in [
        ("ko-words.toml", "ko-words", "typeul-ko-words-v1.0.0"),
        ("en-words.toml", "en-words", "typeul-en-words-v1.0.0"),
    ] {
        let pack = load_word_pack(file_name);
        assert_eq!(pack.id, pack_id);
        assert_eq!(pack.source.author, "Typeul contributors");
        assert_eq!(pack.source.source_id, source_id);
        assert_eq!(
            pack.source.source_url,
            format!("https://github.com/baba9811/typeul/blob/v1.0.0/assets/content/{file_name}")
        );
        assert_eq!(pack.source.license, "CC0-1.0");
        assert_eq!(
            pack.source.license_url,
            "https://creativecommons.org/publicdomain/zero/1.0/"
        );
        assert!(!pack.source.modified);
        assert_eq!(pack.source.retrieved_at, "2026-08-07");
        assert!(pack.items.iter().all(|item| item.source.is_none()));
    }

    let catalog = ContentCatalog::load_builtins().unwrap();
    let mut ids = HashSet::new();
    let mut texts = HashSet::new();
    for item in catalog.items() {
        assert!(ids.insert(item.id.as_str()), "duplicate ID: {}", item.id);
        assert!(
            texts.insert(item.text.as_str()),
            "duplicate normalized text: {}",
            item.text
        );
    }
}

#[test]
fn bundled_tatoeba_sentence_packs_match_the_reviewed_5a_contract() {
    let catalog = ContentCatalog::load_builtins().unwrap();

    for (language, pack_id) in [
        (Language::Ko, "ko-sentences"),
        (Language::En, "en-sentences"),
    ] {
        let items: Vec<_> = catalog
            .items()
            .filter(|item| item.language == language && item.pack_id == pack_id)
            .collect();
        assert_eq!(items.len(), 120, "{pack_id}");
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == ContentKind::Sentence)
                .count(),
            100,
            "{pack_id}"
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == ContentKind::Quote)
                .count(),
            20,
            "{pack_id}"
        );
    }
}

#[test]
fn tatoeba_sentence_packs_have_exact_release_safe_provenance() {
    let cases = [
        (
            include_str!("../assets/content/ko-sentences.toml"),
            "ko-sentences",
            "tatoeba-kor_detailed-ffc266c1db3855728ee382b30530f854e3bf53760fb7c2bb36ddbd14d94efd96",
            "https://downloads.tatoeba.org/exports/per_language/kor/kor_sentences_detailed.tsv.bz2",
            "CC-BY-2.0-FR",
            "https://creativecommons.org/licenses/by/2.0/fr/",
        ),
        (
            include_str!("../assets/content/en-sentences.toml"),
            "en-sentences",
            "tatoeba-eng_cc0-6ab169264a28008c25bf63042bf7535fc63137c9d7e09b7b8bd7812d10117d1b",
            "https://downloads.tatoeba.org/exports/per_language/eng/eng_sentences_CC0.tsv.bz2",
            "CC0-1.0",
            "https://creativecommons.org/publicdomain/zero/1.0/",
        ),
    ];

    for (source, pack_id, source_id, source_url, license, license_url) in cases {
        let pack = parse_pack(source).unwrap();
        assert!(validate_pack(&pack).is_empty(), "{pack_id}");
        assert_eq!(pack.id, pack_id);
        assert_eq!(pack.source.source_id, source_id);
        assert_eq!(pack.source.source_url, source_url);
        assert_eq!(pack.source.license, license);
        assert_eq!(pack.source.license_url, license_url);
        assert!(!pack.source.modified);
        assert_eq!(pack.source.retrieved_at, "2026-08-07");

        for item in pack.resolve_items().unwrap() {
            let sentence_id = item.id.rsplit('-').next().unwrap();
            assert!(!item.source.author.trim().is_empty(), "{}", item.id);
            assert_eq!(item.source.source_id, format!("tatoeba:{sentence_id}"));
            assert_eq!(
                item.source.source_url,
                format!("https://tatoeba.org/en/sentences/show/{sentence_id}")
            );
            assert_eq!(item.source.license, license);
            assert_eq!(item.source.license_url, license_url);
            assert!(!item.source.modified, "{}", item.id);
            assert_eq!(item.source.retrieved_at, "2026-08-07");
        }
    }
}

#[test]
fn reviewed_replacement_rows_are_verbatim_and_rejected_rows_are_absent() {
    let cases = [
        (
            include_str!("../assets/content/ko-sentences.toml"),
            vec![
                (
                    "ko-tatoeba-13187817",
                    ContentKind::Sentence,
                    "이 책이 저 책보다 더 재미있어요.",
                    "atitarev",
                ),
                (
                    "ko-tatoeba-13148497",
                    ContentKind::Quote,
                    "잘 보이시나요?",
                    "atitarev",
                ),
            ],
            vec!["ko-tatoeba-2655664", "ko-tatoeba-11104766"],
        ),
        (
            include_str!("../assets/content/en-sentences.toml"),
            vec![
                (
                    "en-tatoeba-8895474",
                    ContentKind::Sentence,
                    "The recipe calls for basmati rice.",
                    "Tatoeba CC0 contributors",
                ),
                (
                    "en-tatoeba-9414186",
                    ContentKind::Sentence,
                    "Someone left their phone on the table.",
                    "Tatoeba CC0 contributors",
                ),
                (
                    "en-tatoeba-9805138",
                    ContentKind::Sentence,
                    "How do I apply a bandage?",
                    "Tatoeba CC0 contributors",
                ),
                (
                    "en-tatoeba-11476060",
                    ContentKind::Sentence,
                    "The sun and clouds create a shifting color palette on the badlands landscape.",
                    "Tatoeba CC0 contributors",
                ),
            ],
            vec![
                "en-tatoeba-7742006",
                "en-tatoeba-8289410",
                "en-tatoeba-8297994",
                "en-tatoeba-7880691",
            ],
        ),
    ];

    for (source, replacements, rejected_ids) in cases {
        let items = parse_pack(source).unwrap().resolve_items().unwrap();
        for (id, kind, text, author) in replacements {
            let item = items.iter().find(|item| item.id == id).unwrap();
            assert_eq!(item.kind, kind, "{id}");
            assert_eq!(item.text, text, "{id}");
            assert_eq!(item.source.author, author, "{id}");
            assert_eq!(
                item.source.source_id,
                format!("tatoeba:{}", id.rsplit('-').next().unwrap())
            );
            assert!(!item.source.modified, "{id}");
        }
        for id in rejected_ids {
            assert!(items.iter().all(|item| item.id != id), "{id}");
        }
    }
}

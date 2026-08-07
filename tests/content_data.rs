use std::{collections::HashSet, fs, path::Path};
use typeul::{
    content::{ContentCatalog, ContentKind, ContentPack, parse_pack, validate_pack},
    model::Language,
};
use unicode_segmentation::UnicodeSegmentation;

fn load_pack(file_name: &str) -> ContentPack {
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

        let pack = load_pack(file_name);
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
        let pack = load_pack(file_name);
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
fn bundled_text_packs_match_the_reviewed_5b2_contract() {
    let catalog = ContentCatalog::load_builtins().unwrap();

    for (language, pack_id, file_name, expected) in [
        (
            Language::Ko,
            "ko-texts",
            "ko-texts.toml",
            [
                (
                    "ko-text-essay-room-to-revise",
                    "고칠 자리를 남기는 일",
                    "essay",
                ),
                ("ko-text-essay-window-routine", "창문을 닦는 순서", "essay"),
                (
                    "ko-text-fiction-blue-umbrella",
                    "파란 우산의 행선지",
                    "fiction",
                ),
                ("ko-text-fiction-last-seed", "마지막 씨앗 봉투", "fiction"),
                (
                    "ko-text-aphorism-daily-bearings",
                    "생활의 작은 방위표",
                    "aphorism",
                ),
                (
                    "ko-text-retelling-heungbu-nolbu",
                    "박씨 하나의 몫 — 흥부와 놀부 다시 쓰기",
                    "retelling",
                ),
                (
                    "ko-text-retelling-sun-moon-siblings",
                    "하늘에 남은 두 빛 — 해와 달이 된 오누이 다시 쓰기",
                    "retelling",
                ),
                (
                    "ko-text-constitution-articles-1-5",
                    "대한민국헌법 제1조부터 제5조",
                    "public-domain",
                ),
            ],
        ),
        (
            Language::En,
            "en-texts",
            "en-texts.toml",
            [
                (
                    "en-text-essay-useful-pause",
                    "The Use of a Useful Pause",
                    "essay",
                ),
                (
                    "en-text-essay-mending-small-things",
                    "The Habit of Mending Small Things",
                    "essay",
                ),
                (
                    "en-text-fiction-upper-window",
                    "The Light in the Upper Window",
                    "fiction",
                ),
                (
                    "en-text-fiction-paper-bridge",
                    "The Paper Bridge",
                    "fiction",
                ),
                (
                    "en-text-aphorism-steady-compass",
                    "A Steady Compass",
                    "aphorism",
                ),
                (
                    "en-text-retelling-tortoise-hare",
                    "Two Ways to the Finish — The Tortoise and the Hare Retold",
                    "retelling",
                ),
                (
                    "en-text-retelling-stone-soup",
                    "The Empty Pot — Stone Soup Retold",
                    "retelling",
                ),
                (
                    "en-text-constitution-article-1-section-2-clauses-1-2",
                    "U.S. Constitution, Article I, Section 2, Clauses 1–2",
                    "public-domain",
                ),
            ],
        ),
    ] {
        let items: Vec<_> = catalog
            .items()
            .filter(|item| item.language == language && item.pack_id == pack_id)
            .collect();
        assert_eq!(items.len(), 8, "{pack_id}");
        assert!(items.iter().all(|item| item.kind == ContentKind::Text));

        let pack = load_pack(file_name);
        assert!(validate_pack(&pack).is_empty(), "{pack_id}");
        assert_eq!(pack.id, pack_id);
        assert_eq!(pack.language, language);
        assert_eq!(pack.items.len(), 8, "{pack_id}");

        for (id, title, tag) in expected {
            let item = items.iter().find(|item| item.id == id).unwrap();
            assert_eq!(item.title.as_deref(), Some(title), "{id}");
            assert!(item.difficulty.is_some(), "{id}");
            assert_eq!(item.tags, [tag], "{id}");
            assert!(item.text.contains("\n\n"), "{id} is not multi-paragraph");
            assert!(
                item.text
                    .split("\n\n")
                    .all(|paragraph| !paragraph.trim().is_empty()),
                "{id}"
            );
            let graphemes = item.text.graphemes(true).count();
            assert!(graphemes >= 200, "{id} has only {graphemes} graphemes");
        }

        for (tag, count) in [
            ("essay", 2),
            ("fiction", 2),
            ("aphorism", 1),
            ("retelling", 2),
            ("public-domain", 1),
        ] {
            assert_eq!(
                items.iter().filter(|item| item.tags == [tag]).count(),
                count,
                "{pack_id} {tag}"
            );
        }
    }
}

#[test]
fn text_packs_have_exact_item_level_provenance() {
    let cases = [
        (
            "ko-texts.toml",
            "ko-texts",
            "ko-text-constitution-articles-1-5",
            "대한민국",
            "rok-constitution:articles-1-5",
            "https://www.law.go.kr/법령/대한민국헌법",
            "https://www.law.go.kr/법령/저작권법/제7조",
            "제1조 ①대한민국은 민주공화국이다.\n\n②대한민국의 주권은 국민에게 있고, 모든 권력은 국민으로부터 나온다.\n\n제2조 ①대한민국의 국민이 되는 요건은 법률로 정한다.\n\n②국가는 법률이 정하는 바에 의하여 재외국민을 보호할 의무를 진다.\n\n제3조 대한민국의 영토는 한반도와 그 부속도서로 한다.\n\n제4조 대한민국은 통일을 지향하며, 자유민주적 기본질서에 입각한 평화적 통일정책을 수립하고 이를 추진한다.\n\n제5조 ①대한민국은 국제평화의 유지에 노력하고 침략적 전쟁을 부인한다.\n\n②국군은 국가의 안전보장과 국토방위의 신성한 의무를 수행함을 사명으로 하며, 그 정치적 중립성은 준수된다.",
        ),
        (
            "en-texts.toml",
            "en-texts",
            "en-text-constitution-article-1-section-2-clauses-1-2",
            "Constitutional Convention of 1787",
            "us-constitution:article-1-section-2-clauses-1-2",
            "https://constitution.congress.gov/constitution/article-1/",
            "https://copyright.gov/what-is-copyright/",
            "The House of Representatives shall be composed of Members chosen every second Year by the People of the several States, and the Electors in each State shall have the Qualifications requisite for Electors of the most numerous Branch of the State Legislature.\n\nNo Person shall be a Representative who shall not have attained to the Age of twenty five Years, and been seven Years a Citizen of the United States, and who shall not, when elected, be an Inhabitant of that State in which he shall be chosen.",
        ),
    ];

    for (
        file_name,
        pack_id,
        constitution_id,
        constitution_author,
        constitution_source_id,
        constitution_source_url,
        constitution_license_url,
        constitution_text,
    ) in cases
    {
        let pack = load_pack(file_name);
        let repository_url =
            format!("https://github.com/baba9811/typeul/blob/v1.0.0/assets/content/{file_name}");
        assert_eq!(pack.source.author, "Typeul contributors");
        assert_eq!(pack.source.source_id, format!("typeul-{pack_id}-v1.0.0"));
        assert_eq!(pack.source.source_url, repository_url);
        assert_eq!(pack.source.license, "CC0-1.0");
        assert_eq!(
            pack.source.license_url,
            "https://creativecommons.org/publicdomain/zero/1.0/"
        );
        assert!(!pack.source.modified);
        assert_eq!(pack.source.retrieved_at, "2026-08-07");
        assert!(pack.items.iter().all(|item| item.source.is_some()));

        for item in pack.resolve_items().unwrap() {
            assert!(!item.source.modified, "{}", item.id);
            assert_eq!(item.source.retrieved_at, "2026-08-07", "{}", item.id);
            if item.id == constitution_id {
                assert_eq!(item.source.author, constitution_author);
                assert_eq!(item.source.source_id, constitution_source_id);
                assert_eq!(item.source.source_url, constitution_source_url);
                assert_eq!(item.source.license, "LicenseRef-Public-Domain");
                assert_eq!(item.source.license_url, constitution_license_url);
                assert_eq!(item.text, constitution_text);
            } else {
                assert_eq!(item.source.author, "Typeul contributors", "{}", item.id);
                assert_eq!(
                    item.source.source_id,
                    format!("typeul:{}:v1.0.0", item.id),
                    "{}",
                    item.id
                );
                assert_eq!(item.source.source_url, repository_url, "{}", item.id);
                assert_eq!(item.source.license, "CC0-1.0", "{}", item.id);
                assert_eq!(
                    item.source.license_url, "https://creativecommons.org/publicdomain/zero/1.0/",
                    "{}",
                    item.id
                );
            }
        }
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

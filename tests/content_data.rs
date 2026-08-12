use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    path::Path,
    process::Command,
};
use typerlude::{
    content::{ContentCatalog, ContentKind, ContentPack, parse_pack, validate_pack},
    model::Language,
};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

fn load_pack(file_name: &str) -> ContentPack {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/content")
        .join(file_name);
    parse_pack(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn every_bundled_practice_text_uses_direct_keyboard_characters() {
    for item in ContentCatalog::load_builtins().unwrap().items() {
        for character in item.text.chars() {
            let allowed = character == '\n'
                || character == ' '
                || character.is_ascii_graphic()
                || (item.language == Language::Ko && ('가'..='힣').contains(&character));
            assert!(
                allowed,
                "{} contains non-keyboard character U+{:04X} {character}",
                item.id, character as u32
            );
        }
    }
}

#[test]
fn tracked_text_has_no_stale_product_identifier() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "-z"])
        .output()
        .expect("git must be executable");
    assert!(
        output.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let old_brand = ["Type", "ul"].concat();
    let old_name = old_brand.to_ascii_lowercase();
    let old_uppercase_name = old_brand.to_ascii_uppercase();
    let old_repository = ["baba9811/", old_name.as_str()].concat();
    let old_korean_brand = ["타이", "플"].concat();
    for path in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let path = std::str::from_utf8(path).expect("tracked paths must be UTF-8");
        let contents = fs::read(root.join(path)).unwrap();
        if contents.contains(&0) {
            continue;
        }
        let contents = String::from_utf8(contents).expect("tracked text must be UTF-8");
        assert!(
            !contents.contains(&old_name)
                && !contents.contains(&old_brand)
                && !contents.contains(&old_uppercase_name)
                && !contents.contains(&old_repository)
                && !contents.contains(&old_korean_brand),
            "stale product identifier in {path}"
        );
    }
}

#[test]
fn cargo_package_includes_both_readmes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO"))
        .current_dir(root)
        .args([
            "package",
            "--list",
            "--allow-dirty",
            "--locked",
            "--offline",
        ])
        .output()
        .expect("cargo package must be executable");
    assert!(
        output.status.success(),
        "cargo package --list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let files = String::from_utf8(output.stdout).expect("package list must be UTF-8");
    let readmes = files
        .lines()
        .filter(|path| path.starts_with("README") && path.ends_with(".md"))
        .collect::<Vec<_>>();
    assert_eq!(readmes, ["README.ko.md", "README.md"]);
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
        ("ko-words.toml", "ko-words", "typerlude-ko-words-v1.0.0"),
        ("en-words.toml", "en-words", "typerlude-en-words-v1.0.0"),
    ] {
        let pack = load_pack(file_name);
        assert_eq!(pack.id, pack_id);
        assert_eq!(pack.source.author, "Typerlude contributors");
        assert_eq!(pack.source.source_id, source_id);
        assert_eq!(
            pack.source.source_url,
            format!("https://github.com/baba9811/typerlude/blob/v1.0.0/assets/content/{file_name}")
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
            "제1조 (1) 대한민국은 민주공화국이다.\n\n(2) 대한민국의 주권은 국민에게 있고, 모든 권력은 국민으로부터 나온다.\n\n제2조 (1) 대한민국의 국민이 되는 요건은 법률로 정한다.\n\n(2) 국가는 법률이 정하는 바에 의하여 재외국민을 보호할 의무를 진다.\n\n제3조 대한민국의 영토는 한반도와 그 부속도서로 한다.\n\n제4조 대한민국은 통일을 지향하며, 자유민주적 기본질서에 입각한 평화적 통일정책을 수립하고 이를 추진한다.\n\n제5조 (1) 대한민국은 국제평화의 유지에 노력하고 침략적 전쟁을 부인한다.\n\n(2) 국군은 국가의 안전보장과 국토방위의 신성한 의무를 수행함을 사명으로 하며, 그 정치적 중립성은 준수된다.",
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
            format!("https://github.com/baba9811/typerlude/blob/v1.0.0/assets/content/{file_name}");
        assert_eq!(pack.source.author, "Typerlude contributors");
        assert_eq!(pack.source.source_id, format!("typerlude-{pack_id}-v1.0.0"));
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
            assert_eq!(
                item.source.modified,
                pack_id == "ko-texts" && item.id == constitution_id,
                "{}",
                item.id
            );
            assert_eq!(item.source.retrieved_at, "2026-08-07", "{}", item.id);
            if item.id == constitution_id {
                assert_eq!(item.source.author, constitution_author);
                assert_eq!(item.source.source_id, constitution_source_id);
                assert_eq!(item.source.source_url, constitution_source_url);
                assert_eq!(item.source.license, "LicenseRef-Public-Domain");
                assert_eq!(item.source.license_url, constitution_license_url);
                assert_eq!(item.text, constitution_text);
            } else {
                assert_eq!(item.source.author, "Typerlude contributors", "{}", item.id);
                assert_eq!(
                    item.source.source_id,
                    format!("typerlude:{}:v1.0.0", item.id),
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

#[derive(Deserialize)]
struct FrozenSelection {
    ko: Vec<FrozenSentence>,
    en: Vec<FrozenSentence>,
}

#[derive(Deserialize)]
struct FrozenSentence {
    id: u64,
    author: String,
}

#[derive(Deserialize)]
struct FrozenSnapshot {
    retrieved_at: String,
    exports: Vec<FrozenExport>,
}

#[derive(Deserialize)]
struct FrozenExport {
    key: String,
    url: String,
    bytes: u64,
    sha256: String,
    decompressed_bytes: u64,
    decompressed_sha256: String,
}

fn all_packs() -> Vec<ContentPack> {
    let content_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/content");
    let mut paths = fs::read_dir(content_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| parse_pack(&fs::read_to_string(path).unwrap()).unwrap())
        .collect()
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[test]
fn complete_release_catalog_has_exact_counts_unique_nfc_content_and_no_warnings() {
    let absent_user_dir = std::env::temp_dir().join(format!(
        "typerlude-no-user-content-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let loaded = ContentCatalog::load(&absent_user_dir).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);

    for language in [Language::Ko, Language::En] {
        assert_eq!(loaded.catalog.count(language, ContentKind::Word), 300);
        assert_eq!(
            loaded
                .catalog
                .count_any(language, &[ContentKind::Sentence, ContentKind::Quote]),
            120
        );
        assert_eq!(loaded.catalog.count(language, ContentKind::Text), 8);
    }

    let mut ids = HashSet::new();
    let mut texts = HashSet::new();
    for item in loaded.catalog.items() {
        assert!(ids.insert(&item.id), "duplicate ID: {}", item.id);
        let normalized = item.text.nfc().collect::<String>();
        assert_eq!(item.text, normalized, "non-NFC text: {}", item.id);
        assert!(texts.insert(normalized), "duplicate NFC text: {}", item.id);
    }
}

#[test]
fn effective_release_licenses_have_exact_offline_text_or_public_domain_notice() {
    let used = all_packs()
        .into_iter()
        .flat_map(|pack| pack.resolve_items().unwrap())
        .map(|item| item.source.license)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        used,
        BTreeSet::from([
            "CC-BY-2.0-FR".to_owned(),
            "CC0-1.0".to_owned(),
            "LicenseRef-Public-Domain".to_owned(),
        ])
    );

    let license_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/licenses");
    let shipped = fs::read_dir(&license_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        shipped,
        BTreeSet::from([
            "CC-BY-2.0-FR.txt".to_owned(),
            "CC0-1.0.txt".to_owned(),
            "NORD-MIT.txt".to_owned(),
        ])
    );

    let cc0_path = license_dir.join("CC0-1.0.txt");
    let cc0 = fs::read_to_string(&cc0_path).unwrap();
    assert_eq!(cc0.len(), 7_048);
    assert_eq!(fnv1a(cc0.as_bytes()), 0xf92ec4039367c961);
    assert!(cc0.contains("CC0 1.0 Universal"));
    assert!(cc0.contains("1. Copyright and Related Rights."));
    assert!(cc0.contains("4. Limitations and Disclaimers."));

    let cc_by_fr_path = license_dir.join("CC-BY-2.0-FR.txt");
    let cc_by_fr = fs::read_to_string(&cc_by_fr_path).unwrap();
    assert_eq!(cc_by_fr.len(), 15_978);
    assert_eq!(fnv1a(cc_by_fr.as_bytes()), 0xc38a2e100edabc1a);
    for marker in [
        "Paternité - 2.0",
        "1. Définitions",
        "3. Autorisation",
        "4. Restrictions",
        "7. Résiliation",
        "8. Divers",
    ] {
        assert!(cc_by_fr.contains(marker), "missing {marker}");
    }

    let notice =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("THIRD_PARTY_NOTICES.md"))
            .unwrap();
    assert!(notice.contains("LicenseRef-Public-Domain"));
    assert!(notice.contains("assets/licenses/CC0-1.0.txt"));
    assert!(notice.contains("assets/licenses/CC-BY-2.0-FR.txt"));
    for digest in [
        "a2010f343487d3f7618affe54f789f5487602331c0a8d03f49e9a7c547cf0499",
        "af0d7ada8b9be52a6874238f4533512d0b2568595bf7cb3427e41f7c38847b71",
        "94690c30fa9b7650a55ea91f9158e3dab81a5e3a79ec1e07d9b0be8a5212b81a",
    ] {
        assert!(notice.contains(digest));
    }
}

fn validate_dependency_license_html(
    html: &str,
    expected_crates: &BTreeSet<(String, String)>,
) -> Result<(), String> {
    if html.contains('\r') || html.lines().any(|line| line.ends_with([' ', '\t'])) {
        return Err("license HTML must use LF without trailing horizontal whitespace".to_owned());
    }

    let reported_crates = html
        .lines()
        .filter_map(|line| line.trim().strip_prefix("<dt>")?.strip_suffix("</dt>"))
        .map(|row| {
            row.split_once(' ')
                .map(|(name, version)| (name.to_owned(), version.to_owned()))
                .ok_or_else(|| format!("malformed crate row <dt>{row}</dt>"))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if reported_crates != *expected_crates {
        let missing = expected_crates
            .difference(&reported_crates)
            .collect::<Vec<_>>();
        let extra = reported_crates
            .difference(expected_crates)
            .collect::<Vec<_>>();
        return Err(format!(
            "crate rows differ; missing: {missing:?}; extra: {extra:?}"
        ));
    }

    let mut sections = 0;
    for (index, after_start) in html.split("<section>").skip(1).enumerate() {
        sections += 1;
        after_start
            .split_once("</section>")
            .and_then(|(section, _)| section.split_once("<pre>"))
            .and_then(|(_, after_pre)| after_pre.split_once("</pre>"))
            .map(|(text, _)| text.trim())
            .filter(|text| !text.is_empty())
            .ok_or_else(|| format!("license section {} has no complete text body", index + 1))?;
    }
    if sections == 0 {
        return Err("no license sections found".to_owned());
    }

    Ok(())
}

#[test]
fn third_party_rust_license_report_covers_locked_supported_target_graph() {
    let sample_crates = BTreeSet::from([("anyhow".to_owned(), "1.0.104".to_owned())]);
    for invalid_html in [
        "",
        "<dt>anyhow 1.0.104</dt>\n<dt>stale 1.0.0</dt>\n<section><pre>MIT</pre></section>",
        "<dt>anyhow 1.0.104</dt><section><pre> </pre></section>",
        "<dt>anyhow 1.0.104</dt>\r\n<section><pre>MIT</pre></section>",
        "<dt>anyhow 1.0.104</dt> \n<section><pre>MIT</pre></section>",
    ] {
        assert!(
            validate_dependency_license_html(invalid_html, &sample_crates).is_err(),
            "validator accepted invalid HTML: {invalid_html:?}"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut expected_crates = BTreeSet::new();
    for target in [
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "aarch64-unknown-linux-gnu",
        "x86_64-unknown-linux-gnu",
        "aarch64-pc-windows-msvc",
        "x86_64-pc-windows-msvc",
    ] {
        let output = Command::new(env!("CARGO"))
            .current_dir(root)
            .args([
                "tree",
                "--locked",
                "--all-features",
                &format!("--target={target}"),
                "--edges=normal,build",
                "--prefix=none",
                "--format={p}",
            ])
            .output()
            .expect("cargo tree must be executable");
        assert!(
            output.status.success(),
            "cargo tree failed for {target}: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let tree = String::from_utf8(output.stdout).expect("cargo tree output must be UTF-8");
        for line in tree.lines().filter(|line| !line.trim().is_empty()) {
            let (name, rest) = line
                .split_once(" v")
                .unwrap_or_else(|| panic!("malformed cargo tree row for {target}: {line}"));
            let version = rest.split_whitespace().next().unwrap();
            if name != env!("CARGO_PKG_NAME") || version != env!("CARGO_PKG_VERSION") {
                expected_crates.insert((name.to_owned(), version.to_owned()));
            }
        }
    }

    let html = fs::read_to_string(root.join("THIRD_PARTY_LICENSES.html"))
        .expect("THIRD_PARTY_LICENSES.html must be readable UTF-8");
    assert!(html.contains("<title>Typerlude third-party Rust licenses</title>"));
    assert!(html.contains("<h1>Typerlude third-party Rust licenses</h1>"));
    validate_dependency_license_html(&html, &expected_crates)
        .unwrap_or_else(|error| panic!("invalid THIRD_PARTY_LICENSES.html: {error}"));
}

#[test]
fn notice_matches_every_frozen_tatoeba_and_public_domain_provenance_fact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let notice = fs::read_to_string(root.join("THIRD_PARTY_NOTICES.md")).unwrap();
    let collapsed_notice = collapse_whitespace(&notice);
    let selection: FrozenSelection = serde_json::from_str(
        &fs::read_to_string(root.join("assets/sources/tatoeba-selection.json")).unwrap(),
    )
    .unwrap();
    let snapshot: FrozenSnapshot = serde_json::from_str(
        &fs::read_to_string(root.join("assets/sources/tatoeba-snapshot.json")).unwrap(),
    )
    .unwrap();

    for required in [
        "Rust and JavaScript software is licensed under the MIT License",
        "Practice data written by Typerlude contributors is released under CC0 1.0 Universal",
        "Third-party material keeps the license or public-domain status stated below",
    ] {
        assert!(collapsed_notice.contains(required));
    }

    let mut korean_by_author = BTreeMap::<String, Vec<u64>>::new();
    for sentence in &selection.ko {
        korean_by_author
            .entry(sentence.author.clone())
            .or_default()
            .push(sentence.id);
    }
    for (author, ids) in korean_by_author {
        let expected = format!(
            "{author} ({}): {}",
            ids.len(),
            ids.iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert!(
            collapsed_notice.contains(&expected),
            "missing grouped Korean attribution: {author}"
        );
    }

    let english_authors = selection
        .en
        .iter()
        .map(|sentence| sentence.author.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        english_authors,
        BTreeSet::from(["Tatoeba CC0 contributors"])
    );
    let expected_english = format!(
        "Tatoeba CC0 contributors ({}): {}",
        selection.en.len(),
        selection
            .en
            .iter()
            .map(|sentence| sentence.id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert!(collapsed_notice.contains(&expected_english));

    assert!(
        notice.contains(&format!("Retrieved: `{}`", snapshot.retrieved_at)),
        "missing frozen retrieval date"
    );
    for export in snapshot.exports {
        let expected = format!(
            "| `{}` | {} | {} | `{}` | {} | `{}` |",
            export.key,
            export.url,
            export.bytes,
            export.sha256,
            export.decompressed_bytes,
            export.decompressed_sha256
        );
        assert!(
            notice.contains(&expected),
            "missing snapshot row: {}",
            export.key
        );
    }

    for pack in [
        load_pack("ko-sentences.toml"),
        load_pack("en-sentences.toml"),
    ] {
        assert!(notice.contains(&pack.source.license));
        assert!(notice.contains(&pack.source.license_url));
    }
    for item in all_packs()
        .into_iter()
        .flat_map(|pack| pack.resolve_items().unwrap())
        .filter(|item| item.source.license == "LicenseRef-Public-Domain")
    {
        assert!(notice.contains(item.title.as_deref().unwrap()));
        assert!(notice.contains(&item.source.source_id));
        assert!(notice.contains(&item.source.source_url));
        assert!(notice.contains(&item.source.license_url));
        assert!(notice.contains(&item.source.retrieved_at));
    }
    for item in all_packs()
        .into_iter()
        .flat_map(|pack| pack.resolve_items().unwrap())
        .filter(|item| item.tags.iter().any(|tag| tag == "retelling"))
    {
        assert_eq!(item.source.author, "Typerlude contributors");
        assert_eq!(item.source.license, "CC0-1.0");
        assert!(notice.contains(&item.id));
        assert!(collapsed_notice.contains(&collapse_whitespace(item.title.as_deref().unwrap())));
    }
    for required in [
        "https://www.law.go.kr/lawPetitionForm.do?menuId=13&subMenuId=79",
        "https://www.archives.gov/founding-docs/constitution-transcript",
        "https://www.archives.gov/founding-docs/downloads",
        "https://tatoeba.org/",
        "https://tatoeba.org/en/sentences/show/<sentence-id>",
        "source_id = tatoeba:<sentence-id>",
        "assets/sources/tatoeba-selection.json",
        "assets/sources/tatoeba-snapshot.json",
        "original rows (`base = 0`)",
        "selection, packaging, and metadata",
        "not normalized or otherwise modified (`modified = false`)",
        "not 17 U.S.C. § 105",
        "federal-government material",
        "circled paragraph numerals `①` and",
        "therefore declares `modified = true`",
        "흥부와 놀부",
        "해와 달이 된 오누이",
        "The Tortoise and the Hare",
        "Stone Soup",
    ] {
        assert!(notice.contains(required), "missing notice fact: {required}");
    }
}

#[test]
fn bundled_content_and_license_text_are_forced_to_lf_by_git_attributes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let attributes = fs::read_to_string(root.join(".gitattributes")).unwrap();

    for required in [
        "assets/content/*.toml text eol=lf",
        "assets/licenses/*.txt text eol=lf",
        "LICENSE text eol=lf",
        "THIRD_PARTY_LICENSES.html text eol=lf",
        "THIRD_PARTY_NOTICES.md text eol=lf",
        "npm/*/package.json text eol=lf",
    ] {
        assert!(
            attributes.lines().any(|line| line == required),
            "missing Git attribute: {required}"
        );
    }
}

#[test]
fn repository_tracks_only_frozen_metadata_not_raw_exports_or_work_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    if root.join(".git").exists() {
        let output = Command::new("git")
            .args(["ls-files", "-z"])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success());
        let tracked = String::from_utf8(output.stdout).unwrap();
        for path in tracked.split('\0').filter(|path| !path.is_empty()) {
            let lower = path.to_ascii_lowercase();
            assert!(
                ![
                    ".tsv", ".bz2", ".zip", ".tar", ".tar.gz", ".tgz", ".gz", ".xz", ".7z", ".tmp",
                    ".temp", ".bak", ".swp", ".orig", ".rej", "~",
                ]
                .iter()
                .any(|suffix| lower.ends_with(suffix)),
                "tracked raw/archive/temporary file: {path}"
            );
            assert!(
                !lower.contains("proprietary") && !lower.contains("source-list"),
                "tracked proprietary source list: {path}"
            );
        }
    }

    let source_files = fs::read_dir(root.join("assets/sources"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        source_files,
        BTreeSet::from([
            "tatoeba-selection.json".to_owned(),
            "tatoeba-snapshot.json".to_owned(),
        ])
    );
    assert!(!root.join("assets/licenses/KOGL-1.0.txt").exists());
}

use super::{
    Range, adaptive_candidates, has_key_attempts, history, intended_key_counts, progress, streak,
    summarize, weak_keys,
};
use crate::{
    content::{ContentCatalog, ContentKind},
    model::{Language, PracticeKind},
    storage::SessionRecord,
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use time::{Date, macros::date};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

fn session(id: &str, local_date: Date, language: Language) -> SessionRecord {
    SessionRecord {
        schema_version: 1,
        id: id.into(),
        started_at_unix_ms: 0,
        local_date,
        language,
        mode: PracticeKind::Words,
        content_id: "fixture".into(),
        difficulty: Some(1),
        duration_ms: 60_000,
        correct_units: 9,
        attempted_units: 10,
        errors: 1,
        backspaces: 0,
        cpm: 0.0,
        kpm: 0.0,
        wpm: 0.0,
        accuracy: 90.0,
        intended_keys: BTreeMap::new(),
    }
}

#[test]
fn overview_summarizes_both_speed_units() {
    let mut ko_a = session("ko-a", date!(2026 - 08 - 07), Language::Ko);
    ko_a.kpm = 400.0;
    ko_a.wpm = 40.0;
    let mut ko_b = session("ko-b", date!(2026 - 08 - 07), Language::Ko);
    ko_b.kpm = 500.0;
    ko_b.wpm = 50.0;

    let overview = summarize([&ko_a, &ko_b]);

    assert_eq!(overview.sessions, 2);
    assert_eq!(
        overview.kpm,
        super::SpeedSummary {
            average: 450.0,
            best: 500.0,
        }
    );
    assert_eq!(
        overview.wpm,
        super::SpeedSummary {
            average: 45.0,
            best: 50.0,
        }
    );
}

#[test]
fn overview_averages_extreme_finite_speeds_without_overflow() {
    let mut first = session("first", date!(2026 - 08 - 07), Language::En);
    first.kpm = f64::MAX;
    first.wpm = f64::MAX;
    let mut second = session("second", date!(2026 - 08 - 07), Language::En);
    second.kpm = f64::MAX;
    second.wpm = f64::MAX;

    let overview = summarize([&first, &second]);

    assert_eq!(overview.kpm.average, f64::MAX);
    assert_eq!(overview.wpm.average, f64::MAX);
    assert_eq!(overview.kpm.best, f64::MAX);
    assert_eq!(overview.wpm.best, f64::MAX);
}

#[test]
fn overview_weights_stored_accuracy_by_attempts_not_final_correct_units() {
    let mut small = session("small", date!(2026 - 08 - 07), Language::En);
    small.attempted_units = 1;
    small.correct_units = 1;
    small.accuracy = 0.0;
    let mut large = session("large", date!(2026 - 08 - 07), Language::En);
    large.attempted_units = 3;
    large.correct_units = 0;
    large.accuracy = 100.0;

    let overview = summarize([&small, &large]);

    assert_eq!(overview.accuracy, 75.0);
}

#[test]
fn empty_overview_is_finite_zero_and_duration_sum_saturates() {
    let empty = summarize(std::iter::empty::<&SessionRecord>());
    assert_eq!(empty.total, Duration::ZERO);
    assert_eq!(empty.sessions, 0);
    assert_eq!(empty.accuracy, 0.0);
    assert_eq!(empty.kpm, super::SpeedSummary::default());
    assert_eq!(empty.wpm, super::SpeedSummary::default());
    assert!(empty.accuracy.is_finite());

    let mut first = session("first", date!(2026 - 08 - 07), Language::En);
    first.duration_ms = u64::MAX;
    let mut second = session("second", date!(2026 - 08 - 07), Language::En);
    second.duration_ms = 1;
    assert_eq!(
        summarize([&first, &second]).total,
        Duration::from_millis(u64::MAX)
    );
}

#[test]
fn finite_ranges_include_exact_calendar_windows_and_exclude_future_dates() {
    let today = date!(2026 - 08 - 07);
    assert!(Range::Days7.includes(date!(2026 - 08 - 01), today));
    assert!(!Range::Days7.includes(date!(2026 - 07 - 31), today));
    assert!(!Range::Days7.includes(date!(2026 - 08 - 08), today));

    assert!(Range::Days30.includes(date!(2026 - 07 - 09), today));
    assert!(!Range::Days30.includes(date!(2026 - 07 - 08), today));
    assert!(!Range::Days30.includes(date!(2026 - 08 - 08), today));

    assert!(Range::Days90.includes(date!(2026 - 05 - 10), today));
    assert!(!Range::Days90.includes(date!(2026 - 05 - 09), today));
    assert!(!Range::Days90.includes(date!(2026 - 08 - 08), today));

    assert!(Range::All.includes(date!(1900 - 01 - 01), today));
    assert!(Range::All.includes(date!(2026 - 08 - 08), today));
}

#[test]
fn history_filters_without_mutation_and_sorts_timestamp_then_id_descending() {
    let today = date!(2026 - 08 - 07);
    let mut a = session("a", today, Language::En);
    a.started_at_unix_ms = 100;
    let mut b = session("b", today, Language::En);
    b.started_at_unix_ms = 100;
    let mut other_language = session("z-ko", today, Language::Ko);
    other_language.started_at_unix_ms = 300;
    let mut other_mode = session("z-test", today, Language::En);
    other_mode.started_at_unix_ms = 400;
    other_mode.mode = PracticeKind::Test;
    let mut too_old = session("z-old", date!(2026 - 07 - 31), Language::En);
    too_old.started_at_unix_ms = 500;
    let sessions = vec![a, b, other_language, other_mode, too_old];

    let selected = history(
        &sessions,
        Range::Days7,
        today,
        Some(Language::En),
        Some(PracticeKind::Words),
    );

    assert_eq!(
        selected
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        ["b", "a"]
    );
    assert_eq!(sessions[0].id, "a");
}

#[test]
fn progress_is_chronological_and_isolates_language_range_and_mode() {
    let today = date!(2026 - 08 - 07);
    let yesterday = date!(2026 - 08 - 06);
    let mut older_word = session("older-word", yesterday, Language::Ko);
    older_word.kpm = 300.0;
    older_word.wpm = 30.0;
    older_word.attempted_units = 1;
    older_word.accuracy = 0.0;
    older_word.duration_ms = 60_000;
    let mut older_test = session("older-test", yesterday, Language::Ko);
    older_test.mode = PracticeKind::Test;
    older_test.kpm = 500.0;
    older_test.wpm = 50.0;
    older_test.attempted_units = 3;
    older_test.accuracy = 100.0;
    older_test.duration_ms = 30_000;
    let mut english = session("english", yesterday, Language::En);
    english.wpm = 900.0;
    english.duration_ms = 600_000;
    let mut current = session("current", today, Language::Ko);
    current.kpm = 500.0;
    current.wpm = 50.0;
    current.attempted_units = 2;
    current.accuracy = 50.0;
    current.duration_ms = 120_000;
    let sessions = [older_word, older_test, english, current];

    let points = progress(&sessions, Range::Days7, today, Language::Ko, None);
    assert_eq!(points.len(), 2);
    assert_eq!(points[0].date, yesterday);
    assert_eq!(points[0].kpm, 400.0);
    assert_eq!(points[0].wpm, 40.0);
    assert_eq!(points[0].accuracy, 75.0);
    assert_eq!(points[0].minutes, 1.5);
    assert_eq!(points[1].date, today);
    assert_eq!(points[1].kpm, 500.0);
    assert_eq!(points[1].wpm, 50.0);
    assert_eq!(points[1].accuracy, 50.0);
    assert_eq!(points[1].minutes, 2.0);

    let words = progress(
        &sessions,
        Range::Days7,
        today,
        Language::Ko,
        Some(PracticeKind::Words),
    );
    assert_eq!(words[0].kpm, 300.0);
    assert_eq!(words[0].wpm, 30.0);
    assert_eq!(words[0].accuracy, 0.0);
    assert_eq!(words[0].minutes, 1.0);
}

#[test]
fn progress_averages_extreme_finite_speeds_without_overflow() {
    let today = date!(2026 - 08 - 07);
    let mut first = session("first", today, Language::En);
    first.kpm = f64::MAX;
    first.wpm = f64::MAX;
    let mut second = session("second", today, Language::En);
    second.kpm = f64::MAX;
    second.wpm = f64::MAX;

    let points = progress(&[first, second], Range::Days7, today, Language::En, None);

    assert_eq!(points[0].kpm, f64::MAX);
    assert_eq!(points[0].wpm, f64::MAX);
}

#[test]
fn streak_deduplicates_today_and_yesterday_runs_and_rejects_old_gaps() {
    let today = date!(2026 - 08 - 07);
    assert_eq!(
        streak(
            [today, today, date!(2026 - 08 - 06), date!(2026 - 08 - 05),],
            today,
        ),
        3
    );
    assert_eq!(
        streak([date!(2026 - 08 - 06), date!(2026 - 08 - 05)], today),
        2
    );
    assert_eq!(streak([date!(2026 - 08 - 05)], today), 0);
    assert_eq!(
        streak([date!(2026 - 08 - 08), date!(2026 - 08 - 07)], today),
        1
    );
}

#[test]
fn weak_keys_require_attempts_and_sort_accuracy_then_key() {
    let counts = BTreeMap::from([
        ('p', [10, 0]),
        ('q', [1, 1]),
        ('x', [8, 2]),
        ('y', [4, 1]),
        ('z', [2, 8]),
    ]);

    let weak = weak_keys(&counts, 5);

    assert!(has_key_attempts(&counts, 5));
    assert_eq!(
        weak.iter().map(|value| value.key).collect::<Vec<_>>(),
        ['z', 'x', 'y']
    );
    assert_eq!(
        (weak[0].correct, weak[0].errors, weak[0].accuracy),
        (2, 8, 20.0)
    );

    assert!(weak_keys(&BTreeMap::from([('0', [0, 0])]), 0).is_empty());
}

#[test]
fn intended_key_counts_filter_language_and_saturate_each_bucket() {
    let mut first = session("first-en", date!(2026 - 08 - 07), Language::En);
    first.intended_keys = BTreeMap::from([('x', [u64::MAX, 5])]);
    let mut second = session("second-en", date!(2026 - 08 - 07), Language::En);
    second.intended_keys = BTreeMap::from([('x', [1, u64::MAX])]);
    let mut korean = session("other-ko", date!(2026 - 08 - 07), Language::Ko);
    korean.intended_keys = BTreeMap::from([('x', [7, 7]), ('한', [9, 1])]);

    assert_eq!(
        intended_key_counts(&[first, second, korean], Language::En),
        BTreeMap::from([('x', [u64::MAX, u64::MAX])])
    );
}

struct TestDir(PathBuf);

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn adaptive_catalog() -> (TestDir, ContentCatalog) {
    let nonce = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
    let dir = TestDir(
        std::env::temp_dir().join(format!("typerlude-stats-{}-{nonce}", std::process::id())),
    );
    fs::create_dir_all(&dir.0).unwrap();
    fs::write(
        dir.0.join("fixture-en.toml"),
        r#"
schema_version = 1
id = "stats-fixture-en"
title = "Stats fixture English"
language = "en"

[source]
author = "Typerlude contributors"
source_id = "stats-fixture-en"
source_url = "https://github.com/baba9811/typerlude"
license = "CC0-1.0"
license_url = "https://creativecommons.org/publicdomain/zero/1.0/"
modified = false
retrieved_at = "2026-08-07"

[[items]]
id = "fixture-repeat"
kind = "word"
text = "☃☃☃"
difficulty = 1

[[items]]
id = "fixture-tie-a"
kind = "word"
text = "☃a"
difficulty = 1

[[items]]
id = "fixture-tie-b"
kind = "word"
text = "b☃"
difficulty = 1

[[items]]
id = "fixture-tie-c"
kind = "sentence"
text = "See ☃."

[[items]]
id = "fixture-star"
kind = "word"
text = "★"
difficulty = 1

[[items]]
id = "fixture-umbrella"
kind = "word"
text = "☂"
difficulty = 1

[[items]]
id = "fixture-yin"
kind = "word"
text = "☯"
difficulty = 1

[[items]]
id = "fixture-plain"
kind = "word"
text = "plain"
difficulty = 1

[[items]]
id = "fixture-quote"
kind = "quote"
text = "☃ quoted"
"#,
    )
    .unwrap();
    fs::write(
        dir.0.join("fixture-ko.toml"),
        r#"
schema_version = 1
id = "stats-fixture-ko"
title = "Stats fixture Korean"
language = "ko"

[source]
author = "Typerlude contributors"
source_id = "stats-fixture-ko"
source_url = "https://github.com/baba9811/typerlude"
license = "CC0-1.0"
license_url = "https://creativecommons.org/publicdomain/zero/1.0/"
modified = false
retrieved_at = "2026-08-07"

[[items]]
id = "fixture-ko-snow"
kind = "word"
text = "눈☃"
difficulty = 1

[[items]]
id = "fixture-ko-plain"
kind = "word"
text = "연습"
difficulty = 1
"#,
    )
    .unwrap();
    let loaded = ContentCatalog::load(&dir.0).unwrap();
    assert!(loaded.warnings.is_empty());
    (dir, loaded.catalog)
}

fn key_session(language: Language, counts: BTreeMap<char, [u64; 2]>) -> SessionRecord {
    let mut record = session("keys", date!(2026 - 08 - 07), language);
    record.intended_keys = counts;
    record
}

#[test]
fn adaptive_scoring_counts_repeated_keys_and_seed_shuffles_only_equal_scores() {
    let (_dir, catalog) = adaptive_catalog();
    let sessions = [key_session(Language::En, BTreeMap::from([('☃', [0, 10])]))];

    let first = adaptive_candidates(&catalog, &sessions, Language::En, 11)
        .into_iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let again = adaptive_candidates(&catalog, &sessions, Language::En, 11)
        .into_iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let other_seed = adaptive_candidates(&catalog, &sessions, Language::En, 12)
        .into_iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();

    assert_eq!(first[0], "fixture-repeat");
    assert_eq!(
        first[1..]
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        ["fixture-tie-a", "fixture-tie-b", "fixture-tie-c"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert_eq!(first, again);
    assert_ne!(first, other_seed);
}

#[test]
fn adaptive_selection_uses_only_three_weakest_sufficiently_sampled_keys() {
    let (_dir, catalog) = adaptive_catalog();
    let sessions = [key_session(
        Language::En,
        BTreeMap::from([('☃', [0, 10]), ('★', [1, 9]), ('☂', [2, 8]), ('☯', [3, 7])]),
    )];

    let candidates = adaptive_candidates(&catalog, &sessions, Language::En, 9);

    assert!(candidates.iter().any(|item| item.id == "fixture-star"));
    assert!(candidates.iter().any(|item| item.id == "fixture-umbrella"));
    assert!(!candidates.iter().any(|item| item.id == "fixture-yin"));
    assert!(candidates.iter().all(|item| {
        item.language == Language::En
            && matches!(item.kind, ContentKind::Word | ContentKind::Sentence)
    }));
}

#[test]
fn adaptive_history_is_language_isolated() {
    let (_dir, catalog) = adaptive_catalog();
    let sessions = [key_session(Language::Ko, BTreeMap::from([('☃', [0, 10])]))];
    let ordinary_count = catalog
        .items()
        .filter(|item| {
            item.language == Language::En
                && matches!(item.kind, ContentKind::Word | ContentKind::Sentence)
        })
        .count();

    let candidates = adaptive_candidates(&catalog, &sessions, Language::En, 7);

    assert_eq!(candidates.len(), ordinary_count);
    assert!(candidates.iter().all(|item| item.language == Language::En));
}

#[test]
fn no_weak_key_falls_back_to_seeded_ordinary_language_candidates() {
    let (_dir, catalog) = adaptive_catalog();
    let sessions = [key_session(Language::En, BTreeMap::from([('☃', [0, 9])]))];
    let expected_count = catalog
        .items()
        .filter(|item| {
            item.language == Language::En
                && matches!(item.kind, ContentKind::Word | ContentKind::Sentence)
        })
        .count();

    let first = adaptive_candidates(&catalog, &sessions, Language::En, 21);
    let again = adaptive_candidates(&catalog, &sessions, Language::En, 21);
    let other_seed = adaptive_candidates(&catalog, &sessions, Language::En, 22);

    assert_eq!(first.len(), expected_count);
    assert!(first.iter().all(|item| {
        item.language == Language::En
            && matches!(item.kind, ContentKind::Word | ContentKind::Sentence)
    }));
    assert_eq!(
        first
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        again
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_ne!(
        first
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        other_seed
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
    );
}

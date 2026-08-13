use super::{InputOutcome, PracticeEngine};
use crate::model::{Language, PracticeKind};
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

fn line_texts(engine: &PracticeEngine) -> Vec<String> {
    let graphemes = engine
        .target_cells()
        .map(|(grapheme, _)| grapheme)
        .collect::<Vec<_>>();
    engine
        .line_ranges()
        .map(|range| graphemes[range].concat())
        .collect()
}

#[test]
fn logical_lines_keep_english_words_and_korean_eojeol_whole() {
    let english = format!("{} world", "a".repeat(68));
    let engine = PracticeEngine::new_for_items(
        Language::En,
        PracticeKind::Long,
        &english,
        &[english.graphemes(true).count()],
        None,
    )
    .unwrap();
    assert_eq!(
        line_texts(&engine),
        [format!("{} ", "a".repeat(68)), "world".into()]
    );

    let korean = format!("{} 세계", "가".repeat(34));
    let engine = PracticeEngine::new_for_items(
        Language::Ko,
        PracticeKind::Long,
        &korean,
        &[korean.graphemes(true).count()],
        None,
    )
    .unwrap();
    assert_eq!(
        line_texts(&engine),
        [format!("{} ", "가".repeat(34)), "세계".into()]
    );
}

#[test]
fn a_single_oversized_token_splits_only_at_grapheme_boundaries() {
    let target = "x".repeat(73);
    let engine =
        PracticeEngine::new_for_items(Language::En, PracticeKind::Long, &target, &[73], None)
            .unwrap();

    assert_eq!(
        line_texts(&engine)
            .iter()
            .map(String::len)
            .collect::<Vec<_>>(),
        [72, 1]
    );
}

#[test]
fn correction_does_not_erase_an_accuracy_error_or_inflate_speed() {
    let start = Instant::now();
    let mut engine = PracticeEngine::new(Language::Ko, PracticeKind::Key, "한", None).unwrap();
    engine.input("강", start);
    assert!(engine.backspace());
    engine.input("한", start + Duration::from_secs(60));

    let metrics = engine.metrics(start + Duration::from_secs(60));
    assert_eq!(metrics.correct_units, 3);
    assert_eq!(metrics.attempted_units, 6);
    assert_eq!(metrics.errors, 1);
    assert_eq!(metrics.backspaces, 1);
    assert_eq!(metrics.kpm, 3.0);
    assert_eq!(metrics.accuracy, 50.0);
}

#[test]
fn paused_time_is_excluded_and_tests_refuse_pause() {
    let start = Instant::now();
    let mut practice = PracticeEngine::new(Language::En, PracticeKind::Words, "ab", None).unwrap();
    practice.input("a", start);
    assert!(practice.toggle_pause(start + Duration::from_secs(10)));
    assert!(practice.toggle_pause(start + Duration::from_secs(40)));
    practice.input("b", start + Duration::from_secs(70));
    assert_eq!(
        practice.metrics(start + Duration::from_secs(70)).active,
        Duration::from_secs(40)
    );

    let mut test = PracticeEngine::new(
        Language::En,
        PracticeKind::Test,
        "ab",
        Some(Duration::from_secs(60)),
    )
    .unwrap();
    assert!(!test.toggle_pause(start));
}

#[test]
fn a_current_pause_does_not_add_active_time() {
    let start = Instant::now();
    let mut engine = PracticeEngine::new(Language::En, PracticeKind::Words, "ab", None).unwrap();
    engine.input("a", start);
    engine.toggle_pause(start + Duration::from_secs(10));

    assert_eq!(
        engine.metrics(start + Duration::from_secs(40)).active,
        Duration::from_secs(10)
    );
}

#[test]
fn a_timed_test_finishes_at_its_active_deadline() {
    let start = Instant::now();
    let mut engine = PracticeEngine::new(
        Language::En,
        PracticeKind::Test,
        "abcdef",
        Some(Duration::from_secs(60)),
    )
    .unwrap();
    engine.input("a", start);

    assert!(!engine.is_finished(start + Duration::from_secs(59)));
    assert!(engine.is_finished(start + Duration::from_secs(60)));
}

#[test]
fn timed_finalization_clamps_late_active_time_and_then_freezes() {
    let start = Instant::now();
    let mut test = PracticeEngine::new(
        Language::En,
        PracticeKind::Test,
        "abcdef",
        Some(Duration::from_secs(60)),
    )
    .unwrap();
    test.input("a", start);
    let test_metrics = test.finalize(start + Duration::from_secs(90));
    assert_eq!(test_metrics.active, Duration::from_secs(60));
    assert_eq!(test.metrics(start + Duration::from_secs(300)), test_metrics);

    let mut quick = PracticeEngine::new(
        Language::En,
        PracticeKind::Quick,
        "abcdef",
        Some(Duration::from_secs(60)),
    )
    .unwrap();
    quick.input("a", start);
    assert!(quick.toggle_pause(start + Duration::from_secs(10)));
    assert!(quick.toggle_pause(start + Duration::from_secs(40)));
    assert_eq!(
        quick.finalize(start + Duration::from_secs(100)).active,
        Duration::from_secs(60)
    );
}

#[test]
fn empty_targets_are_rejected() {
    assert!(PracticeEngine::new(Language::En, PracticeKind::Words, "", None).is_err());
}

#[test]
fn empty_input_does_not_start_the_timer_or_change_accuracy() {
    let start = Instant::now();
    let mut engine = PracticeEngine::new(Language::En, PracticeKind::Words, "a", None).unwrap();

    assert_eq!(engine.input("", start), InputOutcome::Accepted);
    let metrics = engine.metrics(start + Duration::from_secs(60));
    assert_eq!(metrics.active, Duration::ZERO);
    assert_eq!(metrics.attempted_units, 0);
    assert_eq!(metrics.accuracy, 100.0);
}

#[test]
fn multi_grapheme_input_is_normalized_and_stops_at_completion() {
    let start = Instant::now();
    let mut engine =
        PracticeEngine::new(Language::Ko, PracticeKind::Sentence, "한글", None).unwrap();

    assert_eq!(engine.input("한글더", start), InputOutcome::Finished);
    let metrics = engine.metrics(start + Duration::from_secs(60));
    assert_eq!(metrics.correct_units, 6);
    assert_eq!(metrics.attempted_units, 6);
    assert_eq!(metrics.errors, 0);
    assert_eq!(metrics.cpm, 2.0);
    assert_eq!(metrics.kpm, 6.0);
    assert_eq!(metrics.wpm, 0.4);
}

#[test]
fn intended_key_buckets_track_correct_and_erroneous_attempts() {
    let start = Instant::now();
    let mut engine = PracticeEngine::new(Language::Ko, PracticeKind::Key, "한", None).unwrap();
    engine.input("강", start);
    engine.backspace();
    engine.input("한", start);

    assert_eq!(engine.intended.get(&'ㅎ'), Some(&[1, 1]));
    assert_eq!(engine.intended.get(&'ㅏ'), Some(&[1, 1]));
    assert_eq!(engine.intended.get(&'ㄴ'), Some(&[1, 1]));
}

#[test]
fn a_wrong_full_length_key_cell_requires_correction() {
    let start = Instant::now();
    let mut engine = PracticeEngine::new(Language::En, PracticeKind::Key, "a", None).unwrap();

    assert_eq!(engine.input("x", start), InputOutcome::Accepted);
    assert!(!engine.is_finished(start));
    assert_eq!(engine.input("a", start), InputOutcome::Accepted);
    assert_eq!(engine.metrics(start).attempted_units, 1);
    assert!(engine.backspace());
    assert_eq!(engine.input("a", start), InputOutcome::Finished);
    assert_eq!(engine.metrics(start).attempted_units, 2);
}

#[test]
fn a_wrong_line_advances_and_finishes_non_key_practice() {
    let start = Instant::now();
    let mut engine =
        PracticeEngine::new_for_items(Language::En, PracticeKind::Sentence, "ab cd", &[3, 5], None)
            .unwrap();

    engine.input("ax", start);
    assert_eq!(engine.current_line_index(), 0);
    engine.input(" ", start);
    assert_eq!(engine.current_line_index(), 1);
    engine.input("cd", start);

    assert!(engine.target_complete());
    assert_eq!(engine.metrics(start).errors, 1);
}

#[test]
fn enter_marks_the_untyped_remainder_wrong_and_moves_on() {
    let start = Instant::now();
    let mut engine = PracticeEngine::new_for_items(
        Language::En,
        PracticeKind::Sentence,
        "hello world",
        &[6, 11],
        None,
    )
    .unwrap();

    engine.input("he", start);
    engine.submit_line(start);

    assert_eq!(engine.cursor(), 6);
    assert_eq!(engine.metrics(start).errors, 4);
    assert_eq!(engine.current_line_index(), 1);
}

#[test]
fn one_enter_crosses_a_blank_line_run() {
    let start = Instant::now();
    let mut engine =
        PracticeEngine::new_for_items(Language::En, PracticeKind::Long, "a\n\nb", &[3, 4], None)
            .unwrap();

    engine.input("a", start);
    assert_eq!(engine.submit_line(start), InputOutcome::Accepted);

    assert_eq!(engine.cursor(), 3);
    assert_eq!(engine.current_line_index(), 2);
    let metrics = engine.metrics(start);
    assert_eq!(metrics.attempted_units, 2);
    assert_eq!(metrics.correct_units, 2);
    assert_eq!(metrics.errors, 0);
}

#[test]
fn an_incomplete_line_submission_also_crosses_a_blank_line_run() {
    let start = Instant::now();
    let mut engine =
        PracticeEngine::new_for_items(Language::En, PracticeKind::Long, "ab\n\nc", &[4, 5], None)
            .unwrap();

    engine.input("a", start);
    assert_eq!(engine.submit_line(start), InputOutcome::Accepted);

    assert_eq!(engine.cursor(), 4);
    assert_eq!(engine.current_line_index(), 2);
    let metrics = engine.metrics(start);
    assert_eq!(metrics.attempted_units, 3);
    assert_eq!(metrics.correct_units, 2);
    assert_eq!(metrics.errors, 1);
}

#[test]
fn backspace_reopens_before_deleting_and_history_never_decreases() {
    let start = Instant::now();
    let mut engine =
        PracticeEngine::new_for_items(Language::En, PracticeKind::Sentence, "a b", &[2, 3], None)
            .unwrap();
    engine.input("a ", start);
    let attempted = engine.metrics(start).attempted_units;

    assert!(engine.backspace());
    assert_eq!(engine.cursor(), 2);
    assert!(engine.backspace());
    assert_eq!(engine.cursor(), 1);
    assert_eq!(engine.metrics(start).attempted_units, attempted);
    assert_eq!(engine.metrics(start).backspaces, 2);
}

#[test]
fn paused_finished_and_timed_out_inputs_do_not_change_metrics() {
    let start = Instant::now();
    let mut paused = PracticeEngine::new(Language::En, PracticeKind::Words, "ab", None).unwrap();
    paused.input("a", start);
    paused.toggle_pause(start);
    let before_pause = paused.metrics(start);
    assert_eq!(
        paused.input("x", start + Duration::from_secs(1)),
        InputOutcome::IgnoredWhilePaused
    );
    assert_eq!(paused.metrics(start), before_pause);

    let mut completed = PracticeEngine::new(Language::En, PracticeKind::Words, "a", None).unwrap();
    assert_eq!(completed.input("a", start), InputOutcome::Finished);
    let before_extra = completed.metrics(start);
    assert_eq!(completed.input("x", start), InputOutcome::Finished);
    assert_eq!(completed.metrics(start), before_extra);

    let mut timed = PracticeEngine::new(
        Language::En,
        PracticeKind::Test,
        "ab",
        Some(Duration::from_secs(1)),
    )
    .unwrap();
    timed.input("a", start);
    let at_deadline = timed.metrics(start + Duration::from_secs(1));
    assert_eq!(
        timed.input("b", start + Duration::from_secs(1)),
        InputOutcome::Finished
    );
    assert_eq!(timed.metrics(start + Duration::from_secs(1)), at_deadline);
}

#[test]
fn extending_a_completed_target_preserves_time_totals_and_normalizes() {
    let start = Instant::now();
    let mut engine = PracticeEngine::new(Language::En, PracticeKind::Key, "a", None).unwrap();

    assert_eq!(engine.input("x", start), InputOutcome::Accepted);
    assert!(engine.backspace());
    assert_eq!(engine.input("a", start), InputOutcome::Finished);
    assert!(engine.target_complete());
    assert!(engine.toggle_pause(start + Duration::from_secs(10)));
    assert!(engine.toggle_pause(start + Duration::from_secs(40)));
    engine.extend_target(" ", "e\u{301}", &[1]).unwrap();
    assert!(!engine.target_complete());
    assert_eq!(
        engine
            .target_cells()
            .map(|(grapheme, _)| grapheme)
            .collect::<String>(),
        "a é"
    );

    assert_eq!(
        engine.input(" é", start + Duration::from_secs(70)),
        InputOutcome::Finished
    );
    let metrics = engine.metrics(start + Duration::from_secs(70));
    assert_eq!(metrics.active, Duration::from_secs(40));
    assert_eq!(metrics.correct_units, 3);
    assert_eq!(metrics.attempted_units, 4);
    assert_eq!(engine.attempted_units(), 4);
    assert_eq!(metrics.errors, 1);
    assert_eq!(metrics.backspaces, 1);
    assert_eq!(metrics.cpm, 4.5);
    assert_eq!(metrics.accuracy, 75.0);
    assert_eq!(engine.intended_keys().get(&'a'), Some(&[1, 1]));
}

#[test]
fn empty_or_finalized_extension_is_transactional_and_finalization_freezes() {
    let start = Instant::now();
    let mut engine = PracticeEngine::new(Language::En, PracticeKind::Words, "ab", None).unwrap();
    engine.input("a", start);
    let target_before = engine
        .target_cells()
        .map(|(grapheme, entered)| (grapheme.to_owned(), entered))
        .collect::<Vec<_>>();
    let metrics_before = engine.metrics(start + Duration::from_secs(10));

    assert!(engine.extend_target(" ", "", &[]).is_err());
    assert_eq!(
        engine
            .target_cells()
            .map(|(grapheme, entered)| (grapheme.to_owned(), entered))
            .collect::<Vec<_>>(),
        target_before
    );
    assert_eq!(
        engine.metrics(start + Duration::from_secs(10)),
        metrics_before
    );

    let frozen = engine.finalize(start + Duration::from_secs(10));
    assert_eq!(engine.metrics(start + Duration::from_secs(60)), frozen);
    assert!(engine.extend_target(" ", "c", &[1]).is_err());
    assert_eq!(
        engine.input("b", start + Duration::from_secs(60)),
        InputOutcome::Finished
    );
    assert!(!engine.backspace());
    assert!(!engine.toggle_pause(start + Duration::from_secs(60)));
}

#[test]
fn pause_blocks_backspace_and_time_limit_is_distinct_from_target_completion() {
    let start = Instant::now();
    let mut paused = PracticeEngine::new(Language::En, PracticeKind::Words, "ab", None).unwrap();
    paused.input("a", start);
    assert!(paused.toggle_pause(start));
    assert!(paused.is_paused());
    assert!(!paused.backspace());
    assert_eq!(paused.cursor(), 1);

    let mut timed = PracticeEngine::new(
        Language::En,
        PracticeKind::Test,
        "a",
        Some(Duration::from_secs(60)),
    )
    .unwrap();
    timed.input("a", start);
    assert!(timed.target_complete());
    assert!(!timed.time_limit_reached(start + Duration::from_secs(59)));
    assert!(timed.time_limit_reached(start + Duration::from_secs(60)));
    assert_eq!(timed.language(), Language::En);
}

#[test]
fn best_rolling_speeds_use_the_last_thirty_active_seconds() {
    let start = Instant::now();
    let mut engine =
        PracticeEngine::new(Language::En, PracticeKind::Long, &"a".repeat(40), None).unwrap();

    for second in 0..=30 {
        engine.input("a", start + Duration::from_secs(second));
    }

    let (kpm, wpm) = engine.best_rolling_speeds();
    assert!((kpm - 60.0).abs() < f64::EPSILON * 8.0);
    assert!((wpm - 12.0).abs() < f64::EPSILON * 8.0);

    let mut korean =
        PracticeEngine::new(Language::Ko, PracticeKind::Long, &"가".repeat(40), None).unwrap();
    for second in 0..=30 {
        korean.input("가", start + Duration::from_secs(second));
    }
    let (kpm, wpm) = korean.best_rolling_speeds();
    assert!((kpm - 120.0).abs() < f64::EPSILON * 8.0);
    assert!((wpm - 12.0).abs() < f64::EPSILON * 8.0);
}

#[test]
fn out_of_order_event_times_cannot_inflate_rolling_speed() {
    let start = Instant::now();
    let mut ordered = PracticeEngine::new(Language::En, PracticeKind::Long, "aaaaa", None).unwrap();
    let mut out_of_order =
        PracticeEngine::new(Language::En, PracticeKind::Long, "aaaaa", None).unwrap();
    for second in [0, 20, 30, 50] {
        ordered.input("a", start + Duration::from_secs(second));
    }
    for second in [0, 30, 20, 50] {
        out_of_order.input("a", start + Duration::from_secs(second));
    }

    let (out_of_order_kpm, out_of_order_wpm) = out_of_order.best_rolling_speeds();
    let (ordered_kpm, ordered_wpm) = ordered.best_rolling_speeds();
    assert!(out_of_order_kpm <= ordered_kpm);
    assert!(out_of_order_wpm <= ordered_wpm);
}

#[test]
fn large_ascii_target_uses_compact_indexed_storage() {
    let target = "a".repeat(1024 * 1024);
    let engine = PracticeEngine::new(Language::En, PracticeKind::Long, &target, None).unwrap();
    let indexed_bytes =
        engine.target.capacity() + engine.target_ends.capacity() * std::mem::size_of::<u32>();

    assert_eq!(engine.target_len(), target.len());
    assert!(indexed_bytes <= target.len() * 6, "{indexed_bytes}");
}

use super::{FallingWord, LOGICAL_HEIGHT, LOGICAL_WIDTH, WordRain};
use crate::model::{Difficulty, Language};
use std::time::{Duration, Instant};

fn game(difficulty: Difficulty, words: &[&str], now: Instant) -> WordRain {
    WordRain::new(
        Language::En,
        difficulty,
        words.iter().map(|word| (*word).to_owned()).collect(),
        7,
        now,
    )
    .unwrap()
}

#[test]
fn difficulty_controls_base_fall_and_spawn_times() {
    let now = Instant::now();
    for (difficulty, fall, spawn) in [
        (Difficulty::Easy, 18.0, 2.4),
        (Difficulty::Medium, 14.0, 2.0),
        (Difficulty::Hard, 10.0, 1.6),
    ] {
        let game = game(difficulty, &["alpha"], now);
        assert_eq!(game.effective_fall_time(), Duration::from_secs_f64(fall));
        assert_eq!(game.spawn_interval(), Duration::from_secs_f64(spawn));
    }
    assert!(
        WordRain::new(
            Language::En,
            Difficulty::Mixed,
            vec!["alpha".into()],
            7,
            now,
        )
        .is_err()
    );
}

#[test]
fn level_speed_is_exponential_and_not_capped_at_three_times() {
    let now = Instant::now();
    let mut game = game(Difficulty::Medium, &["alpha"], now);
    game.cleared = 120;

    assert_eq!(game.level(), 13);
    assert!(game.speed_multiplier() > 3.0);
    assert!(game.spawn_interval() < Duration::from_secs_f64(2.0 / 3.0));
}

#[test]
fn new_starts_with_one_visible_word() {
    let now = Instant::now();
    let game = game(Difficulty::Easy, &["alpha", "beta"], now);

    assert_eq!(game.active.len(), 1);
    let word = &game.active[0];
    assert!(word.left.saturating_add(word.width) <= LOGICAL_WIDTH);
    assert_eq!(word.progress, 0.0);
}

#[test]
fn tick_spawns_at_most_one_word_and_discards_successful_backlog() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["alpha", "beta", "gamma"], now);
    game.spawn_elapsed = Duration::from_secs(20);

    game.tick(now + Duration::from_millis(1));
    assert_eq!(game.active.len(), 2);
    assert_eq!(game.spawn_elapsed, Duration::ZERO);

    game.tick(now + Duration::from_millis(2));
    assert_eq!(game.active.len(), 2);
}

#[test]
fn collision_finishes_before_a_same_tick_spawn() {
    let now = Instant::now();
    let mut game = game(Difficulty::Hard, &["alpha", "beta"], now);
    game.active[0].text = "alpha".into();
    game.active[0].progress = 0.99;
    game.spawn_elapsed = Duration::from_secs(20);

    game.tick(now + Duration::from_millis(250));

    assert!(game.outcome.is_some());
    assert_eq!(game.active.len(), 1);
    assert_eq!(game.outcome.as_ref().unwrap().missed_word, "alpha");
}

#[test]
fn spawn_positions_stay_in_bounds_and_keep_two_cell_padding() {
    let now = Instant::now();
    let mut game = game(
        Difficulty::Easy,
        &["aaaaaaaa", "bbbbbbbb", "cccccccc", "dddddddd"],
        now,
    );

    for _ in 0..3 {
        assert!(game.spawn());
    }

    for word in &game.active {
        assert!(word.left.saturating_add(word.width) <= LOGICAL_WIDTH);
    }
    for (index, word) in game.active.iter().enumerate() {
        for other in &game.active[index + 1..] {
            if (word.progress - other.progress).abs() < 2.0 / LOGICAL_HEIGHT {
                let separated = word.left.saturating_add(word.width).saturating_add(2)
                    <= other.left
                    || other.left.saturating_add(other.width).saturating_add(2) <= word.left;
                assert!(separated, "{word:?} overlaps {other:?}");
            }
        }
    }
}

#[test]
fn spawn_waits_when_every_column_is_blocked() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["abcdefghijklmnopqrstuvwx"], now);
    game.active = vec![
        FallingWord {
            id: 1,
            text: "left".into(),
            width: 24,
            left: 0,
            progress: 0.0,
        },
        FallingWord {
            id: 2,
            text: "middle".into(),
            width: 24,
            left: 24,
            progress: 0.0,
        },
        FallingWord {
            id: 3,
            text: "right".into(),
            width: 24,
            left: 48,
            progress: 0.0,
        },
    ];
    game.spawn_elapsed = Duration::from_secs(20);

    game.tick(now + Duration::from_millis(1));

    assert_eq!(game.active.len(), 3);
    assert!(game.spawn_elapsed >= Duration::from_secs(20));
}

#[test]
fn pause_and_viewport_suspension_exclude_elapsed_time() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["alpha", "beta"], now);
    let initial = game.active[0].progress;

    assert!(game.toggle_pause(now));
    game.tick(now + Duration::from_secs(5));
    assert_eq!(game.active[0].progress, initial);
    assert_eq!(game.active_time, Duration::ZERO);

    assert!(game.toggle_pause(now + Duration::from_secs(5)));
    game.tick(now + Duration::from_millis(5_250));
    let after_resume = game.active[0].progress;
    assert!(after_resume > initial);

    game.set_viewport_supported(false, now + Duration::from_millis(5_250));
    game.tick(now + Duration::from_secs(100));
    assert_eq!(game.active[0].progress, after_resume);

    game.set_viewport_supported(true, now + Duration::from_secs(100));
    game.tick(now + Duration::from_millis(100_250));
    assert!(game.active[0].progress > after_resume);
    assert_eq!(game.active_time, Duration::from_millis(500));
}

#[test]
fn spawn_prefers_an_unused_initial() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["apple", "apricot", "banana"], now);
    game.active = vec![FallingWord {
        id: 1,
        text: "apple".into(),
        width: 5,
        left: 0,
        progress: 0.5,
    }];

    assert!(game.spawn());
    assert_eq!(game.active.last().unwrap().text, "banana");
}

#[test]
fn spawn_falls_back_to_an_unused_word_with_a_repeated_initial() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["apple", "apricot"], now);
    game.active = vec![FallingWord {
        id: 1,
        text: "apple".into(),
        width: 5,
        left: 0,
        progress: 0.5,
    }];

    assert!(game.spawn());
    assert_eq!(game.active.last().unwrap().text, "apricot");
}

#[test]
fn a_small_pool_can_repeat_an_active_word() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["apple"], now);
    game.active[0].progress = 0.5;

    assert!(game.spawn());
    assert_eq!(game.active.last().unwrap().text, "apple");
}

#[test]
fn first_input_targets_the_lowest_matching_word() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["apple", "atom"], now);
    game.active = vec![
        FallingWord {
            id: 1,
            text: "apple".into(),
            width: 5,
            left: 0,
            progress: 0.4,
        },
        FallingWord {
            id: 2,
            text: "atom".into(),
            width: 4,
            left: 20,
            progress: 0.7,
        },
    ];

    game.input_char('a');

    assert_eq!(game.target_id(), Some(2));
}

#[test]
fn equal_height_target_selection_prefers_the_older_word() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["apple", "atom"], now);
    game.active = vec![
        FallingWord {
            id: 1,
            text: "apple".into(),
            width: 5,
            left: 0,
            progress: 0.7,
        },
        FallingWord {
            id: 2,
            text: "atom".into(),
            width: 4,
            left: 20,
            progress: 0.7,
        },
    ];

    game.input_char('a');

    assert_eq!(game.target_id(), Some(1));
}

#[test]
fn korean_partial_input_selects_a_word() {
    let now = Instant::now();
    let mut game =
        WordRain::new(Language::Ko, Difficulty::Easy, vec!["안녕".into()], 7, now).unwrap();
    let id = game.active[0].id;

    game.input_char('ㅇ');

    assert_eq!(game.target_id(), Some(id));
    assert!(game.input_is_valid());
    assert_eq!(game.input(), "ㅇ");
}

#[test]
fn invalid_input_is_retained_and_never_captured_by_a_later_word() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["alpha"], now);
    game.combo = 4;

    game.input_char('z');
    game.active.push(FallingWord {
        id: 99,
        text: "zebra".into(),
        width: 5,
        left: 30,
        progress: 0.8,
    });
    game.input_char('e');

    assert_eq!(game.input(), "ze");
    assert!(!game.input_is_valid());
    assert_eq!(game.target_id(), None);
    assert_eq!(game.combo, 0);
}

#[test]
fn backspace_removes_the_latest_character_and_empty_input_unlocks_target() {
    let now = Instant::now();
    let mut game = WordRain::new(
        Language::Ko,
        Difficulty::Easy,
        vec!["안녕".into(), "아침".into()],
        7,
        now,
    )
    .unwrap();
    game.active = vec![FallingWord {
        id: 1,
        text: "안녕".into(),
        width: 4,
        left: 0,
        progress: 0.5,
    }];

    for character in ['안', 'ㄴ', '녕'] {
        game.input_char(character);
    }
    assert_eq!(game.input(), "안ㄴ녕");
    assert!(!game.input_is_valid());

    assert!(game.backspace());
    assert_eq!(game.input(), "안ㄴ");
    assert!(game.backspace());
    assert_eq!(game.input(), "안");
    assert!(game.input_is_valid());
    assert_eq!(game.target_id(), Some(1));

    assert!(game.backspace());
    assert_eq!(game.input(), "");
    assert_eq!(game.target_id(), None);
}

#[test]
fn empty_input_can_retarget_a_different_word() {
    let now = Instant::now();
    let mut game = WordRain::new(
        Language::Ko,
        Difficulty::Easy,
        vec!["안녕".into(), "바다".into()],
        7,
        now,
    )
    .unwrap();
    game.active = vec![
        FallingWord {
            id: 1,
            text: "안녕".into(),
            width: 4,
            left: 0,
            progress: 0.8,
        },
        FallingWord {
            id: 2,
            text: "바다".into(),
            width: 4,
            left: 20,
            progress: 0.5,
        },
    ];

    game.input_char('ㅇ');
    assert_eq!(game.target_id(), Some(1));
    assert!(game.backspace());
    game.input_char('ㅂ');

    assert_eq!(game.target_id(), Some(2));
    assert_eq!(game.input(), "ㅂ");
}

#[test]
fn wrong_input_resets_combo_and_backspace_does_not_restore_it() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["alpha"], now);
    game.combo = 3;

    game.input_char('z');
    assert_eq!(game.combo, 0);
    assert!(game.backspace());
    assert_eq!(game.combo, 0);
}

#[test]
fn completion_scores_before_the_level_transition_using_typing_units() {
    let now = Instant::now();
    let mut game =
        WordRain::new(Language::Ko, Difficulty::Easy, vec!["안녕".into()], 7, now).unwrap();
    game.cleared = 9;
    game.combo = 2;

    game.input_char('안');
    game.input_char('녕');

    assert_eq!(game.score, 78);
    assert_eq!(game.combo, 3);
    assert_eq!(game.max_combo, 3);
    assert_eq!(game.cleared, 10);
    assert_eq!(game.level(), 2);
    assert!(game.active.is_empty());
    assert_eq!(game.input(), "");
    assert_eq!(game.target_id(), None);
}

#[test]
fn english_targeting_is_case_insensitive() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["alpha"], now);

    for character in "ALPHA".chars() {
        game.input_char(character);
    }

    assert_eq!(game.cleared, 1);
    assert!(game.active.is_empty());
}

#[test]
fn score_and_counters_saturate() {
    let now = Instant::now();
    let mut game = game(Difficulty::Easy, &["a"], now);
    game.score = u64::MAX - 1;
    game.combo = u64::MAX;
    game.max_combo = u64::MAX;
    game.cleared = u64::MAX;

    game.input_char('a');

    assert_eq!(game.score, u64::MAX);
    assert_eq!(game.combo, u64::MAX);
    assert_eq!(game.max_combo, u64::MAX);
    assert_eq!(game.cleared, u64::MAX);
}

#[test]
fn matched_graphemes_count_only_complete_target_graphemes() {
    let now = Instant::now();
    let mut game =
        WordRain::new(Language::Ko, Difficulty::Easy, vec!["안녕".into()], 7, now).unwrap();
    let id = game.active[0].id;

    game.input_char('ㅇ');
    assert_eq!(game.matched_graphemes(id), 0);
    game.backspace();
    game.input_char('안');
    assert_eq!(game.matched_graphemes(id), 1);
}

#[test]
fn a_miss_snapshots_the_complete_outcome() {
    let now = Instant::now();
    let mut game = game(Difficulty::Hard, &["alpha"], now);
    game.active[0].progress = 0.99;
    game.score = 123;
    game.combo = 4;
    game.max_combo = 7;
    game.cleared = 20;

    game.tick(now + Duration::from_millis(250));

    let outcome = game.outcome.as_ref().unwrap();
    assert_eq!(outcome.score, 123);
    assert_eq!(outcome.cleared, 20);
    assert_eq!(outcome.max_combo, 7);
    assert_eq!(outcome.level, 3);
    assert_eq!(outcome.active_time, Duration::from_millis(250));
    assert_eq!(outcome.missed_word, "alpha");
}

#[test]
fn simultaneous_misses_choose_farthest_fallen_then_oldest_id() {
    let now = Instant::now();
    let mut lower_game = game(Difficulty::Hard, &["older", "lower"], now);
    lower_game.active = vec![
        FallingWord {
            id: 1,
            text: "older".into(),
            width: 5,
            left: 0,
            progress: 0.99,
        },
        FallingWord {
            id: 2,
            text: "lower".into(),
            width: 5,
            left: 20,
            progress: 1.01,
        },
    ];

    lower_game.tick(now + Duration::from_millis(250));
    assert_eq!(lower_game.outcome.as_ref().unwrap().missed_word, "lower");

    let mut tie_game = game(Difficulty::Hard, &["older", "newer"], now);
    tie_game.active = vec![
        FallingWord {
            id: 1,
            text: "older".into(),
            width: 5,
            left: 0,
            progress: 1.0,
        },
        FallingWord {
            id: 2,
            text: "newer".into(),
            width: 5,
            left: 20,
            progress: 1.0,
        },
    ];

    tie_game.tick(now);
    assert_eq!(tie_game.outcome.as_ref().unwrap().missed_word, "older");
}

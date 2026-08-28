use super::{
    BATTLE_LIMIT, BattleCue, BossBattle, BossBattleOutcome, BossKind, BossPatternView, BossPhase,
};
use crate::typing::{key_units, unit_count};
use crate::{
    content::{ContentCatalog, ContentKind},
    game::GameDifficulty,
    model::Language,
};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

const INTRO: Duration = Duration::from_millis(800);

fn battle(now: Instant) -> BossBattle {
    BossBattle::new(
        BossKind::IronWarden,
        Language::En,
        GameDifficulty::Easy,
        vec![
            "alpha".into(),
            "bravo".into(),
            "cider".into(),
            "delta".into(),
        ],
        7,
        now,
    )
    .unwrap()
}

fn advance(game: &mut BossBattle, now: &mut Instant, duration: Duration) {
    let end = *now + duration;
    while *now < end {
        *now = (*now + Duration::from_millis(250)).min(end);
        game.tick(*now);
    }
}

fn finish_intro(game: &mut BossBattle, now: &mut Instant) {
    advance(game, now, INTRO);
    assert_eq!(game.active_time(), Duration::ZERO);
}

fn type_current_prompt(game: &mut BossBattle) {
    let prompt = game.prompts().next().unwrap().text().to_owned();
    for character in prompt.chars() {
        game.input_char(character);
    }
}

fn type_current_prompt_at_kpm(game: &mut BossBattle, now: &mut Instant, kpm: u32) -> u64 {
    let prompt = game.prompts().next().unwrap().text().to_owned();
    let units = unit_count(game.language, &prompt);
    for character in prompt.chars() {
        let character_units = key_units(game.language, &character.to_string()).len() as f64;
        advance(
            game,
            now,
            Duration::from_secs_f64(character_units * 60.0 / f64::from(kpm)),
        );
        game.input_char(character);
    }
    units
}

fn queen(now: Instant, language: Language, words: &[&str]) -> BossBattle {
    BossBattle::new(
        BossKind::ThornQueen,
        language,
        GameDifficulty::Easy,
        words.iter().map(|word| (*word).to_owned()).collect(),
        7,
        now,
    )
    .unwrap()
}

fn archon(now: Instant) -> BossBattle {
    BossBattle::new(
        BossKind::NullArchon,
        Language::En,
        GameDifficulty::Easy,
        vec![
            "alpha".into(),
            "bravo".into(),
            "cider".into(),
            "delta".into(),
        ],
        7,
        now,
    )
    .unwrap()
}

fn drive_profile(
    game: &mut BossBattle,
    now: &mut Instant,
    kpm: u32,
    accuracy: f64,
) -> BossBattleOutcome {
    let unit_delay = Duration::from_secs_f64(60.0 / f64::from(kpm));
    let error_interval = 100.0 / (100.0 - accuracy);
    let mut next_error = error_interval;

    for _ in 0..100_000 {
        if let Some(outcome) = game.outcome().cloned() {
            return outcome;
        }
        if game.input_locked() {
            advance_locking_cue(game, now);
            continue;
        }

        let target = game.target.or_else(|| {
            game.prompts
                .iter()
                .max_by(|left, right| {
                    left.progress()
                        .total_cmp(&right.progress())
                        .then_with(|| right.id.cmp(&left.id))
                })
                .map(|prompt| prompt.id)
        });
        let Some((target, prompt)) = target.and_then(|id| {
            game.prompts
                .iter()
                .find(|prompt| prompt.id == id)
                .map(|prompt| (id, prompt.text.clone()))
        }) else {
            profile_delay(game, now, unit_delay);
            continue;
        };
        let prefix = if game.target == Some(target) {
            game.input.chars().count()
        } else {
            0
        };
        let Some(character) = prompt.chars().nth(prefix) else {
            game.submit_input();
            continue;
        };
        let units = key_units(game.language, &character.to_string()).len();

        if game.attempted_units as f64 >= next_error {
            if !profile_delay(game, now, unit_delay) {
                continue;
            }
            game.input_char('#');
            next_error += error_interval;
            if game.boss != BossKind::NullArchon && profile_delay(game, now, unit_delay) {
                game.backspace();
            }
            continue;
        }

        if !profile_delay(game, now, unit_delay.mul_f64(units as f64)) {
            continue;
        }
        if game.prompts.iter().any(|prompt| prompt.id == target)
            && game.target.is_none_or(|current| current == target)
        {
            game.input_char(character);
        }
    }
    panic!("scripted profile exceeded its iteration guard");
}

fn profile_delay(game: &mut BossBattle, now: &mut Instant, duration: Duration) -> bool {
    let end = *now + duration;
    while *now < end {
        *now = (*now + Duration::from_millis(250)).min(end);
        game.tick(*now);
        if game.outcome().is_some() || game.input_locked() {
            return false;
        }
    }
    true
}

fn advance_locking_cue(game: &mut BossBattle, now: &mut Instant) {
    let cue = game.cue.expect("input must be locked by a cue");
    let step = cue
        .duration
        .saturating_sub(cue.elapsed)
        .min(Duration::from_millis(250));
    *now += step;
    game.tick(*now);
}

fn scripted_outcome(
    catalog: &ContentCatalog,
    boss: BossKind,
    language: Language,
    difficulty: GameDifficulty,
    kpm: u32,
    accuracy: f64,
) -> BossBattleOutcome {
    let mut seen = HashSet::new();
    let words = catalog
        .select(language, ContentKind::Word, difficulty.content_difficulty())
        .into_iter()
        .map(|item| item.text.clone())
        .filter(|word| seen.insert(word.clone()))
        .collect();
    let mut now = Instant::now();
    let mut game = BossBattle::new(boss, language, difficulty, words, 7, now).unwrap();
    drive_profile(&mut game, &mut now, kpm, accuracy)
}

#[test]
fn iron_warden_breaks_three_locks_then_exposes_its_core() {
    let mut now = Instant::now();
    let mut game = battle(now);
    finish_intro(&mut game, &mut now);

    for expected_locks in 1..=3 {
        type_current_prompt(&mut game);
        let BossPatternView::Warden {
            locks,
            core_exposed,
            cast_progress: _,
        } = game.pattern_view()
        else {
            panic!("expected Warden state");
        };
        assert_eq!(locks, expected_locks);
        assert_eq!(core_exposed, expected_locks == 3);
    }
}

#[test]
fn intro_pause_and_unsupported_viewport_do_not_consume_active_time() {
    let mut now = Instant::now();
    let mut game = battle(now);
    finish_intro(&mut game, &mut now);

    assert!(game.toggle_pause(now));
    advance(&mut game, &mut now, Duration::from_secs(5));
    assert_eq!(game.active_time(), Duration::ZERO);
    assert!(game.toggle_pause(now));

    game.set_viewport_supported(false, now);
    advance(&mut game, &mut now, Duration::from_secs(5));
    assert_eq!(game.active_time(), Duration::ZERO);
    game.set_viewport_supported(true, now);

    advance(&mut game, &mut now, Duration::from_millis(250));
    assert_eq!(game.active_time(), Duration::from_millis(250));
}

#[test]
fn a_missed_pile_driver_costs_one_heart_and_resets_locks() {
    let mut now = Instant::now();
    let mut game = battle(now);
    finish_intro(&mut game, &mut now);
    type_current_prompt(&mut game);

    for _ in 0..200 {
        advance(&mut game, &mut now, Duration::from_millis(250));
        if game.hearts() < 3 {
            break;
        }
    }

    assert_eq!(game.hearts(), 2);
    assert_eq!(game.cue().map(|(cue, _)| cue), Some(BattleCue::BossAttack));
    assert!(matches!(
        game.pattern_view(),
        BossPatternView::Warden { locks: 0, .. }
    ));
}

#[test]
fn max_width_custom_words_keep_pattern_windows_fair_by_physical_key_units() {
    let cases = [
        (Language::En, "abcdefghijklmnopqrstuvwx".to_owned(), 24),
        (Language::Ko, "\u{ad05}".repeat(12), 60),
    ];

    for (language, word, expected_units) in cases {
        assert_eq!(UnicodeWidthStr::width(word.as_str()), 24);
        assert_eq!(unit_count(language, &word), expected_units);

        for boss in [BossKind::IronWarden, BossKind::NullArchon] {
            let mut now = Instant::now();
            let mut game = BossBattle::new(
                boss,
                language,
                GameDifficulty::Easy,
                vec![word.clone()],
                7,
                now,
            )
            .unwrap();
            game.health = 1_000;
            game.max_health = 1_000;
            finish_intro(&mut game, &mut now);

            for _ in 0..3 {
                type_current_prompt_at_kpm(&mut game, &mut now, 180);
            }
            assert_eq!(game.hearts(), 3, "{boss:?} {language:?}");

            match boss {
                BossKind::IronWarden => {
                    assert!(matches!(
                        game.pattern_view(),
                        BossPatternView::Warden {
                            locks: 3,
                            core_exposed: true,
                            ..
                        }
                    ));
                    let health = game.health();
                    let units = type_current_prompt_at_kpm(&mut game, &mut now, 180);
                    assert_eq!(health - game.health(), units * 2, "{language:?}");
                }
                BossKind::NullArchon => assert!(matches!(
                    game.pattern_view(),
                    BossPatternView::NullArchon { checksum: 0, .. }
                )),
                BossKind::ThornQueen => unreachable!(),
            }
        }
    }
}

#[test]
fn ninety_active_seconds_produces_one_timeout_defeat_after_its_cinematic() {
    let mut now = Instant::now();
    let mut game = battle(now);
    finish_intro(&mut game, &mut now);
    game.hearts = u8::MAX;

    while game.active_time() < Duration::from_secs(90) {
        advance(&mut game, &mut now, Duration::from_millis(250));
    }
    assert!(game.outcome().is_none());

    advance(&mut game, &mut now, Duration::from_secs(1));
    let outcome = game.outcome().cloned().unwrap();
    assert!(!outcome.victory);
    assert_eq!(outcome.active_time, Duration::from_secs(90));

    advance(&mut game, &mut now, Duration::from_secs(1));
    assert_eq!(game.outcome(), Some(&outcome));
}

#[test]
fn crossing_half_health_pauses_time_then_enters_phase_two() {
    let mut now = Instant::now();
    let mut game = battle(now);
    finish_intro(&mut game, &mut now);
    game.health = game.max_health / 2 + 1;
    let before = game.active_time();

    type_current_prompt(&mut game);

    assert_eq!(
        game.cue().map(|(cue, _)| cue),
        Some(BattleCue::PhaseTransition)
    );
    assert_eq!(game.phase(), BossPhase::One);
    advance(&mut game, &mut now, Duration::from_millis(750));
    assert_eq!(game.active_time(), before);
    assert_eq!(game.phase(), BossPhase::Two);
}

#[test]
fn zero_health_publishes_one_victory_only_after_the_death_cinematic() {
    let mut now = Instant::now();
    let mut game = battle(now);
    finish_intro(&mut game, &mut now);
    game.health = 1;

    type_current_prompt(&mut game);

    assert_eq!(game.cue().map(|(cue, _)| cue), Some(BattleCue::Victory));
    assert!(game.outcome().is_none());
    advance(&mut game, &mut now, Duration::from_secs(1));
    let outcome = game.outcome().cloned().unwrap();
    assert!(outcome.victory);

    advance(&mut game, &mut now, Duration::from_secs(1));
    assert_eq!(game.outcome(), Some(&outcome));
}

#[test]
fn score_rounds_accuracy_to_the_nearest_basis_point() {
    let mut now = Instant::now();
    let mut game = battle(now);
    finish_intro(&mut game, &mut now);
    game.correct_units = 2;
    game.attempted_units = 3;

    game.start_finish(true);
    advance(&mut game, &mut now, Duration::from_secs(1));

    assert_eq!(game.outcome().unwrap().score, 28_667);
}

#[test]
fn thorn_queen_uses_the_first_unit_to_lock_one_of_two_vines() {
    let mut now = Instant::now();
    let mut game = queen(now, Language::En, &["apple", "berry", "cider", "daisy"]);
    finish_intro(&mut game, &mut now);

    assert_eq!(game.prompts().len(), 2);
    let mut prompts = game
        .prompts()
        .map(|prompt| (prompt.id(), prompt.text().to_owned()))
        .collect::<Vec<_>>();
    prompts.sort_by_key(|(id, _)| *id);
    assert_ne!(prompts[0].1.chars().next(), prompts[1].1.chars().next(),);

    let (target_id, target) = &prompts[1];
    let mut characters = target.chars();
    game.input_char(characters.next().unwrap());
    assert_eq!(game.target_id(), Some(*target_id));

    let health = game.health();
    for character in characters {
        game.input_char(character);
    }
    assert!(game.health() < health);
    assert_eq!(game.prompts().len(), 2);
}

#[test]
fn a_bloom_costs_one_heart_and_refills_the_vine_lane() {
    let mut now = Instant::now();
    let mut game = queen(now, Language::En, &["apple", "berry", "cider", "daisy"]);
    finish_intro(&mut game, &mut now);
    let remaining = game
        .prompts()
        .map(|prompt| prompt.remaining())
        .min()
        .unwrap();

    advance(&mut game, &mut now, remaining + Duration::from_millis(1));

    assert_eq!(game.hearts(), 2);
    assert_eq!(game.prompts().len(), 2);
    assert_eq!(game.cue().map(|(cue, _)| cue), Some(BattleCue::BossAttack));
}

#[test]
fn korean_vines_have_distinct_physical_first_keys() {
    let mut now = Instant::now();
    let mut game = queen(now, Language::Ko, &["가방", "나무", "다리", "마음"]);
    finish_intro(&mut game, &mut now);

    let initials = game
        .prompts()
        .map(|prompt| key_units(Language::Ko, prompt.text())[0])
        .collect::<HashSet<_>>();

    assert_eq!(game.prompts().len(), 2);
    assert_eq!(initials.len(), 2);
}

#[test]
fn thorn_queen_phase_two_adds_a_third_vine_after_the_transition() {
    let mut now = Instant::now();
    let mut game = queen(now, Language::En, &["apple", "berry", "cider", "daisy"]);
    finish_intro(&mut game, &mut now);
    game.health = game.max_health / 2 + 1;

    type_current_prompt(&mut game);
    assert_eq!(
        game.cue().map(|(cue, _)| cue),
        Some(BattleCue::PhaseTransition)
    );
    advance(&mut game, &mut now, Duration::from_millis(750));

    assert_eq!(game.phase(), BossPhase::Two);
    assert_eq!(game.prompts().len(), 3);
    assert!(matches!(
        game.pattern_view(),
        BossPatternView::Queen { target_id: None }
    ));
}

#[test]
fn null_archon_rolls_back_one_checksum_slot_on_a_wrong_key() {
    let mut now = Instant::now();
    let mut game = archon(now);
    finish_intro(&mut game, &mut now);
    type_current_prompt(&mut game);
    type_current_prompt(&mut game);
    assert!(matches!(
        game.pattern_view(),
        BossPatternView::NullArchon { checksum: 2, .. }
    ));

    game.input_char('#');

    assert!(game.input().is_empty());
    assert_eq!(game.combo(), 0);
    assert!(matches!(
        game.pattern_view(),
        BossPatternView::NullArchon { checksum: 1, .. }
    ));
    assert_eq!(game.cue().map(|(cue, _)| cue), Some(BattleCue::Hit));

    let first = game
        .prompts()
        .next()
        .unwrap()
        .text()
        .chars()
        .next()
        .unwrap();
    game.input_char(first);
    assert!(
        !game.input().is_empty(),
        "ordinary corruption must not lock input"
    );
}

#[test]
fn three_checksum_words_reverse_the_canticle_for_bonus_damage() {
    let mut now = Instant::now();
    let mut game = archon(now);
    finish_intro(&mut game, &mut now);
    let before = game.health();

    type_current_prompt(&mut game);
    type_current_prompt(&mut game);
    type_current_prompt(&mut game);

    assert!(matches!(
        game.pattern_view(),
        BossPatternView::NullArchon { checksum: 0, .. }
    ));
    assert!(
        before - game.health() > 15,
        "checksum must add bonus damage"
    );
}

#[test]
fn a_failed_canticle_costs_one_heart_and_locks_without_consuming_time() {
    let mut now = Instant::now();
    let mut game = archon(now);
    finish_intro(&mut game, &mut now);
    type_current_prompt(&mut game);

    for _ in 0..200 {
        advance(&mut game, &mut now, Duration::from_millis(250));
        if game.hearts() < 3 {
            break;
        }
    }

    assert_eq!(game.hearts(), 2);
    assert!(matches!(
        game.pattern_view(),
        BossPatternView::NullArchon { checksum: 0, .. }
    ));
    assert_eq!(game.cue().map(|(cue, _)| cue), Some(BattleCue::BossAttack));
    let before = game.active_time();
    advance(&mut game, &mut now, Duration::from_millis(600));
    assert_eq!(game.active_time(), before);
}

#[test]
fn null_archon_crosses_into_phase_two_after_a_time_freezing_cmax_transition() {
    let mut now = Instant::now();
    let mut game = archon(now);
    finish_intro(&mut game, &mut now);
    game.health = game.max_health / 2 + 1;
    let before = game.active_time();

    type_current_prompt(&mut game);

    assert_eq!(
        game.cue().map(|(cue, _)| cue),
        Some(BattleCue::PhaseTransition)
    );
    advance(&mut game, &mut now, Duration::from_millis(750));
    assert_eq!(game.active_time(), before);
    assert_eq!(game.phase(), BossPhase::Two);
}

#[test]
fn fixed_target_profiles_clear_and_the_prior_tier_does_not() {
    let catalog = ContentCatalog::load_builtins().unwrap();
    let profiles = [
        (GameDifficulty::Easy, 180_u32, 90.0, 70.0..=82.0, None),
        (
            GameDifficulty::Medium,
            300_u32,
            94.0,
            70.0..=84.0,
            Some((180_u32, 90.0)),
        ),
        (
            GameDifficulty::Hard,
            420_u32,
            97.0,
            74.0..=87.0,
            Some((300_u32, 94.0)),
        ),
    ];
    let mut failures = Vec::new();

    for boss in BossKind::ALL {
        for language in [Language::Ko, Language::En] {
            for (difficulty, kpm, accuracy, clear_range, prior) in &profiles {
                let outcome =
                    scripted_outcome(&catalog, boss, language, *difficulty, *kpm, *accuracy);
                eprintln!(
                    "{boss:?} {language:?} {difficulty:?}: victory={} time={:.3}s hearts={} units={} combo={} accuracy={:.1}%",
                    outcome.victory,
                    outcome.active_time.as_secs_f64(),
                    outcome.hearts,
                    outcome.correct_units,
                    outcome.max_combo,
                    outcome.correct_units as f64 * 100.0 / outcome.attempted_units.max(1) as f64,
                );
                if !outcome.victory || !clear_range.contains(&outcome.active_time.as_secs_f64()) {
                    failures.push(format!(
                        "{boss:?} {language:?} {difficulty:?}: victory={} time={:.3}s expected={clear_range:?}",
                        outcome.victory,
                        outcome.active_time.as_secs_f64(),
                    ));
                }

                if let Some((prior_kpm, prior_accuracy)) = prior {
                    let prior_outcome = scripted_outcome(
                        &catalog,
                        boss,
                        language,
                        *difficulty,
                        *prior_kpm,
                        *prior_accuracy,
                    );
                    eprintln!(
                        "{boss:?} {language:?} {difficulty:?} prior: victory={} time={:.3}s hearts={}",
                        prior_outcome.victory,
                        prior_outcome.active_time.as_secs_f64(),
                        prior_outcome.hearts,
                    );
                    let narrow_defeat = !prior_outcome.victory
                        && prior_outcome.active_time >= Duration::from_secs(75)
                        && (prior_outcome.active_time == BATTLE_LIMIT || prior_outcome.hearts == 0);
                    if !narrow_defeat {
                        failures.push(format!(
                            "{boss:?} {language:?} {difficulty:?}: prior tier victory={} time={:.3}s hearts={}",
                            prior_outcome.victory,
                            prior_outcome.active_time.as_secs_f64(),
                            prior_outcome.hearts,
                        ));
                    }
                }
            }
        }
    }

    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

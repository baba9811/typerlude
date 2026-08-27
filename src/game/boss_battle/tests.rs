use super::{BattleCue, BossBattle, BossKind, BossPatternView, BossPhase};
use crate::model::{Difficulty, Language};
use crate::typing::key_units;
use std::collections::HashSet;
use std::time::{Duration, Instant};

const INTRO: Duration = Duration::from_millis(800);

fn battle(now: Instant) -> BossBattle {
    BossBattle::new(
        BossKind::IronWarden,
        Language::En,
        Difficulty::Easy,
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

fn queen(now: Instant, language: Language, words: &[&str]) -> BossBattle {
    BossBattle::new(
        BossKind::ThornQueen,
        language,
        Difficulty::Easy,
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
        Difficulty::Easy,
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

use super::{BattleCue, BossBattle, BossKind, BossPatternView, BossPhase};
use crate::model::{Difficulty, Language};
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
        } = game.pattern_view();
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

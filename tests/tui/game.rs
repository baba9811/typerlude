use super::support::*;

fn open_games(app: &mut App, now: Instant) {
    press(app, Key::Tab, 6, now);
    app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!(app.screen(), Screen::Games);
}

fn open_game_options(app: &mut App, now: Instant) {
    open_games(app, now);
    app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!(app.screen(), Screen::GameOptions);
}

fn open_boss_options(app: &mut App, now: Instant) {
    open_games(app, now);
    app.handle_event(key(Key::Tab), now).unwrap();
    app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!(app.screen(), Screen::GameOptions);
}

fn start_boss(app: &mut App, boss_index: usize, now: Instant) {
    open_boss_options(app, now);
    press(app, Key::Down, boss_index, now);
    press(app, Key::Tab, 3, now);
    app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!(app.screen(), Screen::Game);
}

fn advance(app: &mut App, now: &mut Instant, duration: Duration) {
    let end = *now + duration;
    while *now < end {
        *now = (*now + Duration::from_millis(250)).min(end);
        app.tick(*now).unwrap();
    }
}

fn force_boss_victory(app: &mut App, now: &mut Instant) {
    for _ in 0..10_000 {
        if app.screen() == Screen::GameResult {
            return;
        }
        let output = buffer_text(&draw(app, 80, 24).buffer);
        let word = output.lines().find_map(|row| {
            row.split_once("Prompt:")
                .map(|(_, prompt)| prompt.trim_matches([' ', '│']).to_owned())
        });
        if let Some(word) = word {
            type_text(app, &word, *now);
        }
        advance(app, now, Duration::from_millis(250));
    }
    panic!("boss battle did not finish");
}

fn start_game(app: &mut App, now: Instant) {
    open_game_options(app, now);
    press(app, Key::Tab, 2, now);
    app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!(app.screen(), Screen::Game);
}

fn complete_visible_word(app: &mut App, now: Instant) {
    let output = buffer_text(&draw(app, 80, 24).buffer);
    let word = app
        .content
        .select(Language::En, ContentKind::Word, Difficulty::Medium)
        .into_iter()
        .find(|item| {
            output.lines().any(|row| {
                row.trim_matches(|character| character == '│' || character == ' ') == item.text
            })
        })
        .unwrap()
        .text
        .clone();
    type_text(app, &word, now);
}

fn finish_game(app: &mut App, now: Instant) {
    for step in 1..=100 {
        app.tick(now + Duration::from_millis(step * 250)).unwrap();
    }
    assert_eq!(app.screen(), Screen::GameResult);
}

#[test]
fn games_and_options_render_the_concrete_word_rain_choice() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let now = Instant::now();

    open_games(&mut app, now);
    let games = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(games.contains("Games"), "{games}");
    assert!(games.contains("> Word Rain"), "{games}");

    app.handle_event(key(Key::Enter), now).unwrap();
    let options = buffer_text(&draw(&app, 80, 24).buffer);
    for expected in ["Word Rain", "Language: en", "Difficulty: Medium", "Start"] {
        assert!(options.contains(expected), "{expected}: {options}");
    }
}

#[test]
fn word_rain_keeps_title_collision_stats_and_input_visible_at_80x24() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    start_game(&mut app, Instant::now());

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    let rows = output.lines().collect::<Vec<_>>();
    let title = rows
        .iter()
        .position(|row| row.contains("Word Rain"))
        .unwrap();
    let collision = rows
        .iter()
        .position(|row| row.contains("Miss line"))
        .unwrap();
    let stats = rows.iter().position(|row| {
        row.contains("Score: 0") && row.contains("Level: 1") && row.contains("Combo: 0")
    });
    let input = rows.iter().position(|row| row.contains("Input")).unwrap();

    assert!(title < collision, "{output}");
    assert!(collision < stats.unwrap(), "{output}");
    assert!(stats.unwrap() < input, "{output}");
    assert!(input < 24, "{output}");
}

#[test]
fn word_rain_anchors_the_unicode_input_cursor_in_the_input_dock() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let now = Instant::now();
    start_game(&mut app, now);

    let empty = draw(&app, 80, 24);
    assert_eq!(empty.buffer[(3, 20)].symbol(), " ");
    assert_eq!(empty.cursor, Some((3, 20)));

    app.handle_event(key(Key::Char('오')), now).unwrap();
    let entered = draw(&app, 80, 24);
    assert_eq!(entered.buffer[(3, 20)].symbol(), "오");
    assert_eq!(entered.cursor, Some((5, 20)));

    app.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!(draw(&app, 80, 24).cursor, None);
}

#[test]
fn the_complete_falling_word_is_clamped_inside_the_playfield() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    start_game(&mut app, Instant::now());

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    let visible = app
        .content
        .select(Language::En, ContentKind::Word, Difficulty::Medium)
        .iter()
        .any(|item| output.contains(&item.text));
    assert!(visible, "no complete selected word was visible: {output}");
}

#[test]
fn a_word_reaches_the_row_above_the_miss_line_before_game_over() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let now = Instant::now();
    start_game(&mut app, now);

    let initial = buffer_text(&draw(&app, 80, 24).buffer);
    let first_word = app
        .content
        .select(Language::En, ContentKind::Word, Difficulty::Medium)
        .into_iter()
        .find(|item| {
            initial.lines().any(|row| {
                row.trim_matches(|character| character == '│' || character == ' ') == item.text
            })
        })
        .unwrap()
        .text
        .clone();

    for step in 1..=55 {
        app.tick(now + Duration::from_millis(step * 250)).unwrap();
    }
    assert_eq!(app.screen(), Screen::Game);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    let rows = output.lines().collect::<Vec<_>>();
    let word_row = rows
        .iter()
        .position(|row| {
            row.trim_matches(|character| character == '│' || character == ' ') == first_word
        })
        .unwrap();
    let miss_line = rows
        .iter()
        .position(|row| row.contains("Miss line"))
        .unwrap();

    assert_eq!(word_row + 1, miss_line, "{output}");
}

#[test]
fn pause_overlay_keeps_the_board_and_leave_confirmation_visible() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let now = Instant::now();
    start_game(&mut app, now);

    app.handle_event(key(Key::Esc), now).unwrap();
    let paused = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(paused.contains("Word Rain"), "{paused}");
    assert!(paused.contains("Pause"), "{paused}");
    assert!(paused.contains("Esc"), "{paused}");

    app.handle_event(key(Key::Char('q')), now).unwrap();
    let confirm = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(
        confirm.contains("Press q again to leave the game"),
        "{confirm}"
    );
}

#[test]
fn game_result_shows_every_outcome_and_retry_action() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let now = Instant::now();
    start_game(&mut app, now);

    for step in 1..=56 {
        app.tick(now + Duration::from_millis(step * 250)).unwrap();
    }
    assert_eq!(app.screen(), Screen::GameResult);
    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for expected in [
        "Word Rain",
        "Game over",
        "Score",
        "Cleared words",
        "Max combo",
        "Level",
        "Duration",
        "Missed word",
        "Enter: Retry",
    ] {
        assert!(output.contains(expected), "{expected}: {output}");
    }
}

#[test]
fn updated_personal_best_fanfare_is_localized_and_uses_two_lines() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.settings.ui_language = Language::Ko;
    let now = Instant::now();
    start_game(&mut app, now);
    complete_visible_word(&mut app, now);
    finish_game(&mut app, now);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    let rows = output.lines().collect::<Vec<_>>();
    let update = rows
        .iter()
        .position(|row| row.contains("개인 최고 기록 갱신!"))
        .unwrap();
    assert!(rows[update + 1].contains("0 -> "), "{output}");
}

#[test]
fn unbeaten_personal_best_is_grouped_without_a_fanfare() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.settings.word_rain_high_scores[1][1] = 1_000_000;
    let now = Instant::now();
    start_game(&mut app, now);
    finish_game(&mut app, now);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("Personal best: 1,000,000"), "{output}");
    assert!(!output.contains("Personal best updated!"), "{output}");
}

#[test]
fn warning_footer_does_not_clip_the_game_input_dock() {
    let (_root, mut app) = fixture_app();
    start_game(&mut app, Instant::now());

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("Input"), "{output}");
    assert!(output.contains("review warning"), "{output}");
}

#[test]
fn boss_select_uses_roster_preview_stars_and_sequential_locks_at_80x24() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    open_boss_options(&mut app, Instant::now());

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for expected in [
        "IRON WARDEN",
        "THORN QUEEN",
        "NULL ARCHON",
        "☆☆☆",
        "Language: en",
        "Easy",
        "LOCKED",
        "Enter/Tab Options",
    ] {
        assert!(output.contains(expected), "{expected}: {output}");
    }
}

#[test]
fn boss_select_keeps_the_exact_unlock_requirement_above_a_warning_footer() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();
    open_boss_options(&mut app, now);
    app.handle_event(key(Key::Down), now).unwrap();

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("Clear IRON WARDEN Easy"), "{output}");
    assert!(output.contains("review warning"), "{output}");
}

#[test]
fn iron_warden_battle_keeps_art_pattern_status_prompt_and_input_visible() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let mut now = Instant::now();
    start_boss(&mut app, 0, now);

    let intro = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(intro.contains("Prompt: —"), "{intro}");

    advance(&mut app, &mut now, Duration::from_millis(800));

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for expected in [
        "▄████████▄",
        "PILE DRIVER",
        "◇ ◇ ◇",
        "HP",
        "01:30",
        "♥♥♥",
        "Prompt:",
        "Input",
    ] {
        assert!(output.contains(expected), "{expected}: {output}");
    }
}

#[test]
fn large_boss_battle_keeps_prompt_and_typed_input_with_the_boss() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.settings.ui_language = Language::Ko;
    let mut now = Instant::now();
    start_boss(&mut app, 0, now);
    advance(&mut app, &mut now, Duration::from_millis(800));

    let drawn = draw(&app, 190, 50);
    let output = buffer_text(&drawn.buffer);
    let rows = output.lines().collect::<Vec<_>>();
    let boss = rows
        .iter()
        .position(|row| row.contains("▄████████▄"))
        .unwrap();
    let prompt = rows.iter().position(|row| row.contains("Prompt:")).unwrap();
    let cursor = drawn.cursor.unwrap();

    assert!(output.contains("Enter: 제출"), "{output}");
    assert!(prompt > boss && prompt - boss <= 18, "{output}");
    assert!(cursor.0 > 190 / 4 && cursor.0 < 190 * 3 / 4, "{output}");
    assert_eq!(usize::from(cursor.1), prompt + 1, "{output}");

    app.handle_event(key(Key::Char('#')), now).unwrap();
    let typed = draw(&app, 190, 50);
    assert!(buffer_text(&typed.buffer).contains("> #"));
    assert_eq!(typed.cursor, Some((cursor.0 + 1, cursor.1)));
}

#[test]
fn thorn_queen_battle_shows_crown_and_parallel_vine_lanes() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.settings.boss_battle_progress[0].clear_rank = 1;
    let mut now = Instant::now();
    start_boss(&mut app, 1, now);
    advance(&mut app, &mut now, Duration::from_millis(800));

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for expected in [
        "✦",
        "VINE 1",
        "VINE 2",
        "TARGET",
        "HP",
        "01:30",
        "♥♥♥",
        "Prompt:",
    ] {
        assert!(output.contains(expected), "{expected}: {output}");
    }
}

#[test]
fn null_archon_has_stable_checksum_ui_and_full_cmax_system_lock() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.settings.boss_battle_progress[0].clear_rank = 1;
    app.settings.boss_battle_progress[1].clear_rank = 1;
    let mut now = Instant::now();
    start_boss(&mut app, 2, now);

    let locked = buffer_text(&draw(&app, 80, 24).buffer);
    for expected in ["SYSTEM LOCK", "VOID_CANTICLE", "NULL", "ERROR"] {
        assert!(locked.contains(expected), "{expected}: {locked}");
    }

    app.handle_event(key(Key::Esc), now).unwrap();
    let paused_locked = buffer_text(&draw(&app, 80, 24).buffer);
    for expected in ["SYSTEM LOCK", "FRAME", "Pause"] {
        assert!(
            paused_locked.contains(expected),
            "{expected}: {paused_locked}"
        );
    }
    assert!(!paused_locked.contains("Prompt:"), "{paused_locked}");
    app.handle_event(key(Key::Esc), now).unwrap();

    advance(&mut app, &mut now, Duration::from_millis(800));
    let stable_frame = draw(&app, 80, 24);
    let stable = buffer_text(&stable_frame.buffer);
    for expected in [
        "/ERROR\\",
        "NULL://ACTIVE",
        "C MAX // STANDBY",
        "Checksum",
        "□ □ □",
        "VOID_CANTICLE",
        "Prompt:",
        "Input",
    ] {
        assert!(stable.contains(expected), "{expected}: {stable}");
    }
    let rows = stable.lines().collect::<Vec<_>>();
    let art = rows.iter().position(|row| row.contains("C C C")).unwrap();
    let checksum = rows
        .iter()
        .position(|row| row.contains("Checksum"))
        .unwrap();
    let input = rows.iter().position(|row| row.contains("Prompt:")).unwrap();
    assert!(art < checksum && checksum < input, "{stable}");

    let trace_before = rows
        .iter()
        .find(|row| row.contains("TRACE::"))
        .unwrap()
        .to_string();
    let stable_archon = stable_frame
        .buffer
        .content
        .iter()
        .position(|cell| cell.symbol() == "{")
        .unwrap();
    let stable_cmax = (0..80)
        .find(|x| stable_frame.buffer[(*x, art as u16)].symbol() == "C")
        .unwrap();
    assert_role_style(
        &stable_frame.buffer.content[stable_archon],
        default_styles().base,
    );

    advance(&mut app, &mut now, Duration::from_millis(250));
    let animated = buffer_text(&draw(&app, 80, 24).buffer);
    let trace_after = animated
        .lines()
        .find(|row| row.contains("TRACE::"))
        .unwrap();
    assert_ne!(trace_before, trace_after, "{animated}");

    app.handle_event(key(Key::Char('#')), now).unwrap();
    let hit = draw(&app, 80, 24);
    let hit_archon = hit
        .buffer
        .content
        .iter()
        .position(|cell| cell.symbol() == "{")
        .unwrap();
    let hit_cmax = (0..80)
        .find(|x| hit.buffer[(*x, art as u16)].symbol() == "C")
        .unwrap();
    assert_ne!(stable_cmax, hit_cmax);
    assert_role_style(&hit.buffer.content[hit_archon], default_styles().error);
}

#[test]
fn null_archon_overlaps_and_animates_its_cmax_glitch_layers() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.settings.boss_battle_progress[0].clear_rank = 1;
    app.settings.boss_battle_progress[1].clear_rank = 1;
    let mut now = Instant::now();
    start_boss(&mut app, 2, now);
    advance(&mut app, &mut now, Duration::from_millis(800));

    let first = draw(&app, 80, 24);
    let output = buffer_text(&first.buffer);
    let archon = first
        .buffer
        .content
        .iter()
        .position(|cell| cell.symbol() == "{")
        .unwrap();
    let archon_x = archon as u16 % 80;
    let archon_y = archon as u16 / 80;
    let cmax = (0..80)
        .filter(|x| matches!(first.buffer[(*x, archon_y)].symbol(), "C" | "M" | "A" | "X"))
        .collect::<Vec<_>>();

    for symbol in ["C", "M", "A", "X"] {
        assert!(
            (0..80).any(|x| first.buffer[(x, archon_y)].symbol() == symbol),
            "missing {symbol}: {output}"
        );
    }
    assert!(
        cmax.first().is_some_and(|x| *x < archon_x) && cmax.last().is_some_and(|x| *x > archon_x),
        "{output}"
    );

    advance(&mut app, &mut now, Duration::from_millis(250));
    let animated = draw(&app, 80, 24);
    let animated_archon = animated
        .buffer
        .content
        .iter()
        .position(|cell| cell.symbol() == "{")
        .unwrap() as u16;
    assert_ne!(archon_x, animated_archon % 80, "{output}");
}

#[test]
fn boss_victory_result_shows_progress_metrics_unlocks_and_actions() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let mut now = Instant::now();
    start_boss(&mut app, 0, now);
    force_boss_victory(&mut app, &mut now);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for expected in [
        "Victory",
        "☆☆☆ -> ★☆☆",
        "Boss unlocked",
        "Score",
        "KPM",
        "Accuracy",
        "Max combo",
        "Time",
        "Enter: Retry",
        "Esc: Boss select",
    ] {
        assert!(output.contains(expected), "{expected}: {output}");
    }
    assert!(output.lines().count() <= 24, "{output}");

    app.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!(app.screen(), Screen::GameOptions);
    let boss_select = buffer_text(&draw(&app, 80, 24).buffer);
    for expected in ["BOSSES", "IRON WARDEN", "☆☆☆"] {
        assert!(boss_select.contains(expected), "{expected}: {boss_select}");
    }
}

#[test]
fn leaving_a_paused_boss_fight_returns_to_boss_selection() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let now = Instant::now();
    start_boss(&mut app, 0, now);

    app.handle_event(key(Key::Esc), now).unwrap();
    app.handle_event(key(Key::Char('q')), now).unwrap();
    app.handle_event(key(Key::Char('q')), now).unwrap();

    assert_eq!(app.screen(), Screen::GameOptions);
    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("BOSSES"), "{output}");
}

#[test]
fn boss_defeat_keeps_progress_locked() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let mut now = Instant::now();
    start_boss(&mut app, 0, now);
    advance(&mut app, &mut now, Duration::from_secs(50));

    assert_eq!(app.screen(), Screen::GameResult);
    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("Defeat"), "{output}");
    assert!(output.contains("☆☆☆ -> ☆☆☆"), "{output}");
    assert!(!output.contains("unlocked:"), "{output}");
}

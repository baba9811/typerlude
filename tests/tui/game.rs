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

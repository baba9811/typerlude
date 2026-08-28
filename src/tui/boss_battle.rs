use super::{
    format::{grouped_u64, language_name},
    game::{centered, difficulty_name},
    theme::ThemeStyles,
    titled,
};
use crate::{
    app::App,
    game::{
        GameDifficulty,
        boss_battle::{BattleCue, BossBattle, BossKind, BossPatternView, BossPhase},
    },
    i18n::{TextKey, text},
    model::Language,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

const IRON_WARDEN_ART: &[&str] = &[
    "          ▄████████▄          ",
    "      ▄██▀    ██    ▀██▄      ",
    "     ███   ▄██████▄   ███     ",
    "   ▄██████▄███ ◈ ███▄██████▄   ",
    "  ███▀ ▀██████████████▀ ▀███  ",
    "  ▀▀     ▀██  ██▀     ▀▀     ",
    "         ▄██  ██▄             ",
];

const THORN_QUEEN_ART: &[&str] = &[
    "             ✦             ",
    "         ╭───┼───╮         ",
    "      ╭──╯  ╲│╱  ╰──╮      ",
    "     ╱      ─◇─      ╲     ",
    "    ╱   ╭────┼────╮   ╲    ",
    "   ╱   ╱     │     ╲   ╲   ",
    "  ╵   ╵      │      ╵   ╵  ",
];

const NULL_ARCHON_ART: &[&str] = &[
    "          /ERROR\\          ",
    "    NULL   \\ 00 /   NULL    ",
    "   ERR  --  { }  --  ERR   ",
    "    VOID   / 10 \\   VOID    ",
    "          \\FAIL/           ",
];

const CMAX_ART: &[&str] = &[
    "C C C       M   M    AAAAA   X   X",
    "C           MM MM    A   A    X X",
    "C           M M M    AAAAA     X",
    "C           M   M    A   A    X X",
    "C C C       M   M    A   A   X   X",
];

pub(super) fn render_boss_options(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    styles: ThemeStyles,
) {
    let language = app.settings.ui_language;
    let options = app.game_options();
    let outer = titled(text(language, TextKey::BossBattle), styles);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let columns = Layout::horizontal([Constraint::Length(26), Constraint::Min(1)]).split(inner);

    let roster = titled("BOSSES", styles);
    let roster_inner = roster.inner(columns[0]);
    frame.render_widget(roster, columns[0]);
    let mut roster_lines = Vec::new();
    for (index, boss) in BossKind::ALL.into_iter().enumerate() {
        let unlocked = app.settings.boss_is_unlocked(boss);
        roster_lines.push(Line::from(vec![
            Span::styled(
                if app.focus() == index { "> " } else { "  " },
                styles.accent,
            ),
            Span::styled(
                if unlocked { "◆ " } else { "× " },
                if unlocked {
                    styles.correct
                } else {
                    styles.error
                },
            ),
            Span::styled(
                boss_name(language, boss),
                if unlocked { styles.base } else { styles.dim },
            ),
        ]));
        roster_lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(stars(app.settings.boss_clear_rank(boss)), styles.accent),
            Span::styled(if unlocked { "" } else { "  LOCKED" }, styles.error),
        ]));
    }
    frame.render_widget(
        Paragraph::new(roster_lines).style(styles.base),
        roster_inner,
    );

    let preview = titled(boss_name(language, options.boss), styles);
    let preview_inner = preview.inner(columns[1]);
    frame.render_widget(preview, columns[1]);
    let regions = Layout::vertical([
        Constraint::Length(7),
        Constraint::Length(2),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .split(preview_inner);
    render_static_art(frame, boss_art(options.boss), regions[0], styles.base);
    frame.render_widget(
        Paragraph::new(mechanic_summary(language, options.boss))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .style(styles.dim),
        regions[1],
    );

    let unlocked = app.settings.boss_is_unlocked(options.boss);
    let difficulty_unlocked = app
        .settings
        .boss_difficulty_is_unlocked(options.boss, options.difficulty);
    let best = if difficulty_unlocked {
        app.settings
            .boss_high_score(options.boss, options.language, options.difficulty)
    } else {
        0
    };
    let marker = |focus| if app.focus() == focus { "> " } else { "  " };
    let clear_requirement = |boss, difficulty| match language {
        Language::Ko => format!(
            "{} {} 클리어 필요",
            boss_name(language, boss),
            difficulty_name(language, difficulty)
        ),
        Language::En => format!(
            "Clear {} {}",
            boss_name(language, boss),
            difficulty_name(language, difficulty)
        ),
    };
    let requirement = if !unlocked {
        options
            .boss
            .index()
            .checked_sub(1)
            .and_then(|index| BossKind::ALL.get(index).copied())
            .map(|boss| clear_requirement(boss, GameDifficulty::Easy))
            .or_else(|| Some(text(language, TextKey::BossLocked).to_owned()))
    } else if !difficulty_unlocked {
        match options.difficulty {
            GameDifficulty::Medium => Some(clear_requirement(options.boss, GameDifficulty::Easy)),
            GameDifficulty::Hard => Some(clear_requirement(options.boss, GameDifficulty::Medium)),
            GameDifficulty::Hell => Some(clear_requirement(options.boss, GameDifficulty::Hard)),
            GameDifficulty::Easy => Some(text(language, TextKey::DifficultyLocked).to_owned()),
        }
    } else {
        options.error.clone()
    };
    let available = GameDifficulty::ALL
        .into_iter()
        .map(|difficulty| {
            let name = difficulty_name(language, difficulty);
            if app
                .settings
                .boss_difficulty_is_unlocked(options.boss, difficulty)
            {
                if difficulty == options.difficulty {
                    format!("[{name}]")
                } else {
                    name.to_owned()
                }
            } else {
                format!("{name}×")
            }
        })
        .collect::<Vec<_>>()
        .join(" · ");
    let mut start = vec![
        Span::styled(marker(5), styles.accent),
        Span::styled(
            text(language, TextKey::Start),
            if unlocked && difficulty_unlocked {
                styles.correct.add_modifier(Modifier::BOLD)
            } else {
                styles.dim
            },
        ),
    ];
    if let Some(requirement) = requirement {
        start.push(Span::styled(" · ", styles.dim));
        start.push(Span::styled(requirement, styles.error));
    }
    let option_lines = vec![
        Line::from(format!(
            "{}{}: {}",
            marker(3),
            text(language, TextKey::Language),
            language_name(options.language)
        )),
        Line::from(format!(
            "{}{}: {}",
            marker(4),
            text(language, TextKey::Difficulty),
            difficulty_name(language, options.difficulty)
        )),
        Line::from(format!("    {available}")),
        Line::from(format!(
            "  {}: {}",
            text(language, TextKey::Best),
            grouped_u64(best)
        )),
        Line::from(start),
    ];
    frame.render_widget(Paragraph::new(option_lines).style(styles.base), regions[2]);
    let help = match (language, app.focus() < BossKind::ALL.len()) {
        (Language::Ko, true) => "↑↓ 보스 · Enter/Tab 옵션 · Esc",
        (Language::En, true) => "↑↓ Boss · Enter/Tab Options · Esc",
        (Language::Ko, false) => "Tab 이동 · ←→ 변경 · Enter 선택 · Esc",
        (Language::En, false) => "Tab Fields · ←→ Change · Enter Select · Esc",
    };
    frame.render_widget(Paragraph::new(help).style(styles.dim), regions[3]);
}

pub(super) fn render_boss_battle(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    styles: ThemeStyles,
) {
    let Some(active) = app.active_boss_battle() else {
        return;
    };
    let game = &active.game;
    let language = app.settings.ui_language;
    let locking_cue = game.cue().filter(|(cue, _)| cue_locks(*cue));
    if game.boss() == BossKind::NullArchon
        && let Some((cue, progress)) = locking_cue
    {
        render_cmax(frame, language, cue, progress, area, styles);
        if game.is_paused() {
            let pause_height = area.height.min(5);
            let pause_area = Rect::new(
                area.x,
                area.bottom().saturating_sub(pause_height),
                area.width,
                pause_height,
            );
            render_pause(
                frame,
                language,
                active.leave_confirmation,
                pause_area,
                styles,
            );
        }
        return;
    }

    let title = format!(
        "{} · {}",
        boss_name(language, game.boss()),
        difficulty_name(language, game.difficulty())
    );
    let outer = titled(&title, styles);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let battle = centered(inner, 96, 18);
    let regions = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(8),
        Constraint::Length(5),
    ])
    .split(battle);

    let remaining = game.time_remaining().as_secs_f64().ceil() as u64;
    let phase = match game.phase() {
        BossPhase::One => 1,
        BossPhase::Two => 2,
    };
    frame.render_widget(
        Paragraph::new(format!(
            "HP [{}] {}/{}  {:02}:{:02}  {}  {} {}  {} {}",
            gauge(
                1.0 - game.health() as f64 / game.max_health().max(1) as f64,
                16,
                true
            ),
            game.health(),
            game.max_health(),
            remaining / 60,
            remaining % 60,
            hearts(game.hearts()),
            text(language, TextKey::Phase),
            phase,
            text(language, TextKey::Combo),
            game.combo(),
        ))
        .alignment(Alignment::Center)
        .style(styles.base),
        regions[0],
    );

    match game.boss() {
        BossKind::IronWarden => render_warden(frame, game, regions[1], styles),
        BossKind::ThornQueen => render_queen(frame, game, regions[1], styles),
        BossKind::NullArchon => render_archon(frame, game, regions[1], styles),
    }
    let input_area = centered(regions[2], 56, regions[2].height);
    render_input(
        frame,
        game,
        language,
        input_area,
        styles,
        locking_cue.is_some(),
    );

    if game.is_paused() {
        render_pause(
            frame,
            language,
            active.leave_confirmation,
            regions[1],
            styles,
        );
    }
}

fn render_pause(
    frame: &mut Frame<'_>,
    language: Language,
    leave_confirmation: bool,
    area: Rect,
    styles: ThemeStyles,
) {
    let overlay = centered(area, 54, 5);
    let message = if leave_confirmation {
        text(language, TextKey::LeaveGameConfirm)
    } else {
        match language {
            Language::Ko => "Esc: 계속 · q: 나가기",
            Language::En => "Esc: Resume · q: Leave",
        }
    };
    frame.render_widget(
        Paragraph::new(message)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .style(styles.base)
            .block(titled(text(language, TextKey::Pause), styles)),
        overlay,
    );
}

pub(super) fn render_boss_result(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    styles: ThemeStyles,
) {
    let Some(result) = app.boss_battle_result() else {
        return;
    };
    let language = app.settings.ui_language;
    let outcome = &result.outcome;
    let outer = titled(boss_name(language, outcome.boss), styles);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let mut lines = vec![
        Line::from(Span::styled(
            text(
                language,
                if outcome.victory {
                    TextKey::Victory
                } else {
                    TextKey::Defeat
                },
            ),
            if outcome.victory {
                styles.correct.add_modifier(Modifier::BOLD)
            } else {
                styles.error
            },
        )),
        Line::from(format!(
            "{} -> {}",
            stars(result.previous_rank),
            stars(result.new_rank)
        )),
    ];
    if outcome.victory && outcome.score > result.previous_best {
        lines.push(Line::from(Span::styled(
            text(language, TextKey::PersonalBestUpdated),
            styles.accent,
        )));
    }
    if outcome.victory && result.new_rank > result.previous_rank {
        if let Some(difficulty) = unlocked_difficulty(result.new_rank) {
            lines.push(Line::from(format!(
                "{}: {}",
                text(language, TextKey::DifficultyUnlocked),
                difficulty_name(language, difficulty)
            )));
        }
        if result.previous_rank == 0
            && let Some(next) = BossKind::ALL.get(outcome.boss.index() + 1).copied()
        {
            lines.push(Line::from(format!(
                "{}: {}",
                text(language, TextKey::BossUnlocked),
                boss_name(language, next)
            )));
        }
    }
    let seconds = outcome.active_time.as_secs_f64();
    let kpm = if seconds == 0.0 {
        0.0
    } else {
        outcome.correct_units as f64 * 60.0 / seconds
    };
    let accuracy = if outcome.attempted_units == 0 {
        100.0
    } else {
        outcome.correct_units as f64 * 100.0 / outcome.attempted_units as f64
    };
    lines.extend([
        Line::from(""),
        Line::from(format!(
            "{}: {}",
            text(language, TextKey::Score),
            grouped_u64(outcome.score)
        )),
        Line::from(format!("KPM: {kpm:.1}")),
        Line::from(format!(
            "{}: {accuracy:.1}%",
            text(language, TextKey::Accuracy)
        )),
        Line::from(format!(
            "{}: {}",
            text(language, TextKey::MaxCombo),
            outcome.max_combo
        )),
        Line::from(format!(
            "{}: {:.1}s",
            text(language, TextKey::Time),
            seconds
        )),
        Line::from(format!(
            "{}: {}",
            text(language, TextKey::Hearts),
            hearts(outcome.hearts)
        )),
        Line::from(""),
        Line::from(match language {
            Language::Ko => "Enter: 다시 하기 · Esc: 보스 선택",
            Language::En => "Enter: Retry · Esc: Boss select",
        }),
    ]);
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(styles.base),
        inner,
    );
}

fn render_warden(frame: &mut Frame<'_>, game: &BossBattle, area: Rect, styles: ThemeStyles) {
    let regions = Layout::vertical([Constraint::Length(7), Constraint::Min(3)]).split(area);
    let cue = game.cue();
    let hit = cue
        .filter(|(cue, _)| *cue == BattleCue::Hit)
        .map(|(_, progress)| progress);
    let attack = cue
        .filter(|(cue, _)| *cue == BattleCue::BossAttack)
        .map(|(_, progress)| progress);
    let victory = cue
        .filter(|(cue, _)| *cue == BattleCue::Victory)
        .map(|(_, progress)| progress);
    let motion = hit.or(attack).or(victory);
    let mut art_area = shifted(
        regions[0],
        u16::from(motion.is_some_and(|progress| progress < 0.55)),
    );
    if victory.is_some_and(|progress| progress > 0.45) {
        art_area.y = art_area.y.saturating_add(1);
        art_area.height = art_area.height.saturating_sub(1);
    }
    render_motion_art(frame, IRON_WARDEN_ART, art_area, styles, motion, 3);

    let BossPatternView::Warden {
        locks,
        core_exposed,
        cast_progress,
    } = game.pattern_view()
    else {
        return;
    };
    let lock_line = (0..3)
        .map(|index| if index < locks { '◆' } else { '◇' })
        .map(|lock| lock.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let lines = vec![
        Line::from(vec![
            Span::styled("ARMOR ", styles.dim),
            Span::styled(lock_line, styles.accent),
            Span::styled(
                if core_exposed {
                    "  CORE EXPOSED"
                } else {
                    "  CORE SEALED"
                },
                if core_exposed {
                    styles.correct
                } else {
                    styles.dim
                },
            ),
        ]),
        Line::from(Span::styled(
            format!(
                "PILE DRIVER [{}]{}",
                gauge(cast_progress, 18, false),
                if attack.is_some() { "  // IMPACT" } else { "" }
            ),
            if attack.is_some() {
                styles.error
            } else {
                styles.base
            },
        )),
        Line::from(Span::styled(
            if victory.is_some() {
                "CORE BREACH // WARDEN COLLAPSE"
            } else if hit.is_some() {
                "✦  ✧  IMPACT  ✧  ✦"
            } else if attack.is_some() {
                "HAMMER DESCENT // PILE DRIVER"
            } else {
                "BREAK 3 LOCKS // STRIKE THE CORE"
            },
            if motion.is_some() {
                styles.accent
            } else {
                styles.dim
            },
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(styles.base),
        regions[1],
    );
}

fn render_queen(frame: &mut Frame<'_>, game: &BossBattle, area: Rect, styles: ThemeStyles) {
    let regions = Layout::vertical([Constraint::Length(7), Constraint::Min(4)]).split(area);
    let cue = game.cue();
    let hit = cue
        .filter(|(cue, _)| *cue == BattleCue::Hit)
        .map(|(_, progress)| progress);
    let attack = cue
        .filter(|(cue, _)| *cue == BattleCue::BossAttack)
        .map(|(_, progress)| progress);
    let victory = cue
        .filter(|(cue, _)| *cue == BattleCue::Victory)
        .map(|(_, progress)| progress);
    let motion = hit.or(attack).or(victory);
    let art = if hit.is_some() || victory.is_some() {
        &THORN_QUEEN_ART[..6]
    } else {
        THORN_QUEEN_ART
    };
    render_motion_art(frame, art, regions[0], styles, motion, 4);

    let target = game.target_id();
    let mut lines = vec![Line::from(Span::styled(
        if target.is_some() {
            "TARGET: LOCKED"
        } else {
            "TARGET: —"
        },
        if target.is_some() {
            styles.accent
        } else {
            styles.dim
        },
    ))];
    for (index, prompt) in game.prompts().enumerate() {
        let selected = target == Some(prompt.id());
        lines.push(Line::from(vec![
            Span::styled(
                if selected { "▶ " } else { "○ " },
                if selected { styles.accent } else { styles.dim },
            ),
            Span::styled(format!("VINE {}  ", index + 1), styles.base),
            Span::styled(
                format!("{:<14}", prompt.text()),
                if selected {
                    styles.correct
                } else {
                    styles.base
                },
            ),
            Span::styled(
                format!("[{}]", gauge(prompt.progress(), 10, true)),
                styles.dim,
            ),
        ]));
    }
    if victory.is_some() {
        lines.push(Line::from(Span::styled(
            "CROWN SEVERED // THORNS WITHER",
            styles.correct,
        )));
    } else if hit.is_some() {
        lines.push(Line::from(Span::styled(
            "❧  ❧  CUT  ❧  ❧  ❧",
            styles.correct,
        )));
    } else if attack.is_some() {
        lines.push(Line::from(Span::styled("THORN BLOOM", styles.error)));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(styles.base),
        regions[1],
    );
}

fn render_archon(frame: &mut Frame<'_>, game: &BossBattle, area: Rect, styles: ThemeStyles) {
    let block = titled("C MAX // STANDBY", styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let regions = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(5),
        Constraint::Min(4),
    ])
    .split(inner);
    let hit = game
        .cue()
        .filter(|(cue, _)| *cue == BattleCue::Hit)
        .map(|(_, progress)| progress);
    let trace = (game.time_remaining().as_millis() / 125) % 256;
    frame.render_widget(
        Paragraph::new(format!(
            "NULL://ACTIVE  //  SIGNAL 00:10  //  TRACE::{trace:03}"
        ))
        .alignment(Alignment::Center)
        .style(styles.dim),
        regions[0],
    );
    let visual_area = centered(regions[1], 48, 5);
    let glitch = u16::from((trace / 2).is_multiple_of(2));
    let mut cmax_area = visual_area;
    cmax_area.x = cmax_area.x.saturating_add(glitch);
    render_motion_art(frame, CMAX_ART, cmax_area, styles, hit, 2);
    let mut archon_area = centered(visual_area, 29, 5);
    archon_area.x = archon_area
        .x
        .saturating_add(1 - glitch + u16::from(hit.is_some_and(|progress| progress < 0.55)) * 2);
    render_ascii_overlay(frame, NULL_ARCHON_ART, archon_area, styles, hit, 2);
    let BossPatternView::NullArchon {
        checksum,
        canticle_progress,
    } = game.pattern_view()
    else {
        return;
    };
    let slots = (0..3)
        .map(|index| if index < checksum { '■' } else { '□' })
        .map(|slot| slot.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let mut lines = vec![
        Line::from(format!("Checksum  [{slots}]")),
        Line::from(format!(
            "VOID_CANTICLE [{}]",
            gauge(canticle_progress, 20, false)
        )),
        Line::from(Span::styled("ERR 00 ── NULL ── ERR 10 ── VOID", styles.dim)),
    ];
    if hit.is_some() {
        lines.push(Line::from(Span::styled(
            "ERROR      NULL      ERROR",
            styles.error,
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "REVERSE 3 CHECKSUMS // BREAK THE VOID",
            styles.dim,
        )));
    }
    let status_area = centered(regions[2], regions[2].width, 4);
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(styles.base),
        status_area,
    );
}

fn render_input(
    frame: &mut Frame<'_>,
    game: &BossBattle,
    language: Language,
    area: Rect,
    styles: ThemeStyles,
    locked: bool,
) {
    let block = titled(text(language, TextKey::Input), styles);
    let inner = block.inner(area);
    let target = if locked {
        None
    } else {
        game.target_id()
            .and_then(|id| game.prompts().find(|prompt| prompt.id() == id))
            .or_else(|| game.prompts().next())
    };
    let prompt = target.map_or("—", |prompt| prompt.text());
    let entered = if locked { "" } else { game.input() };
    let invalid = !entered.is_empty() && !game.input_is_valid();
    let lines = vec![
        Line::from(format!("Prompt: {prompt}")),
        Line::from(vec![
            Span::styled("> ", styles.accent),
            Span::styled(entered, if invalid { styles.error } else { styles.base }),
            Span::styled(
                if locked {
                    format!("  // {}", text(language, TextKey::InputLocked))
                } else if invalid {
                    format!("  ! {}", text(language, TextKey::CorrectionNeeded))
                } else {
                    String::new()
                },
                if invalid { styles.error } else { styles.dim },
            ),
        ]),
        Line::from(match language {
            Language::Ko => "Enter: 제출 · Esc: 일시 정지 · Backspace: 수정",
            Language::En => "Enter: Submit · Esc: Pause · Backspace: Correct",
        }),
    ];
    frame.render_widget(Paragraph::new(lines).style(styles.base).block(block), area);
    if !game.is_paused() && !locked && inner.width > 2 && inner.height > 1 {
        let entered_width = UnicodeWidthStr::width(game.input())
            .min(usize::from(inner.width.saturating_sub(3))) as u16;
        frame.set_cursor_position((inner.x + 2 + entered_width, inner.y + 1));
    }
}

fn render_cmax(
    frame: &mut Frame<'_>,
    language: Language,
    cue: BattleCue,
    progress: f64,
    area: Rect,
    styles: ThemeStyles,
) {
    let block = titled("C MAX // NULL ARCHON", styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let alternate = ((progress * 8.0) as u8).is_multiple_of(2);
    let shift = if alternate { "" } else { "     " };
    let cue = match cue {
        BattleCue::Intro => "BOOT_SEQUENCE",
        BattleCue::PhaseTransition => "PHASE_OVERRIDE",
        BattleCue::BossAttack => "CANTICLE_FAILURE",
        BattleCue::Victory => "NULL_COLLAPSE",
        BattleCue::Defeat => "HOST_TERMINATED",
        BattleCue::Hit => "CORRUPTION",
    };
    let lines = vec![
        Line::from(Span::styled(
            format!("{shift}NULL // ERROR // NULL"),
            styles.error,
        )),
        Line::from(""),
        Line::from(Span::styled(format!("VOID_CANTICLE::{cue}"), styles.accent)),
        Line::from(Span::styled("ERR 00  ERR 10  ERR 00", styles.dim)),
        Line::from(""),
        Line::from(Span::styled(CMAX_ART[0], styles.base)),
        Line::from(Span::styled(CMAX_ART[1], styles.base)),
        Line::from(Span::styled(CMAX_ART[2], styles.accent)),
        Line::from(Span::styled(CMAX_ART[3], styles.base)),
        Line::from(Span::styled(CMAX_ART[4], styles.base)),
        Line::from(""),
        Line::from(Span::styled(
            match language {
                Language::Ko => "SYSTEM LOCK · 입력 잠김",
                Language::En => "SYSTEM LOCK · INPUT LOCKED",
            },
            styles.error.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "NULL        ERROR        VOID_CANTICLE",
            styles.dim,
        )),
        Line::from(Span::styled(
            format!("FRAME {:03} // TIMER HALTED", (progress * 100.0) as u8),
            styles.accent,
        )),
        Line::from(""),
        Line::from(Span::styled("ERROR // NULL // ERROR // NULL", styles.error)),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(styles.base),
        inner,
    );
}

fn render_static_art(frame: &mut Frame<'_>, art: &[&str], area: Rect, style: Style) {
    frame.render_widget(
        Paragraph::new(
            art.iter()
                .map(|line| Line::from(Span::styled((*line).to_owned(), style)))
                .collect::<Vec<_>>(),
        )
        .alignment(Alignment::Center),
        area,
    );
}

fn render_motion_art(
    frame: &mut Frame<'_>,
    art: &[&str],
    area: Rect,
    styles: ThemeStyles,
    hit: Option<f64>,
    impact_row: usize,
) {
    let lines = art
        .iter()
        .enumerate()
        .map(|(index, line)| {
            Line::from(Span::styled(
                (*line).to_owned(),
                if hit.is_some() && index == impact_row {
                    styles.error
                } else if hit.is_some() && index.is_multiple_of(2) {
                    styles.accent
                } else {
                    styles.base
                },
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}

fn render_ascii_overlay(
    frame: &mut Frame<'_>,
    art: &[&str],
    area: Rect,
    styles: ThemeStyles,
    hit: Option<f64>,
    impact_row: usize,
) {
    for (row, line) in art.iter().enumerate().take(usize::from(area.height)) {
        let x = area
            .x
            .saturating_add(area.width.saturating_sub(line.len() as u16) / 2);
        let y = area.y.saturating_add(row as u16);
        let style = if hit.is_some() && row == impact_row {
            styles.error
        } else if hit.is_some() && row.is_multiple_of(2) {
            styles.accent
        } else {
            styles.base
        };
        for (column, symbol) in line.chars().enumerate() {
            let x = x.saturating_add(column as u16);
            if symbol != ' ' && x < area.right() {
                frame.buffer_mut()[(x, y)].set_char(symbol).set_style(style);
            }
        }
    }
}

fn shifted(area: Rect, amount: u16) -> Rect {
    Rect::new(
        area.x.saturating_add(amount),
        area.y,
        area.width.saturating_sub(amount),
        area.height,
    )
}

fn boss_art(boss: BossKind) -> &'static [&'static str] {
    match boss {
        BossKind::IronWarden => IRON_WARDEN_ART,
        BossKind::ThornQueen => THORN_QUEEN_ART,
        BossKind::NullArchon => NULL_ARCHON_ART,
    }
}

fn boss_name(language: Language, boss: BossKind) -> &'static str {
    text(
        language,
        match boss {
            BossKind::IronWarden => TextKey::IronWarden,
            BossKind::ThornQueen => TextKey::ThornQueen,
            BossKind::NullArchon => TextKey::NullArchon,
        },
    )
}

fn mechanic_summary(language: Language, boss: BossKind) -> &'static str {
    match (language, boss) {
        (Language::Ko, BossKind::IronWarden) => {
            "세 개의 장갑 잠금을 부수고 노출된 코어를 타격하세요."
        }
        (Language::En, BossKind::IronWarden) => {
            "Break three armor locks, then strike the exposed core."
        }
        (Language::Ko, BossKind::ThornQueen) => {
            "첫 키로 덩굴을 지정하고 꽃이 피기 전에 잘라내세요."
        }
        (Language::En, BossKind::ThornQueen) => {
            "Choose a vine with its first key. Cut it before it blooms."
        }
        (Language::Ko, BossKind::NullArchon) => "체크섬 세 개를 연결해 공허의 성가를 역전하세요.",
        (Language::En, BossKind::NullArchon) => {
            "Chain three checksums to reverse the void canticle."
        }
    }
}

fn stars(rank: u8) -> String {
    format!(
        "{}{}",
        "★".repeat(rank.min(3) as usize),
        "☆".repeat(3 - rank.min(3) as usize)
    )
}

fn hearts(remaining: u8) -> String {
    format!(
        "{}{}",
        "♥".repeat(remaining.min(3) as usize),
        "♡".repeat(3 - remaining.min(3) as usize)
    )
}

fn gauge(progress: f64, width: usize, invert: bool) -> String {
    let progress = if invert { 1.0 - progress } else { progress }.clamp(0.0, 1.0);
    let filled = (progress * width as f64).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn cue_locks(cue: BattleCue) -> bool {
    cue != BattleCue::Hit
}

fn unlocked_difficulty(rank: u8) -> Option<GameDifficulty> {
    match rank {
        1 => Some(GameDifficulty::Medium),
        2 => Some(GameDifficulty::Hard),
        3 => Some(GameDifficulty::Hell),
        _ => None,
    }
}

use crate::{
    config::Settings,
    content::ContentCatalog,
    i18n::{TextKey, text},
    model::{Difficulty, Language, PracticeKind},
    practice::PracticeEngine,
    stats::{KeyAccuracy, weak_keys},
    storage::{AppPaths, SessionRecord, save_session},
    theme::ThemeCatalog,
};
use anyhow::{Result, bail};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Screen {
    Home,
    ModeSelect,
    Practice,
    Result,
    Stats,
    History,
    WeakKeys,
    Goals,
    Content,
    ContentDetail,
    Settings,
    Themes,
    Help,
}

impl Screen {
    pub const ALL: [Self; 13] = [
        Self::Home,
        Self::ModeSelect,
        Self::Practice,
        Self::Result,
        Self::Stats,
        Self::History,
        Self::WeakKeys,
        Self::Goals,
        Self::Content,
        Self::ContentDetail,
        Self::Settings,
        Self::Themes,
        Self::Help,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Grade {
    A,
    B,
    C,
    D,
}

pub fn grade(speed: f64, speed_goal: f64, accuracy: f64, accuracy_goal: f64) -> Grade {
    if speed >= speed_goal && accuracy >= accuracy_goal {
        Grade::A
    } else if speed >= speed_goal * 0.8 && accuracy >= 95.0 {
        Grade::B
    } else if speed >= speed_goal * 0.6 && accuracy >= 90.0 {
        Grade::C
    } else {
        Grade::D
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemDelta {
    pub correct_units: u64,
    pub attempted_units: u64,
    pub errors: u64,
    pub speed: f64,
    pub accuracy: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultView {
    pub session: SessionRecord,
    pub previous_speed: Option<f64>,
    pub best_speed: Option<f64>,
    pub speed_delta: Option<f64>,
    pub speed_goal_met: bool,
    pub accuracy_goal_met: bool,
    pub daily_minutes_met: bool,
    pub weak_keys: Vec<KeyAccuracy>,
    pub grade: Option<Grade>,
    pub save_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PracticeMode {
    Quick {
        completed: usize,
    },
    Key {
        stage: u8,
        random: bool,
        weak_repeat: bool,
    },
    Words {
        difficulty: Difficulty,
        completed: usize,
        streak: usize,
    },
    Sentence {
        completed: usize,
        last_item: Option<ItemDelta>,
    },
    Long {
        item_id: String,
        paragraph: usize,
    },
    Test {
        grade: Option<Grade>,
    },
}

impl PracticeMode {
    pub const fn kind(&self) -> PracticeKind {
        match self {
            Self::Quick { .. } => PracticeKind::Quick,
            Self::Key { .. } => PracticeKind::Key,
            Self::Words { .. } => PracticeKind::Words,
            Self::Sentence { .. } => PracticeKind::Sentence,
            Self::Long { .. } => PracticeKind::Long,
            Self::Test { .. } => PracticeKind::Test,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StopRule {
    TargetEnd,
    Items(usize),
    ActiveTime(Duration),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModeRequest {
    pub kind: PracticeKind,
    pub language: Language,
    pub target: String,
    pub mode: PracticeMode,
    pub stop: StopRule,
    pub item_ends: Vec<usize>,
    pub content_ids: Vec<String>,
}

pub struct ActivePractice {
    pub mode: PracticeMode,
    pub engine: PracticeEngine,
    pub stop: StopRule,
    pub item_ends: Vec<usize>,
    pub content_ids: Vec<String>,
    pub status: Option<(String, Instant)>,
    started_at_utc: Option<OffsetDateTime>,
}

impl ActivePractice {
    pub const fn kind(&self) -> PracticeKind {
        self.mode.kind()
    }
}

pub struct App {
    screen: Screen,
    parent: Screen,
    parent_before_help: Option<Screen>,
    focus: usize,
    quit: bool,
    retry_request: Option<ModeRequest>,
    pub settings: Settings,
    pub paths: AppPaths,
    pub content: ContentCatalog,
    pub themes: ThemeCatalog,
    pub sessions: Vec<SessionRecord>,
    pub practice: Option<ActivePractice>,
    pub result: Option<ResultView>,
    pub warnings: Vec<String>,
}

impl App {
    pub fn new(
        settings: Settings,
        paths: AppPaths,
        content: ContentCatalog,
        themes: ThemeCatalog,
        sessions: Vec<SessionRecord>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            screen: Screen::Home,
            parent: Screen::Home,
            parent_before_help: None,
            focus: 0,
            quit: false,
            retry_request: None,
            settings,
            paths,
            content,
            themes,
            sessions,
            practice: None,
            result: None,
            warnings,
        }
    }

    pub const fn screen(&self) -> Screen {
        self.screen
    }

    pub const fn parent(&self) -> Screen {
        self.parent
    }

    pub const fn focus(&self) -> usize {
        self.focus
    }

    pub const fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn active_practice(&self) -> Option<&ActivePractice> {
        self.practice.as_ref()
    }

    pub fn active_practice_mut(&mut self) -> Option<&mut ActivePractice> {
        self.practice.as_mut()
    }

    pub fn retry_request(&self) -> Option<&ModeRequest> {
        self.retry_request.as_ref()
    }

    pub fn open(&mut self, screen: Screen) {
        self.focus = 0;
        if screen == self.screen {
            return;
        }

        if screen == Screen::Help {
            self.parent_before_help = Some(self.parent);
            self.parent = self.screen;
            self.screen = Screen::Help;
            return;
        }

        let prior = if self.screen == Screen::Help {
            self.parent
        } else {
            self.screen
        };
        self.parent = if prior == screen { Screen::Home } else { prior };
        self.parent_before_help = None;
        self.screen = screen;
    }

    pub fn start_mode(&mut self, request: ModeRequest, _now: Instant) -> Result<()> {
        if request.mode.kind() != request.kind {
            bail!("practice mode does not match requested kind");
        }
        let limit = match request.stop {
            StopRule::ActiveTime(duration) => Some(duration),
            StopRule::TargetEnd | StopRule::Items(_) => None,
        };
        let engine = PracticeEngine::new(
            request.language,
            request.kind,
            request.target.as_str(),
            limit,
        )?;
        let retry_request = request.clone();
        let active = ActivePractice {
            mode: request.mode,
            engine,
            stop: request.stop,
            item_ends: request.item_ends,
            content_ids: request.content_ids,
            status: None,
            started_at_utc: None,
        };

        self.screen = Screen::Practice;
        self.parent = Screen::Home;
        self.parent_before_help = None;
        self.focus = 0;
        self.retry_request = Some(retry_request);
        self.practice = Some(active);
        self.result = None;
        Ok(())
    }

    pub fn handle_event(&mut self, event: Event, now: Instant) -> Result<()> {
        let quit = matches!(
            &event,
            Event::Key(key)
                if key.kind != KeyEventKind::Release
                    && matches!(key.code, KeyCode::Char('c' | 'C'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
        );
        let was_practicing = self.screen == Screen::Practice;
        let tick = self.tick(now);
        if quit {
            self.quit = true;
            return tick;
        }
        tick?;
        if was_practicing && self.screen != Screen::Practice {
            return Ok(());
        }
        match event {
            Event::Paste(_) if self.screen == Screen::Practice => {
                if let Some(active) = self.practice.as_mut() {
                    active.status = Some((
                        text(self.settings.ui_language, TextKey::PasteIgnored).into(),
                        now.checked_add(Duration::from_secs(3)).unwrap_or(now),
                    ));
                }
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                self.handle_key(key, now)?;
            }
            _ => {}
        }
        self.tick(now)
    }

    fn handle_key(&mut self, key: KeyEvent, now: Instant) -> Result<()> {
        if self.screen == Screen::Practice {
            return self.handle_practice_key(key, now);
        }

        if key.code == KeyCode::Char('q') && key.modifiers == KeyModifiers::NONE {
            self.quit = true;
            return Ok(());
        }
        if key.code == KeyCode::Char('?')
            && matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT)
        {
            self.open(Screen::Help);
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => self.escape(),
            KeyCode::Tab | KeyCode::Down if self.screen != Screen::Practice => self.move_focus(1),
            KeyCode::BackTab | KeyCode::Up if self.screen != Screen::Practice => {
                self.move_focus(-1);
            }
            KeyCode::Char('j')
                if self.screen != Screen::Practice && key.modifiers == KeyModifiers::NONE =>
            {
                self.move_focus(1);
            }
            KeyCode::Char('k')
                if self.screen != Screen::Practice && key.modifiers == KeyModifiers::NONE =>
            {
                self.move_focus(-1);
            }
            KeyCode::Enter => self.enter(),
            KeyCode::Char('r')
                if self.screen == Screen::Result && key.modifiers == KeyModifiers::NONE =>
            {
                if let Some(request) = self.retry_request.clone() {
                    self.start_mode(request, now)?;
                }
            }
            KeyCode::Char('n') if self.screen == Screen::Result => {}
            _ => {}
        }
        Ok(())
    }

    fn handle_practice_key(&mut self, key: KeyEvent, now: Instant) -> Result<()> {
        let pause = key.kind == KeyEventKind::Press
            && (key.code == KeyCode::Esc
                || (matches!(key.code, KeyCode::Char('p' | 'P'))
                    && key.modifiers == KeyModifiers::CONTROL));
        if pause {
            if let Some(active) = self.practice.as_mut() {
                active.engine.toggle_pause(now);
            }
            return Ok(());
        }

        let Some(active) = self.practice.as_mut() else {
            return Ok(());
        };
        if active.engine.is_paused() {
            return Ok(());
        }
        match key.code {
            KeyCode::Backspace
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                active.engine.backspace();
            }
            KeyCode::Char(character)
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) =>
            {
                let wall_now = OffsetDateTime::now_utc();
                let attempted_before = active.engine.attempted_units();
                active.engine.input(character.encode_utf8(&mut [0; 4]), now);
                if active.started_at_utc.is_none()
                    && active.engine.attempted_units() > attempted_before
                {
                    active.started_at_utc = Some(wall_now);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn practice_status(&self) -> Option<&str> {
        self.practice
            .as_ref()
            .and_then(|active| active.status.as_ref())
            .map(|(message, _)| message.as_str())
    }

    pub fn tick(&mut self, now: Instant) -> Result<()> {
        if let Some(active) = self.practice.as_mut()
            && active
                .status
                .as_ref()
                .is_some_and(|(_, expires_at)| now >= *expires_at)
        {
            active.status = None;
        }
        let finished = self.screen == Screen::Practice
            && self
                .practice
                .as_ref()
                .is_some_and(|active| match active.stop {
                    StopRule::TargetEnd | StopRule::Items(_) => active.engine.target_complete(),
                    StopRule::ActiveTime(_) => active.engine.time_limit_reached(now),
                });
        if finished {
            self.finish_practice(now)?;
        }
        Ok(())
    }

    pub fn finish_practice(&mut self, now: Instant) -> Result<ResultView> {
        let Some(active) = self.practice.as_ref() else {
            bail!("no active practice");
        };
        if active.engine.metrics(now).attempted_units == 0 {
            bail!("cannot finish practice without an attempt");
        }

        let Some(mut active) = self.practice.take() else {
            bail!("no active practice");
        };
        let metrics = active.engine.finalize(now);
        let language = active.engine.language();
        let kind = active.kind();
        let started_at = active
            .started_at_utc
            .unwrap_or_else(OffsetDateTime::now_utc);
        let content_id = active
            .content_ids
            .first()
            .cloned()
            .unwrap_or_else(|| practice_id(kind).into());
        let difficulty = match active.mode {
            PracticeMode::Words { difficulty, .. } => match difficulty {
                Difficulty::Easy => Some(1),
                Difficulty::Medium => Some(2),
                Difficulty::Hard => Some(3),
                Difficulty::Mixed => None,
            },
            _ => None,
        };
        let session = SessionRecord::from_result(
            started_at,
            language,
            kind,
            content_id,
            difficulty,
            &metrics,
            active.engine.intended_keys(),
        );
        let speed = session_speed(&session);
        let comparable = self
            .sessions
            .iter()
            .filter(|prior| prior.language == language && prior.mode == kind)
            .collect::<Vec<_>>();
        let previous_speed = comparable
            .iter()
            .copied()
            .filter(|prior| session_speed(prior).is_finite())
            .max_by(|left, right| {
                left.started_at_unix_ms
                    .cmp(&right.started_at_unix_ms)
                    .then_with(|| left.id.cmp(&right.id))
            })
            .map(session_speed);
        let best_speed = comparable
            .iter()
            .map(|prior| session_speed(prior))
            .filter(|speed| speed.is_finite())
            .max_by(f64::total_cmp);
        let speed_goal = match language {
            Language::Ko => f64::from(self.settings.target_kpm),
            Language::En => f64::from(self.settings.target_wpm),
        };
        let prior_duration = self
            .sessions
            .iter()
            .filter(|prior| prior.local_date == session.local_date)
            .fold(0_u64, |total, prior| {
                total.saturating_add(prior.duration_ms)
            });
        let daily_target = u64::from(self.settings.daily_minutes).saturating_mul(60_000);
        let result_grade = (kind == PracticeKind::Test).then(|| {
            grade(
                speed,
                speed_goal,
                session.accuracy,
                self.settings.target_accuracy,
            )
        });
        let mut view = ResultView {
            previous_speed,
            best_speed,
            speed_delta: previous_speed.map(|previous| speed - previous),
            speed_goal_met: speed >= speed_goal,
            accuracy_goal_met: session.accuracy >= self.settings.target_accuracy,
            daily_minutes_met: prior_duration.saturating_add(session.duration_ms) >= daily_target,
            weak_keys: weak_keys(&session.intended_keys, 1)
                .into_iter()
                .take(5)
                .collect(),
            grade: result_grade,
            save_error: None,
            session,
        };
        match save_session(&self.paths, &view.session) {
            Ok(_) => self.sessions.push(view.session.clone()),
            Err(error) => view.save_error = Some(error.root_cause().to_string()),
        }

        self.screen = Screen::Result;
        self.parent = Screen::Home;
        self.parent_before_help = None;
        self.focus = 0;
        self.result = Some(view.clone());
        Ok(view)
    }

    fn escape(&mut self) {
        match self.screen {
            Screen::Home => self.quit = true,
            Screen::Result => self.return_home(),
            Screen::Help => {
                let destination = self.parent;
                let restored_parent = self.parent_before_help.take().unwrap_or(Screen::Home);
                self.screen = destination;
                self.parent = if restored_parent == destination {
                    Screen::Home
                } else {
                    restored_parent
                };
                self.focus = 0;
            }
            _ => {
                self.screen = if self.parent == self.screen {
                    Screen::Home
                } else {
                    self.parent
                };
                self.parent = Screen::Home;
                self.parent_before_help = None;
                self.focus = 0;
            }
        }
    }

    fn return_home(&mut self) {
        self.screen = Screen::Home;
        self.parent = Screen::Home;
        self.parent_before_help = None;
        self.focus = 0;
    }

    fn focus_count(&self) -> usize {
        match self.screen {
            Screen::Home => 10,
            _ => 1,
        }
    }

    fn move_focus(&mut self, delta: isize) {
        let count = self.focus_count();
        self.focus = if delta < 0 {
            (self.focus + count - 1) % count
        } else {
            (self.focus + 1) % count
        };
    }

    fn enter(&mut self) {
        if self.screen != Screen::Home {
            return;
        }
        let screen = match self.focus {
            0..=5 => Screen::ModeSelect,
            6 => Screen::Stats,
            7 => Screen::Goals,
            8 => Screen::Content,
            9 => Screen::Settings,
            _ => return,
        };
        self.open(screen);
    }
}

fn session_speed(session: &SessionRecord) -> f64 {
    match session.language {
        Language::Ko => session.kpm,
        Language::En => session.wpm,
    }
}

const fn practice_id(kind: PracticeKind) -> &'static str {
    match kind {
        PracticeKind::Quick => "quick",
        PracticeKind::Key => "key",
        PracticeKind::Words => "words",
        PracticeKind::Sentence => "sentence",
        PracticeKind::Long => "long",
        PracticeKind::Test => "test",
    }
}

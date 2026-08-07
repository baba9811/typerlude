use crate::{
    config::Settings,
    content::ContentCatalog,
    model::{Difficulty, Language, PracticeKind},
    practice::PracticeEngine,
    stats::KeyAccuracy,
    storage::{AppPaths, SessionRecord},
    theme::ThemeCatalog,
};
use anyhow::{Result, bail};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

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
        let Event::Key(key) = event else {
            return Ok(());
        };
        if key.kind == KeyEventKind::Release {
            return Ok(());
        }
        self.handle_key(key, now)
    }

    fn handle_key(&mut self, key: KeyEvent, now: Instant) -> Result<()> {
        if matches!(key.code, KeyCode::Char('c' | 'C'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.quit = true;
            return Ok(());
        }

        if self.screen != Screen::Practice {
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

use crate::{
    config::Settings,
    content::{ContentCatalog, ContentKind, ResolvedItem},
    i18n::{TextKey, text},
    model::{Difficulty, Language, PracticeKind},
    practice::{Metrics, PracticeEngine},
    stats::{KeyAccuracy, adaptive_candidates, intended_key_counts, weak_keys},
    storage::{AppPaths, SessionRecord, save_session},
    theme::ThemeCatalog,
};
use anyhow::{Result, bail};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use time::OffsetDateTime;
use unicode_segmentation::UnicodeSegmentation;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuickSource {
    Words,
    Quote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuickOptions {
    language: Language,
    source: QuickSource,
    stop: StopRule,
}

impl QuickOptions {
    pub fn new(language: Language, source: QuickSource, stop: StopRule) -> Result<Self> {
        let valid = match stop {
            StopRule::ActiveTime(duration) => [15, 30, 60, 120]
                .into_iter()
                .map(Duration::from_secs)
                .any(|allowed| duration == allowed),
            StopRule::Items(items) => [10, 25, 50, 100].contains(&items),
            StopRule::TargetEnd => false,
        };
        if !valid {
            bail!("invalid Quick stop rule");
        }
        Ok(Self {
            language,
            source,
            stop,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyStage {
    pub title: &'static str,
    pub keys: &'static [char],
}

const EN_STAGE_1: &[char] = &['f', 'j'];
const EN_STAGE_2: &[char] = &['f', 'j', 'd', 'k'];
const EN_STAGE_3: &[char] = &['f', 'j', 'd', 'k', 's', 'l'];
const EN_STAGE_4: &[char] = &['f', 'j', 'd', 'k', 's', 'l', 'a', ';'];
const EN_STAGE_5: &[char] = &['f', 'j', 'd', 'k', 's', 'l', 'a', ';', 'g', 'h'];
const EN_STAGE_6: &[char] = &['f', 'j', 'd', 'k', 's', 'l', 'a', ';', 'g', 'h', 'e', 'i'];
const EN_STAGE_7: &[char] = &[
    'f', 'j', 'd', 'k', 's', 'l', 'a', ';', 'g', 'h', 'e', 'i', 'r', 'u', 't', 'y', 'w', 'o', 'q',
    'p',
];
const EN_STAGE_8: &[char] = &[
    'f', 'j', 'd', 'k', 's', 'l', 'a', ';', 'g', 'h', 'e', 'i', 'r', 'u', 't', 'y', 'w', 'o', 'q',
    'p', 'c', 'v', 'b', 'n', 'm', 'x', 'z',
];
const EN_STAGE_9: &[char] = &[
    'f', 'j', 'd', 'k', 's', 'l', 'a', ';', 'g', 'h', 'e', 'i', 'r', 'u', 't', 'y', 'w', 'o', 'q',
    'p', 'c', 'v', 'b', 'n', 'm', 'x', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K',
    'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', ':',
];
const EN_STAGE_10: &[char] = &[
    'f', 'j', 'd', 'k', 's', 'l', 'a', ';', 'g', 'h', 'e', 'i', 'r', 'u', 't', 'y', 'w', 'o', 'q',
    'p', 'c', 'v', 'b', 'n', 'm', 'x', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K',
    'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', ':', '1', '2', '3',
    '4', '5', '6', '7', '8', '9', '0', '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '-', '_',
    '=', '+', '[', '{', ']', '}', '\\', '|', '\'', '"', ',', '<', '.', '>', '/', '?', '`', '~',
    ' ',
];
static EN_KEY_STAGES: [KeyStage; 10] = [
    KeyStage {
        title: "F/J",
        keys: EN_STAGE_1,
    },
    KeyStage {
        title: "D/K",
        keys: EN_STAGE_2,
    },
    KeyStage {
        title: "S/L",
        keys: EN_STAGE_3,
    },
    KeyStage {
        title: "A/;",
        keys: EN_STAGE_4,
    },
    KeyStage {
        title: "Home row",
        keys: EN_STAGE_5,
    },
    KeyStage {
        title: "E/I",
        keys: EN_STAGE_6,
    },
    KeyStage {
        title: "Top row",
        keys: EN_STAGE_7,
    },
    KeyStage {
        title: "Letters",
        keys: EN_STAGE_8,
    },
    KeyStage {
        title: "Shift",
        keys: EN_STAGE_9,
    },
    KeyStage {
        title: "Full keyboard",
        keys: EN_STAGE_10,
    },
];

const KO_STAGE_1: &[char] = &['ㅁ', 'ㄴ', 'ㅇ', 'ㄹ'];
const KO_STAGE_2: &[char] = &['ㅁ', 'ㄴ', 'ㅇ', 'ㄹ', 'ㅎ', 'ㅗ', 'ㅓ', 'ㅏ'];
const KO_STAGE_3: &[char] = &[
    'ㅁ', 'ㄴ', 'ㅇ', 'ㄹ', 'ㅎ', 'ㅗ', 'ㅓ', 'ㅏ', 'ㅣ', 'ㅋ', 'ㅌ', 'ㅊ', 'ㅍ', 'ㅠ', 'ㅜ', 'ㅡ',
];
const KO_STAGE_4: &[char] = &[
    'ㅁ', 'ㄴ', 'ㅇ', 'ㄹ', 'ㅎ', 'ㅗ', 'ㅓ', 'ㅏ', 'ㅣ', 'ㅋ', 'ㅌ', 'ㅊ', 'ㅍ', 'ㅠ', 'ㅜ', 'ㅡ',
    'ㅂ', 'ㅈ', 'ㄷ', 'ㄱ', 'ㅅ',
];
const KO_STAGE_5: &[char] = &[
    'ㅁ', 'ㄴ', 'ㅇ', 'ㄹ', 'ㅎ', 'ㅗ', 'ㅓ', 'ㅏ', 'ㅣ', 'ㅋ', 'ㅌ', 'ㅊ', 'ㅍ', 'ㅠ', 'ㅜ', 'ㅡ',
    'ㅂ', 'ㅈ', 'ㄷ', 'ㄱ', 'ㅅ', 'ㅛ', 'ㅕ', 'ㅑ', 'ㅐ', 'ㅔ',
];
const KO_STAGE_6: &[char] = &[
    'ㅁ', 'ㄴ', 'ㅇ', 'ㄹ', 'ㅎ', 'ㅗ', 'ㅓ', 'ㅏ', 'ㅣ', 'ㅋ', 'ㅌ', 'ㅊ', 'ㅍ', 'ㅠ', 'ㅜ', 'ㅡ',
    'ㅂ', 'ㅈ', 'ㄷ', 'ㄱ', 'ㅅ', 'ㅛ', 'ㅕ', 'ㅑ', 'ㅐ', 'ㅔ', 'ㅃ', 'ㅉ', 'ㄸ', 'ㄲ', 'ㅆ', 'ㅒ',
    'ㅖ',
];
const KO_STAGE_7: &[char] = &[
    'ㅁ', 'ㄴ', 'ㅇ', 'ㄹ', 'ㅎ', 'ㅗ', 'ㅓ', 'ㅏ', 'ㅣ', 'ㅋ', 'ㅌ', 'ㅊ', 'ㅍ', 'ㅠ', 'ㅜ', 'ㅡ',
    'ㅂ', 'ㅈ', 'ㄷ', 'ㄱ', 'ㅅ', 'ㅛ', 'ㅕ', 'ㅑ', 'ㅐ', 'ㅔ', 'ㅃ', 'ㅉ', 'ㄸ', 'ㄲ', 'ㅆ', 'ㅒ',
    'ㅖ', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '!', '@', '#', '$', '%', '^', '&', '*',
    '(', ')', '-', '_', '=', '+', '[', '{', ']', '}', '\\', '|', '\'', '"', ',', '<', '.', '>',
    '/', '?', '`', '~', ' ',
];
static KO_KEY_STAGES: [KeyStage; 7] = [
    KeyStage {
        title: "기본자리 1",
        keys: KO_STAGE_1,
    },
    KeyStage {
        title: "기본자리 2",
        keys: KO_STAGE_2,
    },
    KeyStage {
        title: "아랫줄",
        keys: KO_STAGE_3,
    },
    KeyStage {
        title: "윗줄 자음",
        keys: KO_STAGE_4,
    },
    KeyStage {
        title: "윗줄 모음",
        keys: KO_STAGE_5,
    },
    KeyStage {
        title: "Shift 조합",
        keys: KO_STAGE_6,
    },
    KeyStage {
        title: "전체 자판",
        keys: KO_STAGE_7,
    },
];

pub const fn key_stages(language: Language) -> &'static [KeyStage] {
    match language {
        Language::Ko => &KO_KEY_STAGES,
        Language::En => &EN_KEY_STAGES,
    }
}

pub fn key_sequence(
    language: Language,
    stage: u8,
    random: bool,
    weak: &[char],
    seed: u64,
) -> Result<String> {
    let stage = key_stage(language, stage)?;
    let mut cycle = stage.keys.to_vec();
    let mut seen = HashSet::new();
    for &key in weak {
        if stage.keys.contains(&key) && seen.insert(key) {
            cycle.extend([key, key]);
        }
    }
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut sequence = String::new();
    let mut count = 0;
    while count < KEY_SEQUENCE_UNITS {
        if random {
            rng.shuffle(&mut cycle);
        }
        for &key in &cycle {
            sequence.push(key);
            count += 1;
            if count == KEY_SEQUENCE_UNITS {
                break;
            }
        }
    }
    Ok(sequence)
}

fn key_stage(language: Language, stage: u8) -> Result<&'static KeyStage> {
    let Some(stage) = usize::from(stage)
        .checked_sub(1)
        .and_then(|index| key_stages(language).get(index))
    else {
        bail!("invalid key-practice stage");
    };
    Ok(stage)
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
    live_metrics: Metrics,
    item_metrics: Metrics,
    next_item: usize,
    current_item_delta: Option<ItemDelta>,
    sentence_delta_expires_at: Option<Instant>,
    stream: Option<CatalogStream>,
    leave_confirmation: bool,
}

impl ActivePractice {
    pub const fn kind(&self) -> PracticeKind {
        self.mode.kind()
    }

    pub const fn live_metrics(&self) -> &Metrics {
        &self.live_metrics
    }

    pub const fn current_item_delta(&self) -> Option<&ItemDelta> {
        self.current_item_delta.as_ref()
    }

    pub const fn leave_confirmation(&self) -> bool {
        self.leave_confirmation
    }
}

#[derive(Clone)]
struct CatalogStream {
    language: Language,
    kinds: &'static [ContentKind],
    difficulty: Difficulty,
    separator: &'static str,
    next_seed: u64,
    adaptive: bool,
}

pub struct App {
    screen: Screen,
    parent: Screen,
    parent_before_help: Option<Screen>,
    focus: usize,
    quit: bool,
    retry_request: Option<ModeRequest>,
    retry_stream: Option<CatalogStream>,
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
            retry_stream: None,
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

    pub fn start_mode(&mut self, request: ModeRequest, now: Instant) -> Result<()> {
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
        let metrics = engine.metrics(now);
        let retry_request = request.clone();
        let active = ActivePractice {
            mode: request.mode,
            engine,
            stop: request.stop,
            item_ends: request.item_ends,
            content_ids: request.content_ids,
            status: None,
            started_at_utc: None,
            live_metrics: metrics.clone(),
            item_metrics: metrics,
            next_item: 0,
            current_item_delta: None,
            sentence_delta_expires_at: None,
            stream: None,
            leave_confirmation: false,
        };

        self.screen = Screen::Practice;
        self.parent = Screen::Home;
        self.parent_before_help = None;
        self.focus = 0;
        self.retry_request = Some(retry_request);
        self.retry_stream = None;
        self.practice = Some(active);
        self.result = None;
        Ok(())
    }

    pub fn start_quick(&mut self, options: QuickOptions, seed: u64, now: Instant) -> Result<()> {
        let (kinds, separator) = match options.source {
            QuickSource::Words => (WORD_KINDS, " "),
            QuickSource::Quote => (QUOTE_KINDS, "\n"),
        };
        let timed = matches!(options.stop, StopRule::ActiveTime(_));
        let count = match options.stop {
            StopRule::Items(items) => items,
            StopRule::ActiveTime(_) => STREAM_BATCH_ITEMS,
            StopRule::TargetEnd => bail!("invalid Quick stop rule"),
        };
        let stream = CatalogStream {
            language: options.language,
            kinds,
            difficulty: Difficulty::Mixed,
            separator,
            next_seed: seed.wrapping_add(1),
            adaptive: false,
        };
        let request = self.catalog_request(
            PracticeMode::Quick { completed: 0 },
            options.stop,
            &stream,
            count,
            seed,
        )?;
        self.start_mode(request, now)?;
        if timed {
            let Some(active) = self.practice.as_mut() else {
                bail!("practice did not start");
            };
            active.stream = Some(stream.clone());
            self.retry_stream = Some(stream);
        }
        Ok(())
    }

    pub fn start_words(
        &mut self,
        language: Language,
        difficulty: Difficulty,
        seed: u64,
        now: Instant,
    ) -> Result<()> {
        let stream = CatalogStream {
            language,
            kinds: WORD_KINDS,
            difficulty,
            separator: " ",
            next_seed: seed.wrapping_add(1),
            adaptive: self.settings.adaptive,
        };
        let request = self.catalog_request(
            PracticeMode::Words {
                difficulty,
                completed: 0,
                streak: 0,
            },
            StopRule::TargetEnd,
            &stream,
            WORD_BATCH_ITEMS,
            seed,
        )?;
        self.start_mode(request, now)
    }

    pub fn start_sentence(&mut self, language: Language, seed: u64, now: Instant) -> Result<()> {
        let stream = CatalogStream {
            language,
            kinds: SENTENCE_KINDS,
            difficulty: Difficulty::Mixed,
            separator: "\n",
            next_seed: seed.wrapping_add(1),
            adaptive: false,
        };
        let request = self.catalog_request(
            PracticeMode::Sentence {
                completed: 0,
                last_item: None,
            },
            StopRule::TargetEnd,
            &stream,
            SENTENCE_BATCH_ITEMS,
            seed,
        )?;
        self.start_mode(request, now)
    }

    pub fn start_key(
        &mut self,
        language: Language,
        stage: u8,
        random: bool,
        weak_repeat: bool,
        seed: u64,
        now: Instant,
    ) -> Result<()> {
        let stage_keys = key_stage(language, stage)?.keys;
        let weak = if weak_repeat {
            weak_keys(&intended_key_counts(&self.sessions, language), 10)
                .into_iter()
                .filter(|key| stage_keys.contains(&key.key))
                .take(3)
                .map(|key| key.key)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let target = key_sequence(language, stage, random, &weak, seed)?;
        self.start_mode(
            ModeRequest {
                kind: PracticeKind::Key,
                language,
                target,
                mode: PracticeMode::Key {
                    stage,
                    random,
                    weak_repeat,
                },
                stop: StopRule::TargetEnd,
                item_ends: vec![KEY_SEQUENCE_UNITS],
                content_ids: Vec::new(),
            },
            now,
        )
    }

    fn catalog_request(
        &self,
        mode: PracticeMode,
        stop: StopRule,
        stream: &CatalogStream,
        count: usize,
        seed: u64,
    ) -> Result<ModeRequest> {
        let items = select_catalog_items(&self.content, &self.sessions, stream, count, seed)?;
        let (target, item_ends, content_ids) = catalog_target(&items, stream.separator);
        let kind = mode.kind();
        Ok(ModeRequest {
            kind,
            language: stream.language,
            target,
            mode,
            stop,
            item_ends,
            content_ids,
        })
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
                    let stream = self.retry_stream.clone();
                    self.start_mode(request, now)?;
                    if let Some(stream) = stream {
                        if let Some(active) = self.practice.as_mut() {
                            active.stream = Some(stream.clone());
                        }
                        self.retry_stream = Some(stream);
                    }
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
            if let Some(active) = self.practice.as_mut()
                && active.engine.toggle_pause(now)
            {
                active.leave_confirmation = false;
            }
            return Ok(());
        }

        let Some(active) = self.practice.as_ref() else {
            return Ok(());
        };
        if active.engine.is_paused() {
            if key.kind == KeyEventKind::Press
                && key.code == KeyCode::Char('q')
                && key.modifiers == KeyModifiers::NONE
            {
                let confirmed = active.leave_confirmation;
                let attempted = active.engine.attempted_units() != 0;
                if !confirmed {
                    if let Some(active) = self.practice.as_mut() {
                        active.leave_confirmation = true;
                    }
                } else if attempted {
                    self.finish_practice(now)?;
                } else {
                    self.practice = None;
                    self.result = None;
                    self.return_home();
                }
            }
            return Ok(());
        }
        match key.code {
            KeyCode::Backspace
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                if let Some(active) = self.practice.as_mut() {
                    let floor = active
                        .next_item
                        .checked_sub(1)
                        .and_then(|index| active.item_ends.get(index))
                        .copied()
                        .unwrap_or(0);
                    if active.engine.cursor() > floor && active.engine.backspace() {
                        active.live_metrics = active.engine.metrics(now);
                        active.current_item_delta = Some(item_delta(
                            &active.item_metrics,
                            &active.live_metrics,
                            active.engine.language(),
                        ));
                    }
                }
            }
            KeyCode::Char(character)
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) =>
            {
                self.input_practice(character.encode_utf8(&mut [0; 4]), now)?;
            }
            KeyCode::Enter
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) =>
            {
                self.input_practice("\n", now)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn input_practice(&mut self, text: &str, now: Instant) -> Result<()> {
        let Some(active) = self.practice.as_mut() else {
            return Ok(());
        };
        let wall_now = OffsetDateTime::now_utc();
        let attempted_before = active.engine.attempted_units();
        let errors_before = active.live_metrics.errors;
        active.engine.input(text, now);
        if active.started_at_utc.is_none() && active.engine.attempted_units() > attempted_before {
            active.started_at_utc = Some(wall_now);
        }
        active.live_metrics = active.engine.metrics(now);
        if active.engine.attempted_units() > attempted_before {
            active.current_item_delta = Some(item_delta(
                &active.item_metrics,
                &active.live_metrics,
                active.engine.language(),
            ));
        }
        if active.live_metrics.errors > errors_before
            && let PracticeMode::Words { streak, .. } = &mut active.mode
        {
            *streak = 0;
        }
        self.advance_item_boundaries(now)
    }

    fn advance_item_boundaries(&mut self, now: Instant) -> Result<()> {
        let mut advanced = false;
        if let Some(active) = self.practice.as_mut() {
            while let Some(end) = active.item_ends.get(active.next_item).copied() {
                if active.engine.cursor() < end
                    || !active
                        .engine
                        .target_cells()
                        .take(end)
                        .all(|(_, entered)| entered == Some(true))
                {
                    break;
                }

                let delta = item_delta(
                    &active.item_metrics,
                    &active.live_metrics,
                    active.engine.language(),
                );
                active.item_metrics = active.live_metrics.clone();
                active.next_item += 1;
                active.current_item_delta = Some(delta.clone());
                match &mut active.mode {
                    PracticeMode::Quick { completed } => {
                        *completed = completed.saturating_add(1);
                    }
                    PracticeMode::Words {
                        completed, streak, ..
                    } => {
                        *completed = completed.saturating_add(1);
                        if delta.errors == 0 {
                            *streak = streak.saturating_add(1);
                        } else {
                            *streak = 0;
                        }
                    }
                    PracticeMode::Sentence {
                        completed,
                        last_item,
                    } => {
                        *completed = completed.saturating_add(1);
                        *last_item = Some(delta);
                        active.sentence_delta_expires_at =
                            Some(now.checked_add(Duration::from_secs(3)).unwrap_or(now));
                    }
                    PracticeMode::Key { .. }
                    | PracticeMode::Long { .. }
                    | PracticeMode::Test { .. } => {}
                }
                advanced = true;
            }
        }
        if advanced {
            self.extend_catalog_stream()?;
        }
        Ok(())
    }

    fn extend_catalog_stream(&mut self) -> Result<()> {
        let Some(stream) = self.practice.as_ref().and_then(|active| {
            let remaining = active.item_ends.len().saturating_sub(active.next_item);
            (matches!(active.stop, StopRule::ActiveTime(_)) && remaining < 10)
                .then(|| active.stream.clone())
                .flatten()
        }) else {
            return Ok(());
        };
        let items = select_catalog_items(
            &self.content,
            &self.sessions,
            &stream,
            STREAM_BATCH_ITEMS,
            stream.next_seed,
        )?;
        let (target, relative_ends, content_ids) = catalog_target(&items, stream.separator);
        let Some(active) = self.practice.as_mut() else {
            return Ok(());
        };
        let separator_len = UnicodeSegmentation::graphemes(stream.separator, true).count();
        let offset = active.engine.target_len() + separator_len;
        if let Some(end) = active.item_ends.last_mut() {
            *end += separator_len;
        }
        active.engine.extend_target(stream.separator, &target)?;
        active
            .item_ends
            .extend(relative_ends.into_iter().map(|end| offset + end));
        active.content_ids.extend(content_ids);
        if let Some(active_stream) = active.stream.as_mut() {
            active_stream.next_seed = stream.next_seed.wrapping_add(1);
        }
        Ok(())
    }

    pub fn word_progress(&self) -> (usize, usize) {
        match self.practice.as_ref().map(|active| &active.mode) {
            Some(PracticeMode::Words {
                completed, streak, ..
            }) => (*completed, *streak),
            _ => (0, 0),
        }
    }

    pub fn sentence_delta(&self) -> Option<&ItemDelta> {
        match self.practice.as_ref().map(|active| &active.mode) {
            Some(PracticeMode::Sentence { last_item, .. }) => last_item.as_ref(),
            _ => None,
        }
    }

    pub fn practice_status(&self) -> Option<&str> {
        self.practice
            .as_ref()
            .and_then(|active| active.status.as_ref())
            .map(|(message, _)| message.as_str())
    }

    pub fn tick(&mut self, now: Instant) -> Result<()> {
        if let Some(active) = self.practice.as_mut() {
            active.live_metrics = active.engine.metrics(now);
            let item_start = active
                .next_item
                .checked_sub(1)
                .and_then(|index| active.item_ends.get(index))
                .copied()
                .unwrap_or(0);
            if active.engine.cursor() > item_start {
                active.current_item_delta = Some(item_delta(
                    &active.item_metrics,
                    &active.live_metrics,
                    active.engine.language(),
                ));
            }
            if active
                .status
                .as_ref()
                .is_some_and(|(_, expires_at)| now >= *expires_at)
            {
                active.status = None;
            }
            if active
                .sentence_delta_expires_at
                .is_some_and(|expires_at| now >= expires_at)
            {
                if let PracticeMode::Sentence { last_item, .. } = &mut active.mode {
                    *last_item = None;
                }
                active.sentence_delta_expires_at = None;
            }
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

const WORD_KINDS: &[ContentKind] = &[ContentKind::Word];
const QUOTE_KINDS: &[ContentKind] = &[ContentKind::Quote];
const SENTENCE_KINDS: &[ContentKind] = &[ContentKind::Sentence, ContentKind::Quote];
const STREAM_BATCH_ITEMS: usize = 20;
const WORD_BATCH_ITEMS: usize = 25;
const SENTENCE_BATCH_ITEMS: usize = 10;
const KEY_SEQUENCE_UNITS: usize = 120;

fn select_catalog_items<'a>(
    catalog: &'a ContentCatalog,
    sessions: &[SessionRecord],
    stream: &CatalogStream,
    count: usize,
    seed: u64,
) -> Result<Vec<&'a ResolvedItem>> {
    let mut selected = Vec::with_capacity(count);
    let mut cycle_seed = seed;
    while selected.len() < count {
        let mut ordinary = catalog
            .items()
            .filter(|item| catalog_match(item, stream))
            .collect::<Vec<_>>();
        ordinary.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        fastrand::Rng::with_seed(cycle_seed).shuffle(&mut ordinary);

        let mut cycle = if stream.adaptive {
            adaptive_candidates(catalog, sessions, stream.language, cycle_seed)
                .into_iter()
                .filter(|item| catalog_match(item, stream))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut seen = cycle
            .iter()
            .map(|item| item.id.as_str())
            .collect::<HashSet<_>>();
        cycle.extend(
            ordinary
                .into_iter()
                .filter(|item| seen.insert(item.id.as_str())),
        );
        if cycle.is_empty() {
            bail!("no matching practice content");
        }
        let remaining = count - selected.len();
        selected.extend(cycle.into_iter().take(remaining));
        cycle_seed = cycle_seed.wrapping_add(1);
    }
    Ok(selected)
}

fn catalog_match(item: &ResolvedItem, stream: &CatalogStream) -> bool {
    item.language == stream.language
        && stream.kinds.contains(&item.kind)
        && match stream.difficulty {
            Difficulty::Easy => item.difficulty == Some(1),
            Difficulty::Medium => item.difficulty == Some(2),
            Difficulty::Hard => item.difficulty == Some(3),
            Difficulty::Mixed => true,
        }
}

fn catalog_target(items: &[&ResolvedItem], separator: &str) -> (String, Vec<usize>, Vec<String>) {
    let mut target = String::new();
    let mut item_ends = Vec::with_capacity(items.len());
    let mut content_ids = Vec::with_capacity(items.len());
    let mut graphemes = 0;
    for (index, item) in items.iter().enumerate() {
        target.push_str(&item.text);
        graphemes += UnicodeSegmentation::graphemes(item.text.as_str(), true).count();
        if index + 1 != items.len() {
            target.push_str(separator);
            graphemes += UnicodeSegmentation::graphemes(separator, true).count();
        }
        item_ends.push(graphemes);
        content_ids.push(item.id.clone());
    }
    (target, item_ends, content_ids)
}

fn item_delta(before: &Metrics, after: &Metrics, language: Language) -> ItemDelta {
    let correct_units = after.correct_units.saturating_sub(before.correct_units);
    let attempted_units = after.attempted_units.saturating_sub(before.attempted_units);
    let correct_attempts = correct_attempts(after).saturating_sub(correct_attempts(before));
    let minutes = after.active.saturating_sub(before.active).as_secs_f64() / 60.0;
    let units_per_minute = if minutes > 0.0 {
        correct_units as f64 / minutes
    } else {
        0.0
    };
    ItemDelta {
        correct_units,
        attempted_units,
        errors: after.errors.saturating_sub(before.errors),
        speed: match language {
            Language::Ko => units_per_minute,
            Language::En => units_per_minute / 5.0,
        },
        accuracy: if attempted_units == 0 {
            100.0
        } else {
            correct_attempts as f64 / attempted_units as f64 * 100.0
        },
    }
}

fn correct_attempts(metrics: &Metrics) -> u64 {
    (metrics.accuracy / 100.0 * metrics.attempted_units as f64)
        .round()
        .clamp(0.0, metrics.attempted_units as f64) as u64
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

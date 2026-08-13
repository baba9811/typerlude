mod content_flow;
mod navigation;
mod practice_flow;
mod practice_session;
mod result;

use self::content_flow::collect_content_packs;
pub use self::{
    practice_flow::{key_sequence, key_stages},
    result::grade,
};
use crate::{
    config::Settings,
    content::{ContentCatalog, ContentKind, SourceMeta},
    model::{Difficulty, Language, PracticeKind},
    practice::{Metrics, PracticeEngine},
    stats::{KeyAccuracy, ProgressPoint, Range, progress},
    storage::{AppPaths, SessionRecord},
    theme::ThemeCatalog,
    update::UpdateNotice,
};
use anyhow::{Result, anyhow, bail};
use std::{
    collections::HashMap,
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};
use time::{Date, OffsetDateTime, UtcOffset};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputEvent {
    Key(KeyInput),
    Paste,
    Ignored,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyInput {
    pub key: Key,
    pub modifiers: KeyModifiers,
    pub kind: KeyKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    BackTab,
    Backspace,
    Char(char),
    Down,
    Enter,
    Esc,
    Left,
    Right,
    Tab,
    Up,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyKind {
    Press,
    Repeat,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub other: bool,
}

impl KeyModifiers {
    pub const NONE: Self = Self {
        shift: false,
        control: false,
        other: false,
    };
    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };
    pub const CONTROL: Self = Self {
        control: true,
        ..Self::NONE
    };
    pub const OTHER: Self = Self {
        other: true,
        ..Self::NONE
    };
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Screen {
    Home,
    ModeOptions,
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
        Self::ModeOptions,
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
    pub kpm: f64,
    pub wpm: f64,
    pub accuracy: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultView {
    pub session: SessionRecord,
    pub previous_kpm: Option<f64>,
    pub previous_wpm: Option<f64>,
    pub best_kpm: Option<f64>,
    pub best_wpm: Option<f64>,
    pub kpm_delta: Option<f64>,
    pub wpm_delta: Option<f64>,
    pub speed_goal: f64,
    pub accuracy_goal: f64,
    pub daily_minutes_goal: u32,
    pub speed_goal_met: bool,
    pub accuracy_goal_met: bool,
    pub daily_minutes_met: bool,
    pub weak_keys: Vec<KeyAccuracy>,
    pub grade: Option<Grade>,
    pub save_error: Option<String>,
    pub long: Option<LongOutcome>,
}

#[derive(Clone, Debug)]
pub struct ContentProvenance {
    pub item_id: Option<String>,
    pub source: SourceMeta,
}

#[derive(Clone, Debug)]
pub struct ContentPackSummary {
    pub id: String,
    pub sample_item_id: String,
    pub provenance: Vec<ContentProvenance>,
    pub language: Language,
    pub items: usize,
    pub licenses: Vec<String>,
    pub kinds: Vec<ContentKind>,
    pub enabled: bool,
    pub built_in: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustomTextSource {
    File,
    Stdin,
}

impl CustomTextSource {
    const fn content_id(self) -> &'static str {
        match self {
            Self::File => "custom-file",
            Self::Stdin => "stdin",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LongMetadata {
    pub title: String,
    pub author: String,
    pub source: String,
    pub license: String,
    pub difficulty: Option<u8>,
    pub tags: Vec<String>,
    pub custom_source: Option<CustomTextSource>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LongScroll {
    pub active_paragraph: usize,
    pub total_paragraphs: usize,
    pub percent: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongOutcome {
    pub best_rolling_kpm: f64,
    pub best_rolling_wpm: f64,
    pub completed_graphemes: usize,
    pub total_graphemes: usize,
    pub percent: usize,
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
    TargetOrActiveTime(Duration),
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

pub(crate) const QUICK_TIME_PRESETS: &[u64] = &[15, 30, 60, 120];
pub(crate) const QUICK_COUNT_PRESETS: &[usize] = &[10, 25, 50, 100];
pub(crate) const TEST_DURATION_PRESETS: &[u64] = &[60, 180, 300, 600];

#[derive(Clone, Debug)]
pub(crate) struct ModeOptions {
    pub(crate) kind: PracticeKind,
    pub(crate) language: Language,
    pub(crate) quick_source: QuickSource,
    pub(crate) quick_items: bool,
    pub(crate) quick_preset: usize,
    pub(crate) key_stage: u8,
    pub(crate) key_random: bool,
    pub(crate) key_weak_repeat: bool,
    pub(crate) word_difficulty: Difficulty,
    pub(crate) test_preset: usize,
    pub(crate) test_selection: usize,
    pub(crate) long_selection: usize,
}

impl ModeOptions {
    fn new(kind: PracticeKind, language: Language) -> Self {
        Self {
            kind,
            language,
            quick_source: QuickSource::Words,
            quick_items: false,
            quick_preset: 1,
            key_stage: 1,
            key_random: false,
            key_weak_repeat: false,
            word_difficulty: Difficulty::Mixed,
            test_preset: 2,
            test_selection: 0,
            long_selection: 0,
        }
    }

    fn quick_stop(&self) -> StopRule {
        if self.quick_items {
            StopRule::Items(QUICK_COUNT_PRESETS[self.quick_preset])
        } else {
            StopRule::ActiveTime(Duration::from_secs(QUICK_TIME_PRESETS[self.quick_preset]))
        }
    }
}

impl QuickOptions {
    pub fn new(language: Language, source: QuickSource, stop: StopRule) -> Result<Self> {
        let valid = match stop {
            StopRule::ActiveTime(duration) => QUICK_TIME_PRESETS
                .iter()
                .copied()
                .map(Duration::from_secs)
                .any(|allowed| duration == allowed),
            StopRule::Items(items) => QUICK_COUNT_PRESETS.contains(&items),
            StopRule::TargetEnd | StopRule::TargetOrActiveTime(_) => false,
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
    observed_input_language: Option<Language>,
    started_at_utc: Option<OffsetDateTime>,
    live_metrics: Metrics,
    item_metrics: Metrics,
    next_item: usize,
    current_item_delta: Option<ItemDelta>,
    sentence_delta_expires_at: Option<Instant>,
    stream: Option<CatalogStream>,
    long_metadata: Option<LongMetadata>,
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

    pub const fn observed_input_language(&self) -> Option<Language> {
        self.observed_input_language
    }

    pub fn long_metadata(&self) -> Option<&LongMetadata> {
        self.long_metadata.as_ref()
    }

    pub fn long_scroll(&self) -> Option<LongScroll> {
        let PracticeMode::Long { .. } = self.mode else {
            return None;
        };
        let total_paragraphs = self.item_ends.len();
        let position = self
            .engine
            .current_line_range()
            .map_or_else(|| self.engine.cursor(), |range| range.start);
        let paragraph = self.item_ends.partition_point(|&end| end <= position);
        Some(LongScroll {
            active_paragraph: paragraph.saturating_add(1).min(total_paragraphs),
            total_paragraphs,
            percent: self.engine.cursor().saturating_mul(100) / self.engine.target_len(),
        })
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
    focus_memory: HashMap<Screen, usize>,
    mode_options: ModeOptions,
    quit: bool,
    retry_request: Option<ModeRequest>,
    retry_stream: Option<CatalogStream>,
    retry_long_metadata: Option<LongMetadata>,
    stats_range: Range,
    stats_language: Language,
    stats_mode: Option<PracticeKind>,
    selected_content_pack: Option<String>,
    content_disable_confirmation: bool,
    content_pack_summaries: Vec<ContentPackSummary>,
    pub settings: Settings,
    pub paths: AppPaths,
    pub content: ContentCatalog,
    pub themes: ThemeCatalog,
    pub sessions: Vec<SessionRecord>,
    pub practice: Option<ActivePractice>,
    pub result: Option<ResultView>,
    pub update_notice: Option<UpdateNotice>,
    pub warnings: Vec<String>,
    update_rx: Option<Receiver<Option<UpdateNotice>>>,
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
        let stats_language = settings.language;
        let mode_options = ModeOptions::new(PracticeKind::Quick, settings.language);
        let content_pack_summaries = collect_content_packs(&content, &paths.content);
        Self {
            screen: Screen::Home,
            parent: Screen::Home,
            parent_before_help: None,
            focus: 0,
            focus_memory: HashMap::new(),
            mode_options,
            quit: false,
            retry_request: None,
            retry_stream: None,
            retry_long_metadata: None,
            stats_range: Range::Days30,
            stats_language,
            stats_mode: None,
            selected_content_pack: None,
            content_disable_confirmation: false,
            content_pack_summaries,
            settings,
            paths,
            content,
            themes,
            sessions,
            practice: None,
            result: None,
            update_notice: None,
            warnings,
            update_rx: None,
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

    pub(crate) const fn mode_options(&self) -> &ModeOptions {
        &self.mode_options
    }

    pub const fn should_quit(&self) -> bool {
        self.quit
    }

    pub(crate) fn request_quit(&mut self) {
        self.quit = true;
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

    pub const fn stats_range(&self) -> Range {
        self.stats_range
    }

    pub const fn stats_language(&self) -> Language {
        self.stats_language
    }

    pub const fn stats_mode(&self) -> Option<PracticeKind> {
        self.stats_mode
    }

    pub fn set_stats_range(&mut self, range: Range) {
        self.stats_range = range;
    }

    pub fn set_stats_language(&mut self, language: Language) {
        self.stats_language = language;
    }

    pub fn set_stats_mode(&mut self, mode: Option<PracticeKind>) {
        self.stats_mode = mode;
    }

    pub fn stats_points(&self) -> Vec<ProgressPoint> {
        progress(
            &self.sessions,
            self.stats_range,
            self.stats_today(),
            self.stats_language,
            self.stats_mode,
        )
    }

    pub(crate) fn stats_today(&self) -> Date {
        let now = OffsetDateTime::now_utc();
        now.to_offset(UtcOffset::local_offset_at(now).unwrap_or(UtcOffset::UTC))
            .date()
    }

    pub fn set_target_kpm(&mut self, value: u32) -> Result<()> {
        self.change_settings(|settings| settings.target_kpm = value)
    }

    pub fn set_target_wpm(&mut self, value: u32) -> Result<()> {
        self.change_settings(|settings| settings.target_wpm = value)
    }

    pub fn set_target_accuracy(&mut self, value: f64) -> Result<()> {
        self.change_settings(|settings| settings.target_accuracy = value)
    }

    pub fn set_daily_minutes(&mut self, value: u32) -> Result<()> {
        self.change_settings(|settings| settings.daily_minutes = value)
    }

    pub fn select_theme(&mut self, id: &str) -> Result<()> {
        if self.themes.get(id).is_none() {
            let error = anyhow!("unknown theme {id:?}");
            self.warnings.push(format!("settings: {error}"));
            return Err(error);
        }
        self.change_settings(|settings| settings.theme = id.to_owned())
    }

    pub fn set_update_receiver(&mut self, receiver: Receiver<Option<UpdateNotice>>) {
        self.update_rx = Some(receiver);
    }

    pub fn poll_update(&mut self) {
        match self.update_rx.as_ref().map(Receiver::try_recv) {
            Some(Ok(notice)) => {
                if let Some(notice) = notice {
                    self.update_notice = Some(notice);
                }
                self.update_rx = None;
            }
            Some(Err(TryRecvError::Disconnected)) => self.update_rx = None,
            Some(Err(TryRecvError::Empty)) | None => {}
        }
    }

    fn change_settings(&mut self, change: impl FnOnce(&mut Settings)) -> Result<()> {
        let mut candidate = self.settings.clone();
        change(&mut candidate);
        if let Err(error) = candidate.save(&self.paths) {
            self.warnings.push(format!("settings: {error:#}"));
            return Err(error);
        }
        self.settings = candidate;
        Ok(())
    }
}

mod content_flow;
mod practice_flow;
mod result;

use self::{
    content_flow::collect_content_packs,
    practice_flow::{STREAM_BATCH_ITEMS, TEXT_KINDS, catalog_target, select_catalog_items},
    result::item_delta,
};
pub use self::{
    practice_flow::{key_sequence, key_stages},
    result::grade,
};
use crate::{
    config::Settings,
    content::{ContentCatalog, ContentKind, SourceMeta},
    i18n::{TextKey, text},
    model::{Difficulty, Language, PracticeKind},
    practice::{Metrics, PracticeEngine},
    stats::{KeyAccuracy, ProgressPoint, Range, progress},
    storage::{AppPaths, SessionRecord},
    theme::ThemeCatalog,
    typing::input_language,
    update::UpdateNotice,
};
use anyhow::{Result, anyhow, bail};
use std::{
    collections::HashMap,
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};
use time::{Date, OffsetDateTime, UtcOffset};
use unicode_segmentation::UnicodeSegmentation;

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

    pub fn open(&mut self, screen: Screen) {
        if screen == self.screen {
            self.focus = 0;
            return;
        }
        self.remember_focus();

        if screen == Screen::Help {
            self.parent_before_help = Some(self.parent);
            self.parent = self.screen;
            self.screen = Screen::Help;
            self.focus = 0;
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
        self.focus = 0;
    }

    pub fn handle_event(&mut self, event: InputEvent, now: Instant) -> Result<()> {
        let update_notice_was_visible = self.update_notice.is_some();
        let quit = matches!(
            &event,
            InputEvent::Key(key)
                if matches!(key.key, Key::Char('c' | 'C')) && key.modifiers.control
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
            InputEvent::Paste if self.screen == Screen::Practice => {
                if let Some(active) = self.practice.as_mut()
                    && !(active.kind() == PracticeKind::Test && active.leave_confirmation())
                {
                    active.status = Some((
                        text(self.settings.ui_language, TextKey::PasteIgnored).into(),
                        now.checked_add(Duration::from_secs(3)).unwrap_or(now),
                    ));
                }
            }
            InputEvent::Key(key) => {
                self.handle_key(key, now, update_notice_was_visible)?;
            }
            _ => {}
        }
        self.tick(now)
    }

    fn handle_key(
        &mut self,
        key: KeyInput,
        now: Instant,
        update_notice_was_visible: bool,
    ) -> Result<()> {
        if self.screen == Screen::Practice {
            return self.handle_practice_key(key, now);
        }

        if key.key == Key::Char('q') && key.modifiers == KeyModifiers::NONE {
            self.quit = true;
            return Ok(());
        }
        if key.key == Key::Char('?')
            && matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT)
        {
            self.open(Screen::Help);
            return Ok(());
        }
        if matches!(self.screen, Screen::Home | Screen::Result)
            && key.kind == KeyKind::Press
            && key.modifiers == KeyModifiers::NONE
            && update_notice_was_visible
            && self.update_notice.is_some()
        {
            match key.key {
                Key::Char('l') => {
                    self.update_notice = None;
                    return Ok(());
                }
                Key::Char('s') => {
                    let latest = self
                        .update_notice
                        .as_ref()
                        .map(|notice| notice.latest.to_string())
                        .unwrap_or_default();
                    if self
                        .change_settings(|settings| settings.skipped_update_version = latest)
                        .is_ok()
                    {
                        self.update_notice = None;
                    }
                    return Ok(());
                }
                _ => {}
            }
        }

        match key.key {
            Key::Esc => self.escape(),
            Key::Tab | Key::Down if self.screen != Screen::Practice => self.move_focus(1),
            Key::BackTab | Key::Up if self.screen != Screen::Practice => {
                self.move_focus(-1);
            }
            Key::Char('j')
                if self.screen != Screen::Practice && key.modifiers == KeyModifiers::NONE =>
            {
                self.move_focus(1);
            }
            Key::Char('k')
                if self.screen != Screen::Practice && key.modifiers == KeyModifiers::NONE =>
            {
                self.move_focus(-1);
            }
            Key::Left if key.modifiers == KeyModifiers::NONE => self.adjust(-1),
            Key::Right if key.modifiers == KeyModifiers::NONE => self.adjust(1),
            Key::Char('d')
                if self.screen == Screen::ContentDetail
                    && key.kind == KeyKind::Press
                    && key.modifiers == KeyModifiers::NONE =>
            {
                self.disable_selected_content();
            }
            Key::Enter if key.modifiers == KeyModifiers::NONE => self.enter(now)?,
            Key::Char('r')
                if self.screen == Screen::Result && key.modifiers == KeyModifiers::NONE =>
            {
                if let Some(request) = self.retry_request.clone() {
                    let stream = self.retry_stream.clone();
                    let long_metadata = self.retry_long_metadata.clone();
                    self.start_mode(request, now)?;
                    if let Some(stream) = stream {
                        if let Some(active) = self.practice.as_mut() {
                            active.stream = Some(stream.clone());
                        }
                        self.retry_stream = Some(stream);
                    }
                    if let Some(metadata) = long_metadata {
                        if let Some(active) = self.practice.as_mut() {
                            active.long_metadata = Some(metadata.clone());
                        }
                        self.retry_long_metadata = Some(metadata);
                    }
                }
            }
            Key::Char('n')
                if self.screen == Screen::Result && key.modifiers == KeyModifiers::NONE =>
            {
                self.start_next(now)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_practice_key(&mut self, key: KeyInput, now: Instant) -> Result<()> {
        if self
            .practice
            .as_ref()
            .is_some_and(|active| active.kind() == PracticeKind::Test)
        {
            if key.kind == KeyKind::Press && key.key == Key::Esc {
                if let Some(active) = self.practice.as_mut() {
                    active.leave_confirmation = !active.leave_confirmation;
                }
                return Ok(());
            }
            if self
                .practice
                .as_ref()
                .is_some_and(ActivePractice::leave_confirmation)
            {
                if key.kind == KeyKind::Press
                    && key.key == Key::Char('q')
                    && key.modifiers == KeyModifiers::NONE
                {
                    let attempted = self
                        .practice
                        .as_ref()
                        .is_some_and(|active| active.engine.attempted_units() != 0);
                    if attempted {
                        self.finish_practice(now)?;
                    } else {
                        self.practice = None;
                        self.result = None;
                        self.return_home();
                    }
                }
                return Ok(());
            }
        }

        let pause = key.kind == KeyKind::Press
            && (key.key == Key::Esc
                || (matches!(key.key, Key::Char('p' | 'P'))
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
            if key.kind == KeyKind::Press
                && key.key == Key::Char('q')
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
        match key.key {
            Key::Backspace if matches!(key.kind, KeyKind::Press | KeyKind::Repeat) => {
                if let Some(active) = self.practice.as_mut()
                    && active.engine.backspace()
                {
                    active.live_metrics = active.engine.metrics(now);
                    active.current_item_delta =
                        Some(item_delta(&active.item_metrics, &active.live_metrics));
                }
            }
            Key::Char(character)
                if matches!(key.kind, KeyKind::Press | KeyKind::Repeat)
                    && matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) =>
            {
                self.input_practice(character.encode_utf8(&mut [0; 4]), now)?;
            }
            Key::Enter
                if matches!(key.kind, KeyKind::Press | KeyKind::Repeat)
                    && matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) =>
            {
                self.submit_practice_line(now)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn input_practice(&mut self, text: &str, now: Instant) -> Result<()> {
        self.apply_practice_input(Some(text), now)
    }

    fn submit_practice_line(&mut self, now: Instant) -> Result<()> {
        self.apply_practice_input(None, now)
    }

    fn apply_practice_input(&mut self, text: Option<&str>, now: Instant) -> Result<()> {
        let Some(active) = self.practice.as_mut() else {
            return Ok(());
        };
        if let Some(language) = text.and_then(input_language) {
            active.observed_input_language = Some(language);
        }
        let wall_now = OffsetDateTime::now_utc();
        let attempted_before = active.engine.attempted_units();
        let errors_before = active.live_metrics.errors;
        match text {
            Some(text) => active.engine.input(text, now),
            None => active.engine.submit_line(now),
        };
        if active.started_at_utc.is_none() && active.engine.attempted_units() > attempted_before {
            active.started_at_utc = Some(wall_now);
        }
        active.live_metrics = active.engine.metrics(now);
        if active.engine.attempted_units() > attempted_before {
            active.current_item_delta =
                Some(item_delta(&active.item_metrics, &active.live_metrics));
        }
        if active.live_metrics.errors > errors_before
            && let PracticeMode::Words { streak, .. } = &mut active.mode
        {
            *streak = 0;
        }
        self.advance_item_boundaries(now)?;
        if self.practice.as_ref().is_some_and(|active| {
            matches!(active.stop, StopRule::TargetOrActiveTime(_))
                && active.engine.target_complete()
        }) {
            self.finish_practice(now)?;
        }
        Ok(())
    }

    fn advance_item_boundaries(&mut self, now: Instant) -> Result<()> {
        let mut advanced = false;
        if let Some(active) = self.practice.as_mut() {
            while let Some(end) = active.item_ends.get(active.next_item).copied() {
                if active.engine.cursor() < end {
                    break;
                }

                let delta = item_delta(&active.item_metrics, &active.live_metrics);
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
                    PracticeMode::Long { paragraph, .. } => {
                        *paragraph = paragraph.saturating_add(1);
                    }
                    PracticeMode::Key { .. } | PracticeMode::Test { .. } => {}
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
        let Some((stream, excluded_id)) = self.practice.as_ref().and_then(|active| {
            let remaining = active.item_ends.len().saturating_sub(active.next_item);
            (matches!(active.stop, StopRule::ActiveTime(_)) && remaining < 10)
                .then(|| active.stream.clone())
                .flatten()
                .map(|stream| {
                    let excluded_id = (stream.kinds == TEXT_KINDS)
                        .then(|| active.content_ids.last().cloned())
                        .flatten();
                    (stream, excluded_id)
                })
        }) else {
            return Ok(());
        };
        let count = if stream.kinds == TEXT_KINDS {
            1
        } else {
            STREAM_BATCH_ITEMS
        };
        let items = select_catalog_items(
            &self.content,
            &self.sessions,
            &stream,
            count,
            stream.next_seed,
            excluded_id.as_deref(),
        )?;
        let (target, relative_ends, content_ids) = catalog_target(&items, stream.separator);
        let Some(active) = self.practice.as_mut() else {
            return Ok(());
        };
        let separator_len = UnicodeSegmentation::graphemes(stream.separator, true).count();
        let offset = active.engine.target_len() + separator_len;
        active
            .engine
            .extend_target(stream.separator, &target, &relative_ends)?;
        if let Some(end) = active.item_ends.last_mut() {
            *end += separator_len;
        }
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
        self.poll_update();
        if let Some(active) = self.practice.as_mut() {
            active.live_metrics = active.engine.metrics(now);
            let item_start = active
                .next_item
                .checked_sub(1)
                .and_then(|index| active.item_ends.get(index))
                .copied()
                .unwrap_or(0);
            if active.engine.cursor() > item_start {
                active.current_item_delta =
                    Some(item_delta(&active.item_metrics, &active.live_metrics));
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
                    StopRule::TargetOrActiveTime(_) => {
                        active.engine.target_complete() || active.engine.time_limit_reached(now)
                    }
                });
        if finished {
            self.finish_practice(now)?;
        }
        Ok(())
    }

    fn escape(&mut self) {
        self.content_disable_confirmation = false;
        self.remember_focus();
        match self.screen {
            Screen::Home => self.quit = true,
            Screen::Result => self.return_home(),
            Screen::Help => {
                let destination = self.parent;
                let restored_parent = self.parent_before_help.take().unwrap_or(Screen::Home);
                self.parent = if restored_parent == destination {
                    Screen::Home
                } else {
                    restored_parent
                };
                self.restore_focus(destination);
            }
            _ => {
                let destination = if self.parent == self.screen {
                    Screen::Home
                } else {
                    self.parent
                };
                self.parent = Screen::Home;
                self.parent_before_help = None;
                self.restore_focus(destination);
            }
        }
    }

    fn return_home(&mut self) {
        self.remember_focus();
        self.parent = Screen::Home;
        self.parent_before_help = None;
        self.restore_focus(Screen::Home);
    }

    fn remember_focus(&mut self) {
        self.focus_memory.insert(self.screen, self.focus);
    }

    fn restore_focus(&mut self, screen: Screen) {
        self.screen = screen;
        self.focus = self
            .focus_memory
            .get(&screen)
            .copied()
            .unwrap_or(0)
            .min(self.focus_count().saturating_sub(1));
    }

    fn focus_count(&self) -> usize {
        match self.screen {
            Screen::Home => 10,
            Screen::ModeOptions => match self.mode_options.kind {
                PracticeKind::Quick | PracticeKind::Key => 5,
                PracticeKind::Words => 3,
                PracticeKind::Sentence => 2,
                PracticeKind::Test => 4,
                PracticeKind::Long => self
                    .long_items(self.mode_options.language, None)
                    .len()
                    .saturating_add(1),
            },
            Screen::Stats => 5,
            Screen::History => 3,
            Screen::Goals => 4,
            Screen::Content => self.content_packs().len().max(1),
            Screen::ContentDetail => self
                .content_detail_pack()
                .map_or(1, |pack| pack.provenance.len().max(1)),
            Screen::Settings => 9,
            Screen::Themes => self.themes.ids().count().max(1),
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
        if self.screen == Screen::ModeOptions
            && self.mode_options.kind == PracticeKind::Long
            && self.focus != 0
        {
            self.mode_options.long_selection = self.focus - 1;
        }
    }

    fn adjust(&mut self, delta: isize) {
        if self.screen == Screen::ModeOptions {
            self.adjust_mode_options(delta);
            return;
        }
        match (self.screen, self.focus) {
            (Screen::Stats | Screen::History, 0) => {
                self.stats_range = cycle_range(self.stats_range, delta);
            }
            (Screen::Stats | Screen::History, 1) => {
                self.stats_language = other_language(self.stats_language);
            }
            (Screen::Stats | Screen::History, 2) => {
                self.stats_mode = cycle_mode(self.stats_mode, delta);
            }
            (Screen::Goals, 0) => {
                let value = adjusted(self.settings.target_kpm, delta, 10, 1, 5_000);
                let _ = self.set_target_kpm(value);
            }
            (Screen::Goals, 1) => {
                let value = adjusted(self.settings.target_wpm, delta, 5, 1, 5_000);
                let _ = self.set_target_wpm(value);
            }
            (Screen::Goals, 2) => {
                let value = adjusted_decimal(self.settings.target_accuracy, delta, 0.5, 1.0, 100.0);
                let _ = self.set_target_accuracy(value);
            }
            (Screen::Goals, 3) => {
                let value = adjusted(self.settings.daily_minutes, delta, 5, 1, 1_440);
                let _ = self.set_daily_minutes(value);
            }
            (Screen::Settings, _) => self.activate_setting(),
            _ => {}
        }
    }

    fn adjust_mode_options(&mut self, delta: isize) {
        if self.focus == 0 {
            self.mode_options.language = other_language(self.mode_options.language);
            match self.mode_options.kind {
                PracticeKind::Key => {
                    self.mode_options.key_stage = self
                        .mode_options
                        .key_stage
                        .min(key_stages(self.mode_options.language).len() as u8);
                }
                PracticeKind::Long => {
                    let item_count = self.long_items(self.mode_options.language, None).len();
                    self.mode_options.long_selection = self
                        .mode_options
                        .long_selection
                        .min(item_count.saturating_sub(1));
                    self.focus = self.focus.min(item_count);
                }
                PracticeKind::Test => {
                    self.mode_options.test_selection = self
                        .mode_options
                        .test_selection
                        .min(self.long_items(self.mode_options.language, None).len());
                }
                _ => {}
            }
            return;
        }

        match (self.mode_options.kind, self.focus) {
            (PracticeKind::Quick, 1) => {
                self.mode_options.quick_source = match self.mode_options.quick_source {
                    QuickSource::Words => QuickSource::Quote,
                    QuickSource::Quote => QuickSource::Words,
                };
            }
            (PracticeKind::Quick, 2) => {
                self.mode_options.quick_items = !self.mode_options.quick_items;
            }
            (PracticeKind::Quick, 3) => {
                let presets = if self.mode_options.quick_items {
                    QUICK_COUNT_PRESETS.len()
                } else {
                    QUICK_TIME_PRESETS.len()
                };
                self.mode_options.quick_preset =
                    cycle_index(self.mode_options.quick_preset, presets, delta);
            }
            (PracticeKind::Key, 1) => {
                self.mode_options.key_stage = (cycle_index(
                    usize::from(self.mode_options.key_stage.saturating_sub(1)),
                    key_stages(self.mode_options.language).len(),
                    delta,
                ) + 1) as u8;
            }
            (PracticeKind::Key, 2) => self.mode_options.key_random = !self.mode_options.key_random,
            (PracticeKind::Key, 3) => {
                self.mode_options.key_weak_repeat = !self.mode_options.key_weak_repeat;
            }
            (PracticeKind::Words, 1) => {
                self.mode_options.word_difficulty =
                    cycle_difficulty(self.mode_options.word_difficulty, delta);
            }
            (PracticeKind::Test, 1) => {
                self.mode_options.test_preset = cycle_index(
                    self.mode_options.test_preset,
                    TEST_DURATION_PRESETS.len(),
                    delta,
                );
            }
            (PracticeKind::Test, 2) => {
                self.mode_options.test_selection = cycle_index(
                    self.mode_options.test_selection,
                    self.long_items(self.mode_options.language, None).len() + 1,
                    delta,
                );
            }
            _ => {}
        }
    }

    fn enter(&mut self, now: Instant) -> Result<()> {
        match self.screen {
            Screen::Home => {
                let kinds = [
                    PracticeKind::Quick,
                    PracticeKind::Key,
                    PracticeKind::Words,
                    PracticeKind::Sentence,
                    PracticeKind::Long,
                    PracticeKind::Test,
                ];
                match self.focus {
                    0..=5 => {
                        self.mode_options =
                            ModeOptions::new(kinds[self.focus], self.settings.language);
                        self.open(Screen::ModeOptions);
                    }
                    6 => self.open(Screen::Stats),
                    7 => self.open(Screen::Goals),
                    8 => self.open(Screen::Content),
                    9 => self.open(Screen::Settings),
                    _ => {}
                }
            }
            Screen::ModeOptions => {
                let options = self.mode_options.clone();
                match (options.kind, self.focus) {
                    (PracticeKind::Quick, 4) => self.start_quick(
                        QuickOptions::new(
                            options.language,
                            options.quick_source,
                            options.quick_stop(),
                        )?,
                        fastrand::u64(..),
                        now,
                    )?,
                    (PracticeKind::Key, 4) => self.start_key(
                        options.language,
                        options.key_stage,
                        options.key_random,
                        options.key_weak_repeat,
                        fastrand::u64(..),
                        now,
                    )?,
                    (PracticeKind::Words, 2) => self.start_words(
                        options.language,
                        options.word_difficulty,
                        fastrand::u64(..),
                        now,
                    )?,
                    (PracticeKind::Sentence, 1) => {
                        self.start_sentence(options.language, fastrand::u64(..), now)?;
                    }
                    (PracticeKind::Long, focus) if focus != 0 => {
                        let item_id = self
                            .long_items(options.language, None)
                            .get(options.long_selection)
                            .map(|item| item.id.clone());
                        if let Some(item_id) = item_id {
                            self.start_long(&item_id, now)?;
                        }
                    }
                    (PracticeKind::Test, 3) => {
                        let items = self.long_items(options.language, None);
                        let item_id = options
                            .test_selection
                            .checked_sub(1)
                            .and_then(|index| items.get(index))
                            .map(|item| item.id.clone());
                        self.start_test(
                            options.language,
                            Some(TEST_DURATION_PRESETS[options.test_preset]),
                            item_id.as_deref(),
                            fastrand::u64(..),
                            now,
                        )?;
                    }
                    _ => self.adjust(1),
                }
            }
            Screen::Stats => match self.focus {
                0..=2 => self.adjust(1),
                3 => self.open(Screen::History),
                4 => self.open(Screen::WeakKeys),
                _ => {}
            },
            Screen::History => self.adjust(1),
            Screen::Goals => self.adjust(1),
            Screen::Content => {
                if let Some(pack) = self.content_packs().get(self.focus) {
                    self.selected_content_pack = Some(pack.id.clone());
                    self.content_disable_confirmation = false;
                    self.open(Screen::ContentDetail);
                }
            }
            Screen::Settings => self.activate_setting(),
            Screen::Themes => {
                let id = self.themes.ids().nth(self.focus).map(str::to_owned);
                if let Some(id) = id
                    && self.select_theme(&id).is_ok()
                {
                    self.escape();
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn activate_setting(&mut self) {
        match self.focus {
            0 => {
                let language = other_language(self.settings.language);
                if self
                    .change_settings(|settings| settings.language = language)
                    .is_ok()
                {
                    self.stats_language = language;
                }
            }
            1 => {
                let language = other_language(self.settings.ui_language);
                let _ = self.change_settings(|settings| settings.ui_language = language);
            }
            2 => self.open(Screen::Themes),
            3 => {
                let value = !self.settings.show_keyboard;
                let _ = self.change_settings(|settings| settings.show_keyboard = value);
            }
            4 => {
                let value = !self.settings.show_finger_guide;
                let _ = self.change_settings(|settings| settings.show_finger_guide = value);
            }
            5 => {
                let value = !self.settings.show_live_speed;
                let _ = self.change_settings(|settings| settings.show_live_speed = value);
            }
            6 => {
                let value = !self.settings.show_accuracy;
                let _ = self.change_settings(|settings| settings.show_accuracy = value);
            }
            7 => {
                let value = !self.settings.adaptive;
                let _ = self.change_settings(|settings| settings.adaptive = value);
            }
            8 => {
                let value = !self.settings.check_updates;
                let _ = self.change_settings(|settings| settings.check_updates = value);
            }
            _ => {}
        }
    }
}

const fn other_language(language: Language) -> Language {
    match language {
        Language::Ko => Language::En,
        Language::En => Language::Ko,
    }
}

fn cycle_range(range: Range, delta: isize) -> Range {
    const VALUES: [Range; 4] = [Range::Days7, Range::Days30, Range::Days90, Range::All];
    let index = VALUES.iter().position(|value| *value == range).unwrap_or(0);
    VALUES[cycle_index(index, VALUES.len(), delta)]
}

fn cycle_mode(mode: Option<PracticeKind>, delta: isize) -> Option<PracticeKind> {
    const VALUES: [Option<PracticeKind>; 7] = [
        None,
        Some(PracticeKind::Quick),
        Some(PracticeKind::Key),
        Some(PracticeKind::Words),
        Some(PracticeKind::Sentence),
        Some(PracticeKind::Long),
        Some(PracticeKind::Test),
    ];
    let index = VALUES.iter().position(|value| *value == mode).unwrap_or(0);
    VALUES[cycle_index(index, VALUES.len(), delta)]
}

fn cycle_index(index: usize, len: usize, delta: isize) -> usize {
    if delta < 0 {
        (index + len - 1) % len
    } else {
        (index + 1) % len
    }
}

fn adjusted(value: u32, delta: isize, step: u32, minimum: u32, maximum: u32) -> u32 {
    let value = value.clamp(minimum, maximum);
    let next = if delta < 0 {
        if value.is_multiple_of(step) {
            value.saturating_sub(step)
        } else {
            value / step * step
        }
    } else if delta > 0 {
        (value / step * step).saturating_add(step)
    } else {
        value
    };
    next.clamp(minimum, maximum)
}

fn adjusted_decimal(value: f64, delta: isize, step: f64, minimum: f64, maximum: f64) -> f64 {
    let units = value / step;
    let next = if delta < 0 {
        units.ceil() - 1.0
    } else if delta > 0 {
        units.floor() + 1.0
    } else {
        units
    };
    (next * step).clamp(minimum, maximum)
}

fn cycle_difficulty(difficulty: Difficulty, delta: isize) -> Difficulty {
    let values = [
        Difficulty::Easy,
        Difficulty::Medium,
        Difficulty::Hard,
        Difficulty::Mixed,
    ];
    let index = values
        .iter()
        .position(|value| *value == difficulty)
        .unwrap_or_default();
    values[cycle_index(index, values.len(), delta)]
}

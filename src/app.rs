use crate::{
    cli::{disable_user_pack, format_content_error},
    config::Settings,
    content::{
        ContentCatalog, ContentKind, MAX_CONTENT_BYTES, ResolvedItem, SourceMeta, parse_pack,
        read_pack_bytes, validate_pack,
    },
    i18n::{TextKey, text},
    model::{Difficulty, Language, PracticeKind},
    practice::{Metrics, PracticeEngine},
    stats::{
        KeyAccuracy, ProgressPoint, Range, adaptive_candidates, intended_key_counts, progress,
        weak_keys,
    },
    storage::{AppPaths, SessionRecord, save_session},
    theme::ThemeCatalog,
    typing::input_language,
    update::UpdateNotice,
};
use anyhow::{Result, anyhow, bail};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::Path,
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};
use time::{Date, OffsetDateTime, UtcOffset};
use unicode_segmentation::UnicodeSegmentation;

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

    pub fn content_packs(&self) -> &[ContentPackSummary] {
        &self.content_pack_summaries
    }

    pub fn selected_content_pack(&self) -> Option<&str> {
        self.selected_content_pack.as_deref()
    }

    pub fn content_detail_pack(&self) -> Option<&ContentPackSummary> {
        self.selected_content_pack
            .as_deref()
            .and_then(|id| {
                self.content_pack_summaries
                    .iter()
                    .find(|pack| pack.id == id)
            })
            .or_else(|| self.content_pack_summaries.first())
    }

    pub const fn content_disable_confirmation(&self) -> bool {
        self.content_disable_confirmation
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

    pub fn start_mode(&mut self, request: ModeRequest, now: Instant) -> Result<()> {
        if request.mode.kind() != request.kind {
            bail!("practice mode does not match requested kind");
        }
        let limit = match request.stop {
            StopRule::ActiveTime(duration) => Some(duration),
            StopRule::TargetEnd | StopRule::Items(_) => None,
        };
        let engine = PracticeEngine::new_for_items(
            request.language,
            request.kind,
            request.target.as_str(),
            &request.item_ends,
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
            observed_input_language: None,
            started_at_utc: None,
            live_metrics: metrics.clone(),
            item_metrics: metrics,
            next_item: 0,
            current_item_delta: None,
            sentence_delta_expires_at: None,
            stream: None,
            long_metadata: None,
            leave_confirmation: false,
        };

        self.remember_focus();
        self.screen = Screen::Practice;
        self.parent = Screen::Home;
        self.parent_before_help = None;
        self.focus = 0;
        self.retry_request = Some(retry_request);
        self.retry_stream = None;
        self.retry_long_metadata = None;
        self.practice = Some(active);
        self.result = None;
        Ok(())
    }

    pub fn start_default(
        &mut self,
        kind: PracticeKind,
        language: Language,
        seconds: Option<u64>,
        seed: u64,
        now: Instant,
    ) -> Result<()> {
        match kind {
            PracticeKind::Quick => self.start_quick(
                QuickOptions::new(
                    language,
                    QuickSource::Words,
                    StopRule::ActiveTime(Duration::from_secs(seconds.unwrap_or(30))),
                )?,
                seed,
                now,
            ),
            PracticeKind::Key => self.start_key(language, 1, false, false, seed, now),
            PracticeKind::Words => self.start_words(language, Difficulty::Mixed, seed, now),
            PracticeKind::Sentence => self.start_sentence(language, seed, now),
            PracticeKind::Long => {
                let item_id = self
                    .long_items(language, None)
                    .first()
                    .map(|item| item.id.clone())
                    .ok_or_else(|| anyhow!("no long-text content for {language:?}"))?;
                self.start_long(&item_id, now)
            }
            PracticeKind::Test => self.start_test(language, seconds, seed, now),
        }
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
        }
        self.retry_stream = Some(stream);
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
        self.start_mode(request, now)?;
        self.retry_stream = Some(stream);
        Ok(())
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
        self.start_mode(request, now)?;
        self.retry_stream = Some(stream);
        Ok(())
    }

    pub fn long_items(&self, language: Language, category: Option<&str>) -> Vec<&ResolvedItem> {
        self.content
            .items()
            .filter(|item| {
                item.language == language
                    && item.kind == ContentKind::Text
                    && category.is_none_or(|tag| item.tags.iter().any(|item_tag| item_tag == tag))
            })
            .collect()
    }

    pub fn start_long(&mut self, item_id: &str, now: Instant) -> Result<()> {
        let Some(item) = self
            .content
            .items()
            .find(|item| item.id == item_id && item.kind == ContentKind::Text)
            .cloned()
        else {
            bail!("unknown long-text item");
        };
        let item_id = item.id;
        let target = item.text;
        let metadata = LongMetadata {
            title: item.title.unwrap_or_else(|| item_id.clone()),
            author: item.source.author,
            source: item.source.source_url,
            license: item.source.license,
            difficulty: item.difficulty,
            tags: item.tags,
            custom_source: None,
        };
        let item_ends = paragraph_ends(&target);
        self.start_mode(
            ModeRequest {
                kind: PracticeKind::Long,
                language: item.language,
                target,
                mode: PracticeMode::Long {
                    item_id: item_id.clone(),
                    paragraph: 0,
                },
                stop: StopRule::TargetEnd,
                item_ends,
                content_ids: vec![item_id],
            },
            now,
        )?;
        if let Some(active) = self.practice.as_mut() {
            active.long_metadata = Some(metadata.clone());
        }
        self.retry_long_metadata = Some(metadata);
        Ok(())
    }

    pub fn start_custom_text(
        &mut self,
        source: CustomTextSource,
        name: &str,
        text: &str,
        now: Instant,
    ) -> Result<()> {
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            bail!("custom text name must be visible");
        }
        if text.len() > MAX_CONTENT_BYTES {
            bail!("custom text exceeds the 8 MiB limit");
        }
        let text = text.replace("\r\n", "\n");
        if text.trim().is_empty()
            || text
                .chars()
                .any(|character| character != '\n' && character.is_control())
        {
            bail!("custom text is empty or contains a disallowed control character");
        }
        let metadata = LongMetadata {
            title: name.into(),
            author: match source {
                CustomTextSource::File => "Local file",
                CustomTextSource::Stdin => "Standard input",
            }
            .into(),
            source: "User-provided text".into(),
            license: "Not redistributed".into(),
            difficulty: None,
            tags: Vec::new(),
            custom_source: Some(source),
        };
        let item_ends = paragraph_ends(&text);
        self.start_mode(
            ModeRequest {
                kind: PracticeKind::Long,
                language: self.settings.language,
                target: text,
                mode: PracticeMode::Long {
                    item_id: source.content_id().into(),
                    paragraph: 0,
                },
                stop: StopRule::TargetEnd,
                item_ends,
                content_ids: vec![source.content_id().into()],
            },
            now,
        )?;
        if let Some(active) = self.practice.as_mut() {
            active.long_metadata = Some(metadata.clone());
        }
        self.retry_long_metadata = Some(metadata);
        Ok(())
    }

    pub fn start_test(
        &mut self,
        language: Language,
        seconds: Option<u64>,
        seed: u64,
        now: Instant,
    ) -> Result<()> {
        let seconds = seconds.unwrap_or(300);
        if !TEST_DURATION_PRESETS.contains(&seconds) {
            bail!("invalid typing-test duration");
        }
        let stream = CatalogStream {
            language,
            kinds: SENTENCE_KINDS,
            difficulty: Difficulty::Mixed,
            separator: "\n",
            next_seed: seed.wrapping_add(1),
            adaptive: false,
        };
        let request = self.catalog_request(
            PracticeMode::Test { grade: None },
            StopRule::ActiveTime(Duration::from_secs(seconds)),
            &stream,
            SENTENCE_BATCH_ITEMS,
            seed,
        )?;
        self.start_mode(request, now)?;
        if let Some(active) = self.practice.as_mut() {
            active.stream = Some(stream.clone());
        }
        self.retry_stream = Some(stream);
        Ok(())
    }

    pub fn long_metadata(&self) -> Option<&LongMetadata> {
        self.practice
            .as_ref()
            .and_then(ActivePractice::long_metadata)
    }

    pub fn long_scroll(&self) -> Option<LongScroll> {
        self.practice.as_ref().and_then(ActivePractice::long_scroll)
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

    pub fn can_start_next(&self) -> bool {
        let Some(request) = self.retry_request.as_ref() else {
            return false;
        };
        match &request.mode {
            PracticeMode::Quick { .. }
            | PracticeMode::Words { .. }
            | PracticeMode::Sentence { .. } => self.retry_stream.as_ref().is_some_and(|stream| {
                stream.language == request.language
                    && self.content.items().any(|item| catalog_match(item, stream))
            }),
            PracticeMode::Long { item_id, .. } => {
                self.retry_long_metadata
                    .as_ref()
                    .is_some_and(|metadata| metadata.custom_source.is_none())
                    && self
                        .long_items(request.language, None)
                        .iter()
                        .any(|item| item.id == *item_id)
            }
            PracticeMode::Key { .. } | PracticeMode::Test { .. } => false,
        }
    }

    fn start_next(&mut self, now: Instant) -> Result<()> {
        if !self.can_start_next() {
            return Ok(());
        }
        let Some(request) = self.retry_request.clone() else {
            return Ok(());
        };
        if let PracticeMode::Long { item_id, .. } = &request.mode {
            let items = self.long_items(request.language, None);
            let Some(index) = items.iter().position(|item| item.id == *item_id) else {
                return Ok(());
            };
            let next_id = items[(index + 1) % items.len()].id.clone();
            return self.start_long(&next_id, now);
        }

        let Some(mut stream) = self.retry_stream.clone() else {
            return Ok(());
        };
        let (mode, count) = match request.mode {
            PracticeMode::Quick { .. } => {
                let count = match request.stop {
                    StopRule::Items(items) => items,
                    StopRule::ActiveTime(_) => STREAM_BATCH_ITEMS,
                    StopRule::TargetEnd => return Ok(()),
                };
                (PracticeMode::Quick { completed: 0 }, count)
            }
            PracticeMode::Words { difficulty, .. } => (
                PracticeMode::Words {
                    difficulty,
                    completed: 0,
                    streak: 0,
                },
                WORD_BATCH_ITEMS,
            ),
            PracticeMode::Sentence { .. } => (
                PracticeMode::Sentence {
                    completed: 0,
                    last_item: None,
                },
                SENTENCE_BATCH_ITEMS,
            ),
            PracticeMode::Key { .. } | PracticeMode::Long { .. } | PracticeMode::Test { .. } => {
                return Ok(());
            }
        };
        let seed = stream.next_seed;
        stream.next_seed = seed.wrapping_add(1);
        let timed = matches!(request.stop, StopRule::ActiveTime(_));
        let request = self.catalog_request(mode, request.stop, &stream, count, seed)?;
        self.start_mode(request, now)?;
        if timed && let Some(active) = self.practice.as_mut() {
            active.stream = Some(stream.clone());
        }
        self.retry_stream = Some(stream);
        Ok(())
    }

    pub fn handle_event(&mut self, event: Event, now: Instant) -> Result<()> {
        let update_notice_was_visible = self.update_notice.is_some();
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
                if let Some(active) = self.practice.as_mut()
                    && !(active.kind() == PracticeKind::Test && active.leave_confirmation())
                {
                    active.status = Some((
                        text(self.settings.ui_language, TextKey::PasteIgnored).into(),
                        now.checked_add(Duration::from_secs(3)).unwrap_or(now),
                    ));
                }
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                self.handle_key(key, now, update_notice_was_visible)?;
            }
            _ => {}
        }
        self.tick(now)
    }

    fn handle_key(
        &mut self,
        key: KeyEvent,
        now: Instant,
        update_notice_was_visible: bool,
    ) -> Result<()> {
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
        if matches!(self.screen, Screen::Home | Screen::Result)
            && key.kind == KeyEventKind::Press
            && key.modifiers == KeyModifiers::NONE
            && update_notice_was_visible
            && self.update_notice.is_some()
        {
            match key.code {
                KeyCode::Char('l') => {
                    self.update_notice = None;
                    return Ok(());
                }
                KeyCode::Char('s') => {
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
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => self.adjust(-1),
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => self.adjust(1),
            KeyCode::Char('d')
                if self.screen == Screen::ContentDetail
                    && key.kind == KeyEventKind::Press
                    && key.modifiers == KeyModifiers::NONE =>
            {
                self.disable_selected_content();
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => self.enter(now)?,
            KeyCode::Char('r')
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
            KeyCode::Char('n')
                if self.screen == Screen::Result && key.modifiers == KeyModifiers::NONE =>
            {
                self.start_next(now)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_practice_key(&mut self, key: KeyEvent, now: Instant) -> Result<()> {
        if self
            .practice
            .as_ref()
            .is_some_and(|active| active.kind() == PracticeKind::Test)
        {
            if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc {
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
                if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Char('q')
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
                    if active.engine.backspace() {
                        active.live_metrics = active.engine.metrics(now);
                        active.current_item_delta =
                            Some(item_delta(&active.item_metrics, &active.live_metrics));
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
        self.advance_item_boundaries(now)
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
        if let Some(stream) = active.stream.clone() {
            self.retry_stream = Some(stream);
        }
        let metrics = active.engine.finalize(now);
        let language = active.engine.language();
        let kind = active.kind();
        let long = (kind == PracticeKind::Long).then(|| {
            let completed_graphemes = active.engine.cursor();
            let total_graphemes = active.engine.target_len();
            let (best_rolling_kpm, best_rolling_wpm) = active.engine.best_rolling_speeds();
            LongOutcome {
                best_rolling_kpm,
                best_rolling_wpm,
                completed_graphemes,
                total_graphemes,
                percent: completed_graphemes.saturating_mul(100) / total_graphemes,
            }
        });
        let started_at = active
            .started_at_utc
            .unwrap_or_else(OffsetDateTime::now_utc);
        let content_id = active
            .content_ids
            .first()
            .cloned()
            .unwrap_or_else(|| practice_id(kind).into());
        let long_difficulty = active
            .long_metadata
            .as_ref()
            .and_then(|metadata| metadata.difficulty);
        let difficulty = match active.mode {
            PracticeMode::Words { difficulty, .. } => match difficulty {
                Difficulty::Easy => Some(1),
                Difficulty::Medium => Some(2),
                Difficulty::Hard => Some(3),
                Difficulty::Mixed => None,
            },
            PracticeMode::Long { .. } => long_difficulty,
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
        let previous = comparable
            .iter()
            .copied()
            .filter(|prior| prior.kpm.is_finite() && prior.wpm.is_finite())
            .max_by(|left, right| {
                left.started_at_unix_ms
                    .cmp(&right.started_at_unix_ms)
                    .then_with(|| left.id.cmp(&right.id))
            });
        let previous_kpm = previous.map(|prior| prior.kpm);
        let previous_wpm = previous.map(|prior| prior.wpm);
        let best_kpm = comparable
            .iter()
            .map(|prior| prior.kpm)
            .filter(|speed| speed.is_finite())
            .max_by(f64::total_cmp);
        let best_wpm = comparable
            .iter()
            .map(|prior| prior.wpm)
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
            previous_kpm,
            previous_wpm,
            best_kpm,
            best_wpm,
            kpm_delta: previous_kpm.map(|previous| session.kpm - previous),
            wpm_delta: previous_wpm.map(|previous| session.wpm - previous),
            speed_goal,
            accuracy_goal: self.settings.target_accuracy,
            daily_minutes_goal: self.settings.daily_minutes,
            speed_goal_met: speed >= speed_goal,
            accuracy_goal_met: session.accuracy >= self.settings.target_accuracy,
            daily_minutes_met: prior_duration.saturating_add(session.duration_ms) >= daily_target,
            weak_keys: weak_keys(&session.intended_keys, 1)
                .into_iter()
                .take(5)
                .collect(),
            grade: result_grade,
            save_error: None,
            long,
            session,
        };
        match save_session(&self.paths, &view.session) {
            Ok(_) => self.sessions.push(view.session.clone()),
            Err(error) => view.save_error = Some(error.root_cause().to_string()),
        }

        self.remember_focus();
        self.screen = Screen::Result;
        self.parent = Screen::Home;
        self.parent_before_help = None;
        self.focus = 0;
        self.result = Some(view.clone());
        Ok(view)
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
                PracticeKind::Words | PracticeKind::Test => 3,
                PracticeKind::Sentence => 2,
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
                    (PracticeKind::Test, 2) => self.start_test(
                        options.language,
                        Some(TEST_DURATION_PRESETS[options.test_preset]),
                        fastrand::u64(..),
                        now,
                    )?,
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

    fn disable_selected_content(&mut self) {
        let Some(id) = self.selected_content_pack.clone() else {
            return;
        };
        let Some(pack) = self.content_packs().iter().find(|pack| pack.id == id) else {
            return;
        };
        if pack.built_in {
            self.warnings
                .push(format!("content: built-in pack {id:?} cannot be disabled"));
            return;
        }
        if !pack.enabled {
            self.warnings
                .push(format!("content: user pack {id:?} is already disabled"));
            return;
        }
        if !self.content_disable_confirmation {
            self.content_disable_confirmation = true;
            return;
        }
        let mut mutation_warnings = Vec::new();
        let result = disable_user_pack(&self.paths, &id, &mut mutation_warnings);
        self.warnings.extend(
            mutation_warnings
                .iter()
                .map(|warning| format!("content: {}", format_content_error(warning))),
        );
        match result {
            Ok(catalog) => {
                self.content_pack_summaries = collect_content_packs(&catalog, &self.paths.content);
                self.content = catalog;
                self.selected_content_pack = None;
                self.content_disable_confirmation = false;
                self.escape();
            }
            Err(error) => {
                self.warnings.push(format!("content: {error:#}"));
                self.content_disable_confirmation = false;
            }
        }
    }
}

fn collect_content_packs(catalog: &ContentCatalog, content_root: &Path) -> Vec<ContentPackSummary> {
    let mut packs = BTreeMap::<String, ContentPackSummary>::new();
    for item in catalog.items() {
        add_pack_item(
            &mut packs,
            item,
            true,
            catalog.active_user_path(&item.pack_id).is_none(),
        );
    }
    for pack in packs.values_mut() {
        if let Some(source) = catalog.pack_source(&pack.id) {
            if !pack.licenses.contains(&source.license) {
                pack.licenses.push(source.license.clone());
            }
            pack.provenance.push(ContentProvenance {
                item_id: None,
                source: source.clone(),
            });
        }
    }

    let disabled = content_root.join("disabled");
    let mut entries = match fs::symlink_metadata(&disabled) {
        Ok(metadata) if metadata.file_type().is_dir() => fs::read_dir(disabled)
            .map(|entries| entries.flatten().collect::<Vec<_>>())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    entries.sort_unstable_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("toml")
            || !entry.file_type().is_ok_and(|kind| kind.is_file())
        {
            continue;
        }
        let Ok(bytes) = read_pack_bytes(&path) else {
            continue;
        };
        let Ok(source) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let Ok(pack) = parse_pack(source) else {
            continue;
        };
        if !validate_pack(&pack).is_empty() || packs.contains_key(&pack.id) {
            continue;
        }
        let pack_id = pack.id.clone();
        let pack_source = pack.source.clone();
        let Ok(items) = pack.resolve_items() else {
            continue;
        };
        for item in items {
            add_pack_item(&mut packs, &item, false, false);
        }
        if let Some(pack) = packs.get_mut(&pack_id) {
            if !pack.licenses.contains(&pack_source.license) {
                pack.licenses.push(pack_source.license.clone());
            }
            pack.provenance.push(ContentProvenance {
                item_id: None,
                source: pack_source,
            });
        }
    }

    for pack in packs.values_mut() {
        pack.licenses.sort_unstable();
        pack.kinds
            .sort_unstable_by_key(|kind| content_kind_order(*kind));
    }
    packs.into_values().collect()
}

fn add_pack_item(
    packs: &mut BTreeMap<String, ContentPackSummary>,
    item: &ResolvedItem,
    enabled: bool,
    built_in: bool,
) {
    let pack = packs
        .entry(item.pack_id.clone())
        .or_insert_with(|| ContentPackSummary {
            id: item.pack_id.clone(),
            sample_item_id: item.id.clone(),
            provenance: Vec::new(),
            language: item.language,
            items: 0,
            licenses: Vec::new(),
            kinds: Vec::new(),
            enabled,
            built_in,
        });
    pack.items += 1;
    if !pack
        .provenance
        .iter()
        .any(|value| value.item_id.is_some() && value.source == item.source)
    {
        pack.provenance.push(ContentProvenance {
            item_id: Some(item.id.clone()),
            source: item.source.clone(),
        });
    }
    if !pack
        .licenses
        .iter()
        .any(|value| value == &item.source.license)
    {
        pack.licenses.push(item.source.license.clone());
    }
    if !pack.kinds.contains(&item.kind) {
        pack.kinds.push(item.kind);
    }
}

const fn content_kind_order(kind: ContentKind) -> u8 {
    match kind {
        ContentKind::Word => 0,
        ContentKind::Sentence => 1,
        ContentKind::Quote => 2,
        ContentKind::Text => 3,
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

const WORD_KINDS: &[ContentKind] = &[ContentKind::Word];
const QUOTE_KINDS: &[ContentKind] = &[ContentKind::Quote];
const SENTENCE_KINDS: &[ContentKind] = &[ContentKind::Sentence, ContentKind::Quote];
const STREAM_BATCH_ITEMS: usize = 20;
const WORD_BATCH_ITEMS: usize = 25;
const SENTENCE_BATCH_ITEMS: usize = 10;
const KEY_SEQUENCE_UNITS: usize = 120;

fn paragraph_ends(target: &str) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut count = 0;
    let mut newline_run = false;
    for grapheme in UnicodeSegmentation::graphemes(target, true) {
        if grapheme != "\n" && newline_run {
            ends.push(count);
            newline_run = false;
        }
        count += 1;
        newline_run |= grapheme == "\n";
    }
    if ends.last().copied() != Some(count) {
        ends.push(count);
    }
    ends
}

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

fn item_delta(before: &Metrics, after: &Metrics) -> ItemDelta {
    let correct_units = after.correct_units.saturating_sub(before.correct_units);
    let correct_cells = after.correct_cells.saturating_sub(before.correct_cells);
    let attempted_units = after.attempted_units.saturating_sub(before.attempted_units);
    let correct_attempts = correct_attempts(after).saturating_sub(correct_attempts(before));
    let minutes = after.active.saturating_sub(before.active).as_secs_f64() / 60.0;
    let kpm = if minutes > 0.0 {
        correct_units as f64 / minutes
    } else {
        0.0
    };
    ItemDelta {
        correct_units,
        attempted_units,
        errors: after.errors.saturating_sub(before.errors),
        kpm,
        wpm: if minutes > 0.0 {
            correct_cells as f64 / minutes / 5.0
        } else {
            0.0
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

use super::{
    ActivePractice, App, CatalogStream, CustomTextSource, KeyStage, LongMetadata, LongScroll,
    ModeRequest, PracticeMode, QuickOptions, QuickSource, Screen, StopRule, TEST_DURATION_PRESETS,
};
use crate::{
    content::{ContentCatalog, ContentKind, MAX_CONTENT_BYTES, ResolvedItem},
    model::{Difficulty, Language, PracticeKind},
    practice::PracticeEngine,
    stats::{adaptive_candidates, intended_key_counts, weak_keys},
    storage::SessionRecord,
};
use anyhow::{Result, anyhow, bail};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use unicode_segmentation::UnicodeSegmentation;

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

impl App {
    pub fn start_mode(&mut self, request: ModeRequest, now: Instant) -> Result<()> {
        if request.mode.kind() != request.kind {
            bail!("practice mode does not match requested kind");
        }
        let limit = match request.stop {
            StopRule::ActiveTime(duration) | StopRule::TargetOrActiveTime(duration) => {
                Some(duration)
            }
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
            PracticeKind::Test => self.start_test(language, seconds, None, seed, now),
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
            StopRule::TargetEnd | StopRule::TargetOrActiveTime(_) => {
                bail!("invalid Quick stop rule")
            }
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
        item_id: Option<&str>,
        seed: u64,
        now: Instant,
    ) -> Result<()> {
        let seconds = seconds.unwrap_or(300);
        if !TEST_DURATION_PRESETS.contains(&seconds) {
            bail!("invalid typing-test duration");
        }
        if let Some(item_id) = item_id {
            let Some(item) = self
                .content
                .items()
                .find(|item| {
                    item.id == item_id
                        && item.language == language
                        && item.kind == ContentKind::Text
                })
                .cloned()
            else {
                bail!("unknown typing-test text");
            };
            let item_ends = paragraph_ends(&item.text);
            return self.start_mode(
                ModeRequest {
                    kind: PracticeKind::Test,
                    language,
                    target: item.text,
                    mode: PracticeMode::Test { grade: None },
                    stop: StopRule::TargetOrActiveTime(Duration::from_secs(seconds)),
                    item_ends,
                    content_ids: vec![item.id],
                },
                now,
            );
        }
        let stream = CatalogStream {
            language,
            kinds: TEXT_KINDS,
            difficulty: Difficulty::Mixed,
            separator: "",
            next_seed: seed.wrapping_add(1),
            adaptive: false,
        };
        let request = self.catalog_request(
            PracticeMode::Test { grade: None },
            StopRule::ActiveTime(Duration::from_secs(seconds)),
            &stream,
            1,
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
        let items = select_catalog_items(&self.content, &self.sessions, stream, count, seed, None)?;
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

    pub(super) fn start_next(&mut self, now: Instant) -> Result<()> {
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
                    StopRule::TargetEnd | StopRule::TargetOrActiveTime(_) => return Ok(()),
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
}

const WORD_KINDS: &[ContentKind] = &[ContentKind::Word];
const QUOTE_KINDS: &[ContentKind] = &[ContentKind::Quote];
const SENTENCE_KINDS: &[ContentKind] = &[ContentKind::Sentence, ContentKind::Quote];
pub(super) const TEXT_KINDS: &[ContentKind] = &[ContentKind::Text];
pub(super) const STREAM_BATCH_ITEMS: usize = 20;
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

pub(super) fn select_catalog_items<'a>(
    catalog: &'a ContentCatalog,
    sessions: &[SessionRecord],
    stream: &CatalogStream,
    count: usize,
    seed: u64,
    excluded_id: Option<&str>,
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
        if let Some(excluded_id) = excluded_id
            && cycle.iter().any(|item| item.id != excluded_id)
        {
            cycle.retain(|item| item.id != excluded_id);
        }
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

pub(super) fn catalog_target(
    items: &[&ResolvedItem],
    separator: &str,
) -> (String, Vec<usize>, Vec<String>) {
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

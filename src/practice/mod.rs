use crate::{
    model::{Language, PracticeKind},
    typing::{key_units, normalize_nfc, unit_count},
};
use anyhow::{Result, bail};
use std::{
    collections::{BTreeMap, VecDeque},
    ops::Range,
    time::{Duration, Instant},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const LOGICAL_LINE_WIDTH: usize = 72;

#[derive(Clone, Debug)]
struct Cell {
    entered: String,
    correct: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub active: Duration,
    pub correct_cells: u64,
    pub correct_units: u64,
    pub attempted_units: u64,
    pub errors: u64,
    pub backspaces: u64,
    pub cpm: f64,
    pub kpm: f64,
    pub wpm: f64,
    pub accuracy: f64,
}

pub struct PracticeEngine {
    language: Language,
    kind: PracticeKind,
    target: String,
    target_ends: Vec<u32>,
    line_ends: Vec<u32>,
    active_line: usize,
    input: Vec<Cell>,
    started_at: Option<Instant>,
    finalized_at: Option<Instant>,
    paused_at: Option<Instant>,
    paused_total: Duration,
    limit: Option<Duration>,
    attempted_units: u64,
    correct_attempt_units: u64,
    correct_cells: u64,
    correct_units: u64,
    errors: u64,
    backspaces: u64,
    intended: BTreeMap<char, [u64; 2]>,
    rolling_samples: VecDeque<(Duration, u64, u64)>,
    best_rolling_kpm: f64,
    best_rolling_wpm: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputOutcome {
    Accepted,
    Finished,
    IgnoredWhilePaused,
}

impl PracticeEngine {
    pub fn new(
        language: Language,
        kind: PracticeKind,
        target: &str,
        limit: Option<Duration>,
    ) -> Result<Self> {
        let target = normalize_nfc(target);
        let item_end = UnicodeSegmentation::graphemes(target.as_str(), true).count();
        Self::from_target(language, kind, target, &[item_end], limit)
    }

    pub fn new_for_items(
        language: Language,
        kind: PracticeKind,
        target: &str,
        item_ends: &[usize],
        limit: Option<Duration>,
    ) -> Result<Self> {
        Self::from_target(language, kind, normalize_nfc(target), item_ends, limit)
    }

    fn from_target(
        language: Language,
        kind: PracticeKind,
        target: String,
        item_ends: &[usize],
        limit: Option<Duration>,
    ) -> Result<Self> {
        let target_ends = grapheme_ends(&target, 0)?;
        if target_ends.is_empty() {
            bail!("practice target cannot be empty");
        }
        validate_item_ends(item_ends, target_ends.len())?;
        let line_ends = logical_line_ends(kind, &target, &target_ends, item_ends)?;

        Ok(Self {
            language,
            kind,
            target,
            target_ends,
            line_ends,
            active_line: 0,
            input: Vec::new(),
            started_at: None,
            finalized_at: None,
            paused_at: None,
            paused_total: Duration::ZERO,
            limit,
            attempted_units: 0,
            correct_attempt_units: 0,
            correct_cells: 0,
            correct_units: 0,
            errors: 0,
            backspaces: 0,
            intended: BTreeMap::new(),
            rolling_samples: VecDeque::new(),
            best_rolling_kpm: 0.0,
            best_rolling_wpm: 0.0,
        })
    }

    pub fn input(&mut self, text: &str, now: Instant) -> InputOutcome {
        if self.paused_at.is_some() {
            return InputOutcome::IgnoredWhilePaused;
        }
        if self.is_finished(now) {
            return InputOutcome::Finished;
        }

        let attempted_before = self.attempted_units;
        let mut outcome = InputOutcome::Accepted;
        let input = normalize_nfc(text);
        for grapheme in UnicodeSegmentation::graphemes(input.as_str(), true) {
            let Some(target) = self.target_grapheme(self.input.len()) else {
                break;
            };
            let correct = grapheme == target;
            self.record_cell(grapheme, correct, now);
            self.advance_active_line();

            if self.target_complete() {
                outcome = InputOutcome::Finished;
                break;
            }
        }

        if self.kind == PracticeKind::Long && self.attempted_units != attempted_before {
            self.record_rolling_sample(now);
        }
        outcome
    }

    pub fn submit_line(&mut self, now: Instant) -> InputOutcome {
        if self.kind == PracticeKind::Key {
            return self.input("\n", now);
        }
        if self.paused_at.is_some() {
            return InputOutcome::IgnoredWhilePaused;
        }
        if self.is_finished(now) {
            return InputOutcome::Finished;
        }
        if self.target_grapheme(self.input.len()) == Some("\n") {
            return self.input("\n", now);
        }
        let Some(line_end) = self.current_line_range().map(|range| range.end) else {
            return InputOutcome::Finished;
        };
        let attempted_before = self.attempted_units;
        while self.input.len() < line_end {
            let submitted = if self.input.len() + 1 == line_end {
                self.target_grapheme(self.input.len())
                    .filter(|target| {
                        *target == "\n"
                            || (self.kind == PracticeKind::Words
                                && target.chars().all(char::is_whitespace))
                    })
                    .map(str::to_owned)
            } else {
                None
            };
            if let Some(submitted) = submitted {
                self.record_cell(&submitted, true, now);
            } else {
                self.record_cell("", false, now);
            }
        }
        self.advance_active_line();
        if self.kind == PracticeKind::Long && self.attempted_units != attempted_before {
            self.record_rolling_sample(now);
        }
        if self.target_complete() {
            InputOutcome::Finished
        } else {
            InputOutcome::Accepted
        }
    }

    pub fn backspace(&mut self) -> bool {
        if self.finalized_at.is_some() || self.paused_at.is_some() || self.target_complete() {
            return false;
        }
        if self.kind != PracticeKind::Key
            && self.active_line > 0
            && self
                .current_line_range()
                .is_some_and(|range| self.input.len() == range.start)
        {
            self.active_line -= 1;
            self.backspaces += 1;
            return true;
        }
        let index = self.input.len().saturating_sub(1);
        let units = self
            .target_grapheme(index)
            .map(|target| unit_count(self.language, target))
            .unwrap_or(0);
        if let Some(cell) = self.input.pop() {
            if cell.correct {
                self.correct_cells = self.correct_cells.saturating_sub(1);
                self.correct_units = self.correct_units.saturating_sub(units);
            }
            self.backspaces += 1;
            true
        } else {
            false
        }
    }

    pub fn toggle_pause(&mut self, now: Instant) -> bool {
        if self.finalized_at.is_some() || self.kind == PracticeKind::Test {
            return false;
        }

        if let Some(paused_at) = self.paused_at.take() {
            if self.started_at.is_some() {
                self.paused_total += now.saturating_duration_since(paused_at);
            }
        } else {
            self.paused_at = Some(now);
        }
        true
    }

    pub fn metrics(&self, now: Instant) -> Metrics {
        let active = self.active(now);
        let minutes = active.as_secs_f64() / 60.0;
        let cpm = if minutes > 0.0 {
            self.correct_cells as f64 / minutes
        } else {
            0.0
        };
        let kpm = if minutes > 0.0 {
            self.correct_units as f64 / minutes
        } else {
            0.0
        };

        Metrics {
            active,
            correct_cells: self.correct_cells,
            correct_units: self.correct_units,
            attempted_units: self.attempted_units,
            errors: self.errors,
            backspaces: self.backspaces,
            cpm,
            kpm,
            wpm: cpm / 5.0,
            accuracy: if self.attempted_units == 0 {
                100.0
            } else {
                self.correct_attempt_units as f64 / self.attempted_units as f64 * 100.0
            },
        }
    }

    pub fn intended_keys(&self) -> &BTreeMap<char, [u64; 2]> {
        &self.intended
    }

    pub const fn language(&self) -> Language {
        self.language
    }

    pub const fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    pub const fn attempted_units(&self) -> u64 {
        self.attempted_units
    }

    pub fn cursor(&self) -> usize {
        self.input.len()
    }

    pub fn target_len(&self) -> usize {
        self.target_ends.len()
    }

    pub fn target_cells(&self) -> impl Iterator<Item = (&str, Option<bool>)> {
        self.target_ends.iter().enumerate().map(|(index, &end)| {
            let start = index
                .checked_sub(1)
                .map_or(0, |previous| self.target_ends[previous] as usize);
            (
                &self.target[start..end as usize],
                self.input.get(index).map(|cell| cell.correct),
            )
        })
    }

    pub fn input_cells(&self) -> impl Iterator<Item = (&str, Option<&str>, Option<bool>)> {
        self.target_ends.iter().enumerate().map(|(index, &end)| {
            let start = index
                .checked_sub(1)
                .map_or(0, |previous| self.target_ends[previous] as usize);
            let cell = self.input.get(index);
            (
                &self.target[start..end as usize],
                cell.and_then(|cell| (!cell.entered.is_empty()).then_some(cell.entered.as_str())),
                cell.map(|cell| cell.correct),
            )
        })
    }

    pub fn line_ranges(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.line_ends.iter().scan(0, |start, &end| {
            let range = *start..end as usize;
            *start = end as usize;
            Some(range)
        })
    }

    pub const fn current_line_index(&self) -> usize {
        self.active_line
    }

    pub fn current_line_range(&self) -> Option<Range<usize>> {
        self.line_ranges().nth(self.active_line)
    }

    pub const fn best_rolling_speeds(&self) -> (f64, f64) {
        (self.best_rolling_kpm, self.best_rolling_wpm)
    }

    pub fn extend_target(
        &mut self,
        separator: &str,
        target: &str,
        item_ends: &[usize],
    ) -> Result<()> {
        if self.finalized_at.is_some() {
            bail!("cannot extend finalized practice");
        }
        let target = normalize_nfc(target);
        if target.is_empty() {
            bail!("practice target extension cannot be empty");
        }
        let extension = format!("{}{target}", normalize_nfc(separator));
        let ends = grapheme_ends(&extension, self.target.len())?;
        let extension_len = ends.len();
        let target_len = UnicodeSegmentation::graphemes(target.as_str(), true).count();
        let separator_len = extension_len.saturating_sub(target_len);
        validate_item_ends(item_ends, target_len)?;
        let extension_item_ends = item_ends
            .iter()
            .map(|end| separator_len.saturating_add(*end))
            .collect::<Vec<_>>();
        let extension_byte_ends = grapheme_ends(&extension, 0)?;
        let extension_lines = logical_line_ends(
            self.kind,
            &extension,
            &extension_byte_ends,
            &extension_item_ends,
        )?;
        let offset = self.target_ends.len();
        self.target.push_str(&extension);
        self.target_ends.extend(ends);
        if self.kind == PracticeKind::Key {
            self.line_ends = vec![u32::try_from(self.target_ends.len())?];
        } else {
            self.line_ends.extend(
                extension_lines
                    .into_iter()
                    .map(|end| u32::try_from(offset.saturating_add(end as usize)))
                    .collect::<std::result::Result<Vec<_>, _>>()?,
            );
        }
        Ok(())
    }

    pub fn finalize(&mut self, now: Instant) -> Metrics {
        self.finalized_at.get_or_insert(now);
        self.metrics(now)
    }

    pub fn time_limit_reached(&self, now: Instant) -> bool {
        self.limit.is_some_and(|limit| self.active(now) >= limit)
    }

    pub fn is_finished(&self, now: Instant) -> bool {
        self.finalized_at.is_some() || self.target_complete() || self.time_limit_reached(now)
    }

    pub fn target_complete(&self) -> bool {
        self.input.len() == self.target_ends.len()
            && (self.kind != PracticeKind::Key || self.input.iter().all(|cell| cell.correct))
    }

    fn active(&self, now: Instant) -> Duration {
        let Some(started_at) = self.started_at else {
            return Duration::ZERO;
        };
        let now = self.finalized_at.unwrap_or(now);
        let current_pause = self.paused_at.map_or(Duration::ZERO, |paused_at| {
            now.saturating_duration_since(paused_at)
        });
        let active = now
            .saturating_duration_since(started_at)
            .saturating_sub(self.paused_total)
            .saturating_sub(current_pause);
        if self.finalized_at.is_some() {
            self.limit.map_or(active, |limit| active.min(limit))
        } else {
            active
        }
    }

    fn target_grapheme(&self, index: usize) -> Option<&str> {
        indexed_grapheme(&self.target, &self.target_ends, index)
    }

    fn record_cell(&mut self, entered: &str, correct: bool, now: Instant) {
        let intended = self
            .target_grapheme(self.input.len())
            .map(|target| key_units(self.language, target))
            .unwrap_or_default();
        let units = intended.len() as u64;
        self.started_at.get_or_insert(now);
        self.attempted_units += units;
        if correct {
            self.correct_attempt_units += units;
            self.correct_cells += 1;
            self.correct_units += units;
        } else {
            self.errors += 1;
        }
        for unit in intended {
            self.intended.entry(unit).or_default()[usize::from(!correct)] += 1;
        }
        self.input.push(Cell {
            entered: entered.into(),
            correct,
        });
    }

    fn advance_active_line(&mut self) {
        while self
            .line_ends
            .get(self.active_line)
            .is_some_and(|&end| self.input.len() >= end as usize)
        {
            self.active_line += 1;
        }
    }

    fn record_rolling_sample(&mut self, now: Instant) {
        const WINDOW: Duration = Duration::from_secs(30);
        let active = self.active(now);
        let units = self.correct_units;
        let cells = self.correct_cells;
        if let Some((last, _, _)) = self.rolling_samples.back()
            && active < *last
        {
            let last = *last;
            self.rolling_samples.clear();
            self.rolling_samples.push_back((last, units, cells));
            return;
        }
        self.rolling_samples.push_back((active, units, cells));
        let cutoff = active.saturating_sub(WINDOW);
        while self.rolling_samples.len() > 1 && self.rolling_samples[1].0 <= cutoff {
            self.rolling_samples.pop_front();
        }
        if let Some((time, _, _)) = self.rolling_samples.front_mut() {
            *time = (*time).max(cutoff);
        }
        if active < WINDOW {
            return;
        }
        let Some((_, baseline_units, baseline_cells)) = self.rolling_samples.front() else {
            return;
        };
        let kpm = units.saturating_sub(*baseline_units) as f64 * 2.0;
        let wpm = cells.saturating_sub(*baseline_cells) as f64 * 2.0 / 5.0;
        self.best_rolling_kpm = self.best_rolling_kpm.max(kpm);
        self.best_rolling_wpm = self.best_rolling_wpm.max(wpm);
    }
}

fn logical_line_ends(
    kind: PracticeKind,
    target: &str,
    target_ends: &[u32],
    item_ends: &[usize],
) -> Result<Vec<u32>> {
    if kind == PracticeKind::Key {
        return Ok(vec![u32::try_from(target_ends.len())?]);
    }

    let mut lines = Vec::new();
    let mut item_start = 0;
    for &item_end in item_ends {
        let mut line_start = item_start;
        let mut line_width = 0;
        let mut segment_start = item_start;
        for index in item_start..item_end {
            let grapheme = indexed_grapheme(target, target_ends, index)
                .ok_or_else(|| anyhow::anyhow!("practice item boundary exceeds target"))?;
            if grapheme == "\n" {
                append_line_segment(
                    target,
                    target_ends,
                    segment_start..index,
                    &mut line_start,
                    &mut line_width,
                    &mut lines,
                )?;
                push_line_end(&mut lines, index + 1)?;
                line_start = index + 1;
                line_width = 0;
                segment_start = index + 1;
            } else if grapheme.chars().all(char::is_whitespace) {
                append_line_segment(
                    target,
                    target_ends,
                    segment_start..index + 1,
                    &mut line_start,
                    &mut line_width,
                    &mut lines,
                )?;
                segment_start = index + 1;
            }
        }
        append_line_segment(
            target,
            target_ends,
            segment_start..item_end,
            &mut line_start,
            &mut line_width,
            &mut lines,
        )?;
        if line_start < item_end {
            push_line_end(&mut lines, item_end)?;
        }
        item_start = item_end;
    }
    Ok(lines)
}

fn validate_item_ends(item_ends: &[usize], target_len: usize) -> Result<()> {
    if item_ends.last().copied() != Some(target_len)
        || item_ends.first().copied() == Some(0)
        || item_ends.windows(2).any(|ends| ends[0] >= ends[1])
    {
        bail!("practice item boundaries must be ordered and cover the target");
    }
    Ok(())
}

fn append_line_segment(
    target: &str,
    target_ends: &[u32],
    segment: Range<usize>,
    line_start: &mut usize,
    line_width: &mut usize,
    lines: &mut Vec<u32>,
) -> Result<()> {
    if segment.is_empty() {
        return Ok(());
    }
    let segment_width = segment.clone().try_fold(0_usize, |width, index| {
        indexed_grapheme(target, target_ends, index)
            .map(|grapheme| width.saturating_add(UnicodeWidthStr::width(grapheme)))
            .ok_or_else(|| anyhow::anyhow!("practice line segment exceeds target"))
    })?;
    if *line_start < segment.start && line_width.saturating_add(segment_width) > LOGICAL_LINE_WIDTH
    {
        push_line_end(lines, segment.start)?;
        *line_start = segment.start;
        *line_width = 0;
    }
    if segment_width <= LOGICAL_LINE_WIDTH {
        *line_width = line_width.saturating_add(segment_width);
        return Ok(());
    }

    for index in segment {
        let width = UnicodeWidthStr::width(
            indexed_grapheme(target, target_ends, index)
                .ok_or_else(|| anyhow::anyhow!("practice line segment exceeds target"))?,
        );
        if *line_start < index && line_width.saturating_add(width) > LOGICAL_LINE_WIDTH {
            push_line_end(lines, index)?;
            *line_start = index;
            *line_width = 0;
        }
        *line_width = line_width.saturating_add(width);
    }
    Ok(())
}

fn push_line_end(lines: &mut Vec<u32>, end: usize) -> Result<()> {
    let end = u32::try_from(end)?;
    if lines.last().copied() != Some(end) {
        lines.push(end);
    }
    Ok(())
}

fn indexed_grapheme<'a>(target: &'a str, target_ends: &[u32], index: usize) -> Option<&'a str> {
    let end = *target_ends.get(index)? as usize;
    let start = index
        .checked_sub(1)
        .map_or(0, |previous| target_ends[previous] as usize);
    target.get(start..end)
}

fn grapheme_ends(text: &str, offset: usize) -> Result<Vec<u32>> {
    let mut ends = Vec::new();
    for (start, grapheme) in UnicodeSegmentation::grapheme_indices(text, true) {
        let Some(end) = offset.checked_add(start.saturating_add(grapheme.len())) else {
            bail!("practice target is too large");
        };
        let Ok(end) = u32::try_from(end) else {
            bail!("practice target exceeds 4 GiB");
        };
        ends.push(end);
    }
    Ok(ends)
}

#[cfg(test)]
mod tests;

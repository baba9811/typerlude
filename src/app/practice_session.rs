use super::{
    ActivePractice, App, ItemDelta, Key, KeyInput, KeyKind, KeyModifiers, PracticeMode, Screen,
    StopRule,
    practice_flow::{STREAM_BATCH_ITEMS, TEXT_KINDS, catalog_target, select_catalog_items},
    result::item_delta,
};
use crate::{model::PracticeKind, typing::input_language};
use anyhow::Result;
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use unicode_segmentation::UnicodeSegmentation;

impl App {
    pub(super) fn handle_practice_key(&mut self, key: KeyInput, now: Instant) -> Result<()> {
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
                if key.kind == KeyKind::Press && key.is_plain_q_command() {
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
        let practice_kind = active.kind();
        if active.engine.is_paused() {
            if key.kind == KeyKind::Press && key.is_plain_q_command() {
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
            Key::Char(' ')
                if practice_kind == PracticeKind::Words
                    && matches!(key.kind, KeyKind::Press | KeyKind::Repeat)
                    && key.modifiers == KeyModifiers::NONE =>
            {
                self.submit_practice_line(now)?;
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
        self.tick_game(now);
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
}

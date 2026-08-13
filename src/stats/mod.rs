use crate::{
    content::{ContentCatalog, ContentKind, ResolvedItem},
    model::{Language, PracticeKind},
    storage::SessionRecord,
    typing::key_units,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use time::Date;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Range {
    Days7,
    Days30,
    Days90,
    All,
}

impl Range {
    pub fn includes(self, date: Date, today: Date) -> bool {
        let days = match self {
            Self::Days7 => 6,
            Self::Days30 => 29,
            Self::Days90 => 89,
            Self::All => return true,
        };
        date <= today && date >= today.saturating_sub(time::Duration::days(days))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpeedSummary {
    pub average: f64,
    pub best: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overview {
    pub total: Duration,
    pub sessions: usize,
    pub accuracy: f64,
    pub kpm: SpeedSummary,
    pub wpm: SpeedSummary,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyAccuracy {
    pub key: char,
    pub correct: u64,
    pub errors: u64,
    pub accuracy: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressPoint {
    pub date: Date,
    pub kpm: f64,
    pub wpm: f64,
    pub accuracy: f64,
    pub minutes: f64,
}

pub fn summarize<'a>(sessions: impl IntoIterator<Item = &'a SessionRecord>) -> Overview {
    let mut overview = Overview::default();
    let mut duration_ms = 0_u64;
    let mut attempted = 0_u128;
    let mut weighted_accuracy = 0.0;

    for session in sessions {
        overview.sessions += 1;
        let count = overview.sessions as f64;
        duration_ms = duration_ms.saturating_add(session.duration_ms);
        attempted += u128::from(session.attempted_units);
        weighted_accuracy += session.accuracy * session.attempted_units as f64;
        overview.kpm.average += (session.kpm - overview.kpm.average) / count;
        overview.wpm.average += (session.wpm - overview.wpm.average) / count;
        overview.kpm.best = overview.kpm.best.max(session.kpm);
        overview.wpm.best = overview.wpm.best.max(session.wpm);
    }

    overview.total = Duration::from_millis(duration_ms);
    overview.accuracy = if attempted == 0 {
        0.0
    } else {
        weighted_accuracy / attempted as f64
    };
    overview
}

pub fn history(
    sessions: &[SessionRecord],
    range: Range,
    today: Date,
    language: Option<Language>,
    mode: Option<PracticeKind>,
) -> Vec<&SessionRecord> {
    let mut selected = sessions
        .iter()
        .filter(|session| {
            range.includes(session.local_date, today)
                && language.is_none_or(|language| session.language == language)
                && mode.is_none_or(|mode| session.mode == mode)
        })
        .collect::<Vec<_>>();
    selected.sort_unstable_by(|left, right| {
        right
            .started_at_unix_ms
            .cmp(&left.started_at_unix_ms)
            .then_with(|| right.id.cmp(&left.id))
    });
    selected
}

#[derive(Default)]
struct ProgressTotals {
    duration_ms: u64,
    sessions: usize,
    kpm: f64,
    wpm: f64,
    attempted: u128,
    weighted_accuracy: f64,
}

pub fn progress(
    sessions: &[SessionRecord],
    range: Range,
    today: Date,
    language: Language,
    mode: Option<PracticeKind>,
) -> Vec<ProgressPoint> {
    let mut totals = BTreeMap::<Date, ProgressTotals>::new();
    for session in sessions.iter().filter(|session| {
        session.language == language
            && range.includes(session.local_date, today)
            && mode.is_none_or(|mode| session.mode == mode)
    }) {
        let point = totals.entry(session.local_date).or_default();
        point.duration_ms = point.duration_ms.saturating_add(session.duration_ms);
        point.sessions += 1;
        let count = point.sessions as f64;
        point.kpm += (session.kpm - point.kpm) / count;
        point.wpm += (session.wpm - point.wpm) / count;
        point.attempted += u128::from(session.attempted_units);
        point.weighted_accuracy += session.accuracy * session.attempted_units as f64;
    }

    totals
        .into_iter()
        .map(|(date, total)| ProgressPoint {
            date,
            kpm: total.kpm,
            wpm: total.wpm,
            accuracy: if total.attempted == 0 {
                0.0
            } else {
                total.weighted_accuracy / total.attempted as f64
            },
            minutes: total.duration_ms as f64 / 60_000.0,
        })
        .collect()
}

pub fn streak(dates: impl IntoIterator<Item = Date>, today: Date) -> usize {
    let dates = dates
        .into_iter()
        .filter(|date| *date <= today)
        .collect::<BTreeSet<_>>();
    let Some(mut date) = dates.contains(&today).then_some(today).or_else(|| {
        today
            .previous_day()
            .filter(|yesterday| dates.contains(yesterday))
    }) else {
        return 0;
    };

    let mut count = 0;
    loop {
        if !dates.contains(&date) {
            break;
        }
        count += 1;
        let Some(previous) = date.previous_day() else {
            break;
        };
        date = previous;
    }
    count
}

pub(crate) fn intended_key_counts(
    sessions: &[SessionRecord],
    language: Language,
) -> BTreeMap<char, [u64; 2]> {
    let mut counts = BTreeMap::<char, [u64; 2]>::new();
    for session in sessions
        .iter()
        .filter(|session| session.language == language)
    {
        for (&key, &[correct, errors]) in &session.intended_keys {
            let total = counts.entry(key).or_default();
            total[0] = total[0].saturating_add(correct);
            total[1] = total[1].saturating_add(errors);
        }
    }
    counts
}

pub(crate) fn has_key_attempts(counts: &BTreeMap<char, [u64; 2]>, min_attempts: u64) -> bool {
    counts.values().any(|[correct, errors]| {
        let attempts = u128::from(*correct) + u128::from(*errors);
        attempts > 0 && attempts >= u128::from(min_attempts)
    })
}

pub fn weak_keys(counts: &BTreeMap<char, [u64; 2]>, min_attempts: u64) -> Vec<KeyAccuracy> {
    let mut keys = counts
        .iter()
        .filter_map(|(&key, &[correct, errors])| {
            let attempts = u128::from(correct) + u128::from(errors);
            (errors != 0 && attempts >= u128::from(min_attempts)).then_some(KeyAccuracy {
                key,
                correct,
                errors,
                accuracy: if attempts == 0 {
                    0.0
                } else {
                    correct as f64 / attempts as f64 * 100.0
                },
            })
        })
        .collect::<Vec<_>>();
    keys.sort_unstable_by(|left, right| {
        left.accuracy
            .total_cmp(&right.accuracy)
            .then_with(|| left.key.cmp(&right.key))
    });
    keys
}

pub fn adaptive_candidates<'a>(
    catalog: &'a ContentCatalog,
    sessions: &[SessionRecord],
    language: Language,
    seed: u64,
) -> Vec<&'a ResolvedItem> {
    let counts = intended_key_counts(sessions, language);
    let weak = weak_keys(&counts, 10)
        .into_iter()
        .take(3)
        .map(|value| value.key)
        .collect::<BTreeSet<_>>();
    let ordinary = || {
        catalog
            .items()
            .filter(|item| {
                item.language == language
                    && matches!(item.kind, ContentKind::Word | ContentKind::Sentence)
            })
            .collect::<Vec<_>>()
    };
    let mut rng = fastrand::Rng::with_seed(seed);
    if weak.is_empty() {
        let mut candidates = ordinary();
        candidates.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        rng.shuffle(&mut candidates);
        return candidates;
    }

    let mut scored = ordinary()
        .into_iter()
        .filter_map(|item| {
            let score = key_units(language, &item.text)
                .into_iter()
                .filter(|key| weak.contains(key))
                .count();
            (score != 0).then_some((score, item))
        })
        .collect::<Vec<_>>();
    scored.sort_unstable_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut start = 0;
    while start < scored.len() {
        let mut end = start + 1;
        while end < scored.len() && scored[end].0 == scored[start].0 {
            end += 1;
        }
        if end - start > 1 {
            rng.shuffle(&mut scored[start..end]);
        }
        start = end;
    }
    scored.into_iter().map(|(_, item)| item).collect()
}

#[cfg(test)]
mod tests;

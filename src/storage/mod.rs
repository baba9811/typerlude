use crate::{
    model::{Language, PracticeKind},
    practice::Metrics,
};
use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    io::ErrorKind,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use time::{Date, OffsetDateTime, UtcOffset};

mod atomic;

pub(crate) use atomic::{atomic_write, atomic_write_new, rename_no_replace};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    pub config: PathBuf,
    pub sessions: PathBuf,
    pub content: PathBuf,
    pub themes: PathBuf,
    pub update_cache: PathBuf,
}

impl AppPaths {
    pub fn from_override(root: PathBuf) -> Self {
        Self {
            config: root.join("config.toml"),
            sessions: root.join("sessions"),
            content: root.join("content"),
            themes: root.join("themes"),
            update_cache: root.join("cache/update.json"),
        }
    }

    pub fn discover() -> Result<Self> {
        if let Some(root) = std::env::var_os("TYPERLUDE_HOME").filter(|value| !value.is_empty()) {
            return Ok(Self::from_override(root.into()));
        }
        let dirs = ProjectDirs::from("", "", "typerlude")
            .context("unable to resolve the user data directory")?;
        Ok(Self {
            config: dirs.config_dir().join("config.toml"),
            sessions: dirs
                .state_dir()
                .unwrap_or_else(|| dirs.data_local_dir())
                .join("sessions"),
            content: dirs.data_dir().join("content"),
            themes: dirs.data_dir().join("themes"),
            update_cache: dirs.cache_dir().join("update.json"),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadWarning {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadResult<T> {
    pub values: Vec<T>,
    pub warnings: Vec<LoadWarning>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SessionRecord {
    pub schema_version: u16,
    pub id: String,
    pub started_at_unix_ms: i128,
    pub local_date: Date,
    pub language: Language,
    pub mode: PracticeKind,
    pub content_id: String,
    pub difficulty: Option<u8>,
    pub duration_ms: u64,
    pub correct_units: u64,
    pub attempted_units: u64,
    pub errors: u64,
    pub backspaces: u64,
    pub cpm: f64,
    pub kpm: f64,
    pub wpm: f64,
    pub accuracy: f64,
    pub intended_keys: BTreeMap<char, [u64; 2]>,
}

impl SessionRecord {
    pub fn from_result(
        started_at: OffsetDateTime,
        language: Language,
        mode: PracticeKind,
        content_id: impl Into<String>,
        difficulty: Option<u8>,
        metrics: &Metrics,
        intended_keys: &BTreeMap<char, [u64; 2]>,
    ) -> Self {
        let now = OffsetDateTime::now_utc();
        let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            schema_version: 1,
            id: format!(
                "{}-{}-{counter}",
                now.unix_timestamp_nanos(),
                std::process::id()
            ),
            started_at_unix_ms: started_at.unix_timestamp_nanos() / 1_000_000,
            local_date: started_at
                .to_offset(UtcOffset::local_offset_at(started_at).unwrap_or(UtcOffset::UTC))
                .date(),
            language,
            mode,
            content_id: content_id.into(),
            difficulty,
            duration_ms: metrics.active.as_millis().min(u64::MAX.into()) as u64,
            correct_units: metrics.correct_units,
            attempted_units: metrics.attempted_units,
            errors: metrics.errors,
            backspaces: metrics.backspaces,
            cpm: metrics.cpm,
            kpm: metrics.kpm,
            wpm: metrics.wpm,
            accuracy: metrics.accuracy,
            intended_keys: intended_keys.clone(),
        }
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            bail!("unsupported session schema version {}", self.schema_version);
        }
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            bail!("session ID is not filesystem-safe");
        }
        if self.content_id.trim().is_empty() {
            bail!("content ID must not be empty");
        }
        if self
            .difficulty
            .is_some_and(|value| !(1..=3).contains(&value))
        {
            bail!("difficulty must be between 1 and 3");
        }
        if self.attempted_units == 0 {
            bail!("cannot save an empty session");
        }
        if self.correct_units > self.attempted_units {
            bail!("correct units cannot exceed attempted units");
        }
        for (name, value) in [("cpm", self.cpm), ("kpm", self.kpm), ("wpm", self.wpm)] {
            if !value.is_finite() || value < 0.0 {
                bail!("{name} must be finite and nonnegative");
            }
        }
        if !self.accuracy.is_finite() || !(0.0..=100.0).contains(&self.accuracy) {
            bail!("accuracy must be between 0 and 100");
        }
        Ok(())
    }
}

pub fn save_session(paths: &AppPaths, session: &SessionRecord) -> Result<PathBuf> {
    session.validate()?;
    let bytes = serde_json::to_vec_pretty(session).context("failed to serialize session")?;
    fs::create_dir_all(&paths.sessions)
        .with_context(|| format!("failed to create {}", paths.sessions.display()))?;
    let path = paths.sessions.join(format!("{}.json", session.id));
    match atomic_write_new(&path, &bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            bail!("session {} already exists", session.id);
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to save session {}", session.id));
        }
    }
    Ok(path)
}

pub fn load_sessions(paths: &AppPaths) -> Result<LoadResult<SessionRecord>> {
    let entries = match fs::read_dir(&paths.sessions) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(LoadResult {
                values: Vec::new(),
                warnings: Vec::new(),
            });
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read {}", paths.sessions.display()));
        }
    };
    let mut paths = entries
        .filter_map(|entry| match entry {
            Ok(entry) if entry.path().extension() == Some(OsStr::new("json")) => {
                Some(Ok(entry.path()))
            }
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();

    let mut loaded = LoadResult {
        values: Vec::new(),
        warnings: Vec::new(),
    };
    for path in paths {
        let record = fs::read(&path)
            .with_context(|| "unable to read session JSON")
            .and_then(|bytes| {
                serde_json::from_slice::<SessionRecord>(&bytes).context("invalid session JSON")
            })
            .and_then(|record| {
                record.validate()?;
                if path.file_stem() != Some(OsStr::new(&record.id)) {
                    bail!("session ID does not match its filename");
                }
                Ok(record)
            });
        match record {
            Ok(record) => loaded.values.push(record),
            Err(error) => loaded.warnings.push(LoadWarning {
                path,
                message: error.to_string(),
            }),
        }
    }
    Ok(loaded)
}

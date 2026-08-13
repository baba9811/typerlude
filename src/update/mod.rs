use crate::{VERSION, config::Settings, storage::AppPaths};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::OsStr,
    fmt,
    io::IsTerminal,
    str::FromStr,
    sync::mpsc::{Receiver, sync_channel},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

mod cache;
mod registry;

use cache::{load_cache, write_cache};
pub use registry::detect_install_method;
use registry::{COMMAND_TIMEOUT, registry_version};

const CACHE_INTERVAL_SECONDS: i64 = 24 * 60 * 60;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StableVersion(pub u64, pub u64, pub u64);

impl FromStr for StableVersion {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        let mut parts = value.split('.');
        let mut next = || -> Result<u64> {
            let part = parts.next().context("version must have three components")?;
            if part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
            {
                bail!("invalid stable version component");
            }
            part.parse().context("version component is too large")
        };
        let version = Self(next()?, next()?, next()?);
        if parts.next().is_some() {
            bail!("version must have exactly three components");
        }
        Ok(version)
    }
}

impl fmt::Display for StableVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.0, self.1, self.2)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallMethod {
    Npm,
    Cargo,
    Standalone,
}

impl InstallMethod {
    pub const fn instructions(self) -> &'static str {
        match self {
            Self::Npm => "npm install -g typerlude · npx typerlude",
            Self::Cargo => "cargo install --force typerlude",
            Self::Standalone => "https://github.com/baba9811/typerlude/releases",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateNotice {
    pub current: StableVersion,
    pub latest: StableVersion,
    pub method: InstallMethod,
}

impl UpdateNotice {
    pub const fn instructions(&self) -> &'static str {
        self.method.instructions()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UpdateCache {
    pub schema_version: u16,
    pub checked_at_unix: i64,
    pub latest: String,
}

pub fn should_check(cache: Option<&UpdateCache>, now: i64) -> bool {
    let Some(cache) = cache
        .filter(|cache| cache.schema_version == 1 && cache.latest.parse::<StableVersion>().is_ok())
    else {
        return true;
    };
    !now.checked_sub(cache.checked_at_unix)
        .is_some_and(|age| (0..CACHE_INTERVAL_SECONDS).contains(&age))
}

pub fn notice(
    current: &str,
    latest: &str,
    skipped: &str,
    method: InstallMethod,
) -> Option<UpdateNotice> {
    let current = current.parse::<StableVersion>().ok()?;
    let latest = latest.parse::<StableVersion>().ok()?;
    (latest > current && skipped != latest.to_string()).then_some(UpdateNotice {
        current,
        latest,
        method,
    })
}

fn unix_now() -> Result<i64> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs();
    i64::try_from(seconds).context("system clock is out of range")
}

pub fn foreground_check(
    paths: &AppPaths,
) -> Result<(InstallMethod, StableVersion, Option<StableVersion>)> {
    let method = detect_install_method();
    let current = VERSION.parse::<StableVersion>()?;
    let latest = registry_version(method, COMMAND_TIMEOUT)?;
    if let Some(latest) = latest {
        write_cache(
            &paths.update_cache,
            &UpdateCache {
                schema_version: 1,
                checked_at_unix: unix_now()?,
                latest: latest.to_string(),
            },
        )?;
    }
    Ok((method, current, latest))
}

fn automatic_allowed(
    method: InstallMethod,
    stdin_terminal: bool,
    stdout_terminal: bool,
    suppressed: bool,
) -> bool {
    method == InstallMethod::Npm && stdin_terminal && stdout_terminal && !suppressed
}

pub fn start_background_check(
    settings: &Settings,
    paths: &AppPaths,
) -> Option<Receiver<Option<UpdateNotice>>> {
    let method = detect_install_method();
    let suppressed = cfg!(test)
        || !settings.check_updates
        || env::var_os("CI").is_some()
        || env::var_os("TYPERLUDE_NO_UPDATE_CHECK").as_deref() == Some(OsStr::new("1"));
    if !automatic_allowed(
        method,
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal(),
        suppressed,
    ) {
        return None;
    }
    let now = unix_now().ok()?;
    if !should_check(load_cache(&paths.update_cache).as_ref(), now) {
        return None;
    }
    let path = paths.update_cache.clone();
    let skipped = settings.skipped_update_version.clone();
    let (sender, receiver) = sync_channel(1);
    thread::spawn(move || {
        let update = registry_version(method, COMMAND_TIMEOUT)
            .and_then(|latest| latest.context("npm returned no version"))
            .and_then(|latest| {
                write_cache(
                    &path,
                    &UpdateCache {
                        schema_version: 1,
                        checked_at_unix: now,
                        latest: latest.to_string(),
                    },
                )?;
                Ok(latest)
            })
            .ok()
            .and_then(|latest| notice(VERSION, &latest.to_string(), &skipped, method));
        let _ = sender.send(update);
    });
    Some(receiver)
}

#[cfg(test)]
mod tests;

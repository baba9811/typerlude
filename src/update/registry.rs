use super::{InstallMethod, StableVersion};
use anyhow::{Context, Result, bail};
use command_group::CommandGroup;
use directories::BaseDirs;
use std::{
    env,
    ffi::OsStr,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(super) const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);
const MAX_COMMAND_OUTPUT_BYTES: usize = 4 * 1024;
const NPM_REGISTRY: &str = "https://registry.npmjs.org/";
const CRATES_IO_INDEX: &str = "sparse+https://index.crates.io/";

pub fn detect_install_method() -> InstallMethod {
    let marker = env::var_os("TYPERLUDE_INSTALL_METHOD");
    if marker.as_deref() == Some(OsStr::new("npm")) {
        return InstallMethod::Npm;
    }
    let Some(executable) = env::current_exe().ok().map(resolve_path) else {
        return InstallMethod::Standalone;
    };
    let Some(cargo_home) = cargo_home().map(resolve_path) else {
        return InstallMethod::Standalone;
    };
    install_method_from(None, &executable, &cargo_home)
}

pub(super) fn install_method_from(
    marker: Option<&OsStr>,
    executable: &Path,
    cargo_home: &Path,
) -> InstallMethod {
    if marker == Some(OsStr::new("npm")) {
        return InstallMethod::Npm;
    }
    if executable.parent() == Some(cargo_home.join("bin").as_path()) {
        InstallMethod::Cargo
    } else {
        InstallMethod::Standalone
    }
}

fn cargo_home() -> Option<PathBuf> {
    env::var_os("CARGO_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| BaseDirs::new().map(|dirs| dirs.home_dir().join(".cargo")))
}

fn resolve_path(path: PathBuf) -> PathBuf {
    let path = if path.is_absolute() {
        path
    } else {
        env::current_dir().map_or(path.clone(), |current| current.join(path))
    };
    fs::canonicalize(&path).unwrap_or(path)
}

pub(super) fn parse_npm_version(output: &str) -> Result<StableVersion> {
    if output.trim() != output || output.contains(['\r', '\n']) {
        bail!("npm returned an invalid version line");
    }
    output
        .parse()
        .context("npm returned an invalid stable version")
}

pub(super) fn parse_cargo_version(output: &str) -> Result<StableVersion> {
    let value = output
        .lines()
        .next()
        .context("cargo search returned no results")?
        .strip_prefix("typerlude = \"")
        .context("cargo search did not return the typerlude crate")?;
    let (version, suffix) = value
        .split_once('"')
        .context("cargo search returned an invalid version field")?;
    if !(suffix.is_empty()
        || suffix.starts_with(' ') && suffix.trim_start_matches(' ').starts_with('#'))
    {
        bail!("cargo search returned an invalid first field");
    }
    version
        .parse()
        .context("cargo search returned an invalid stable version")
}

#[cfg(test)]
pub(super) fn run_registry_command(
    executable: &Path,
    arguments: &[&str],
    timeout: Duration,
) -> Result<String> {
    let current = env::current_dir().context("failed to resolve registry command directory")?;
    run_registry_command_in(executable, arguments, &current, &[], timeout)
}

fn run_registry_command_in(
    executable: &Path,
    arguments: &[&str],
    current_dir: &Path,
    environment: &[(&str, &OsStr)],
    timeout: Duration,
) -> Result<String> {
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .current_dir(current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for &(key, value) in environment {
        command.env(key, value);
    }
    let mut child = command
        .group_spawn()
        .with_context(|| format!("failed to start {}", executable.display()))?;
    let deadline = Instant::now()
        .checked_add(timeout)
        .unwrap_or_else(Instant::now);
    let mut timed_out = false;
    let finish = loop {
        match child.inner().try_wait() {
            Ok(Some(_)) => break Ok(()),
            Ok(None) => {}
            Err(error) => break Err(error),
        }
        if Instant::now() >= deadline {
            timed_out = true;
            break Ok::<_, std::io::Error>(());
        }
        thread::sleep(
            COMMAND_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
        );
    };
    let _ = child.kill();
    let output = child
        .wait_with_output()
        .context("failed to collect registry command output")?;
    finish.context("failed to poll registry command")?;
    if timed_out {
        bail!("registry command timed out");
    }
    if output.stdout.len() > MAX_COMMAND_OUTPUT_BYTES
        || output.stderr.len() > MAX_COMMAND_OUTPUT_BYTES
    {
        bail!("registry command output exceeded 4 KiB");
    }
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        bail!("registry command failed: {}", error.escape_debug());
    }
    let output =
        String::from_utf8(output.stdout).context("registry command output is not valid UTF-8")?;
    let output = output
        .strip_suffix("\r\n")
        .or_else(|| output.strip_suffix('\n'))
        .unwrap_or(&output);
    if output.is_empty() {
        bail!("registry command returned no output");
    }
    Ok(output.to_owned())
}

pub(super) fn npm_executable() -> &'static Path {
    if cfg!(windows) {
        Path::new("npm.cmd")
    } else {
        Path::new("npm")
    }
}

struct RegistryWorkspace(PathBuf);

impl RegistryWorkspace {
    fn new() -> Result<Self> {
        for _ in 0..10 {
            let path = env::temp_dir().join(format!(
                "typerlude-registry-{}-{}",
                std::process::id(),
                fastrand::u64(..)
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return fs::canonicalize(path)
                        .map(Self)
                        .context("failed to resolve registry workspace");
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error).context("failed to create registry workspace"),
            }
        }
        bail!("failed to create a unique registry workspace")
    }
}

impl Drop for RegistryWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn registry_version(
    method: InstallMethod,
    timeout: Duration,
) -> Result<Option<StableVersion>> {
    if method == InstallMethod::Standalone {
        return Ok(None);
    }
    let workspace = RegistryWorkspace::new()?;
    match method {
        InstallMethod::Npm => {
            let user_config = workspace.0.join("user.npmrc");
            let global_config = workspace.0.join("global.npmrc");
            fs::write(&user_config, []).context("failed to isolate npm user configuration")?;
            fs::write(&global_config, []).context("failed to isolate npm global configuration")?;
            run_registry_command_in(
                npm_executable(),
                &[
                    "view",
                    "typerlude",
                    "version",
                    "--silent",
                    "--registry=https://registry.npmjs.org/",
                ],
                &workspace.0,
                &[
                    ("NPM_CONFIG_REGISTRY", OsStr::new(NPM_REGISTRY)),
                    ("NPM_CONFIG_USERCONFIG", user_config.as_os_str()),
                    ("NPM_CONFIG_GLOBALCONFIG", global_config.as_os_str()),
                ],
                timeout,
            )
            .and_then(|output| parse_npm_version(&output))
            .map(Some)
        }
        InstallMethod::Cargo => {
            let cargo_home = workspace.0.join("cargo-home");
            fs::create_dir(&cargo_home).context("failed to isolate Cargo configuration")?;
            run_registry_command_in(
                Path::new("cargo"),
                &[
                    "search",
                    "typerlude",
                    "--limit",
                    "1",
                    "--registry",
                    "crates-io",
                ],
                &workspace.0,
                &[
                    ("CARGO_HOME", cargo_home.as_os_str()),
                    (
                        "CARGO_REGISTRIES_CRATES_IO_INDEX",
                        OsStr::new(CRATES_IO_INDEX),
                    ),
                ],
                timeout,
            )
            .and_then(|output| parse_cargo_version(&output))
            .map(Some)
        }
        InstallMethod::Standalone => unreachable!(),
    }
}

use super::{Command, ContentCommand, PracticeArgs};
use crate::model::{Language, PracticeKind};
use anyhow::{Context, Result, anyhow, bail};
use std::{ffi::OsString, path::PathBuf};

pub fn parse_args(args: Vec<OsString>) -> Result<Command> {
    if args.is_empty() {
        return Ok(Command::Home);
    }
    let first = args[0].to_str();
    match first {
        Some("quick") => parse_practice(&args, PracticeKind::Quick),
        Some("key" | "keys") => parse_practice(&args, PracticeKind::Key),
        Some("word" | "words") => parse_practice(&args, PracticeKind::Words),
        Some("sentence" | "sentences") => parse_practice(&args, PracticeKind::Sentence),
        Some("long") => parse_practice(&args, PracticeKind::Long),
        Some("test") => parse_practice(&args, PracticeKind::Test),
        Some("practice") => {
            if args.len() != 2 {
                bail!("practice requires exactly one file path");
            }
            let mut parsed = PracticeArgs::new(PracticeKind::Long);
            parsed.file = Some(PathBuf::from(&args[1]));
            Ok(Command::Practice(parsed))
        }
        Some("stats") => exact(&args, Command::Stats),
        Some("history") => exact(&args, Command::History),
        Some("themes") => exact(&args, Command::Themes),
        Some("paths") => exact(&args, Command::Paths),
        Some("licenses") => exact(&args, Command::Licenses),
        Some("update") => exact(&args, Command::Update),
        Some("content") => parse_content(&args),
        Some("--version" | "-V" | "version") => exact(&args, Command::Version),
        Some("--help" | "-h" | "help") => exact(&args, Command::Help),
        Some("--smoke") => exact(&args, Command::Smoke),
        Some(value) if value.starts_with('-') => bail!("unknown option {value:?}"),
        _ => {
            if args.len() != 1 {
                bail!("a direct file path cannot have trailing operands");
            }
            Ok(Command::File(PathBuf::from(&args[0])))
        }
    }
}

fn exact(args: &[OsString], command: Command) -> Result<Command> {
    if args.len() != 1 {
        bail!("unexpected trailing operand");
    }
    Ok(command)
}

fn parse_practice(args: &[OsString], kind: PracticeKind) -> Result<Command> {
    let mut parsed = PracticeArgs::new(kind);
    let mut index = 1;
    while index < args.len() {
        let option = unicode(&args[index], "option")?;
        match option {
            "--lang" => {
                if parsed.language.is_some() {
                    bail!("--lang may be supplied only once");
                }
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("--lang requires ko or en"))?;
                parsed.language = Some(match unicode(value, "--lang value")? {
                    "ko" => Language::Ko,
                    "en" => Language::En,
                    value => bail!("invalid language {value:?}; expected ko or en"),
                });
            }
            "--time" => {
                if !matches!(kind, PracticeKind::Quick | PracticeKind::Test) {
                    bail!("--time is not valid for {kind:?}");
                }
                if parsed.seconds.is_some() {
                    bail!("--time may be supplied only once");
                }
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("--time requires a duration"))?;
                let value = unicode(value, "--time value")?;
                let seconds = value
                    .parse::<u64>()
                    .with_context(|| format!("invalid duration {value:?}"))?;
                let allowed = match kind {
                    PracticeKind::Quick => [15, 30, 60, 120].contains(&seconds),
                    PracticeKind::Test => [60, 180, 300, 600].contains(&seconds),
                    _ => false,
                };
                if !allowed {
                    bail!("invalid duration {seconds} for {kind:?}");
                }
                parsed.seconds = Some(seconds);
            }
            value => bail!("unknown or trailing option {value:?}"),
        }
        index += 1;
    }
    Ok(Command::Practice(parsed))
}

fn parse_content(args: &[OsString]) -> Result<Command> {
    let action = args
        .get(1)
        .ok_or_else(|| anyhow!("content requires list, add, validate, or disable"))?;
    match unicode(action, "content action")? {
        "list" if args.len() == 2 => Ok(Command::Content(ContentCommand::List)),
        "add" if args.len() == 3 => Ok(Command::Content(ContentCommand::Add(PathBuf::from(
            &args[2],
        )))),
        "validate" if args.len() == 2 => Ok(Command::Content(ContentCommand::Validate(None))),
        "validate" if args.len() == 3 => Ok(Command::Content(ContentCommand::Validate(Some(
            PathBuf::from(&args[2]),
        )))),
        "disable" if args.len() == 3 => Ok(Command::Content(ContentCommand::Disable(
            unicode(&args[2], "pack ID")?.to_owned(),
        ))),
        action => bail!("invalid content command or operands: {action}"),
    }
}

fn unicode<'a>(value: &'a OsString, name: &str) -> Result<&'a str> {
    value
        .to_str()
        .ok_or_else(|| anyhow!("{name} must be valid Unicode"))
}

use std::io::IsTerminal;
use typeul::cli::{Exit, is_input_error, parse_args, run, stdin_command, terminal_safe};

fn main() {
    if let Err((code, error)) = execute() {
        eprintln!("typeul: {}", terminal_safe(&format!("{error:#}")));
        std::process::exit(code);
    }
}

fn execute() -> Result<(), (i32, anyhow::Error)> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let no_args = args.is_empty();
    let mut command = parse_args(args).map_err(|error| (2, error))?;
    if no_args && !std::io::stdin().is_terminal() {
        command = stdin_command().map_err(|error| (2, error))?;
    }
    let exit = run(command).map_err(|error| {
        let code = if is_input_error(&error) { 2 } else { 1 };
        (code, error)
    })?;
    if let Exit::Launch(_) = exit {
        // Plan 2 consumes the fully validated startup without re-parsing input.
    }
    Ok(())
}

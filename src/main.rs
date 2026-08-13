use std::io::IsTerminal;
use typerlude::{
    cli::{Exit, is_input_error, parse_args, prepare_app, run, stdin_command},
    storage::AppPaths,
    terminal, terminal_safe,
};

fn main() {
    if let Err((code, error)) = execute() {
        eprintln!("typerlude: {}", terminal_safe(&format!("{error:#}")));
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
    if let Exit::Launch(startup) = exit {
        if !terminal::is_interactive_terminal() {
            return Err((2, anyhow::anyhow!("interactive terminal required")));
        }
        let paths = AppPaths::discover().map_err(|error| (1, error))?;
        let app = prepare_app(startup, paths).map_err(|error| {
            let code = if is_input_error(&error) { 2 } else { 1 };
            (code, error)
        })?;
        terminal::run(app).map_err(|error| (1, error))?;
    }
    Ok(())
}

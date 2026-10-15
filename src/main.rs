mod cli;

use std::process::ExitCode;

use clap::Parser;
use cli::{Cli, Command};
use winnower::commands;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Calls the library function for the given subcommand.
fn run(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Clean(args) => commands::clean::run(&args.path, args.older_than),
        Command::Init(args) => commands::init::run(&args.dir, &args.templates),
        Command::Templates => commands::templates::run(),
    }
}

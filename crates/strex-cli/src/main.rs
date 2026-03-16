#![deny(clippy::all)]

mod cli;
mod commands;
mod output;

use clap::Parser;
use cli::{Cli, Command};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let exit_code = match run(cli).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            2
        }
    };
    std::process::exit(exit_code);
}

async fn run(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        Command::Run(args) => commands::run::execute(args).await,
        Command::Validate(args) => commands::validate::execute(args).await,
        Command::Ui(args) => commands::ui::execute(args).await,
    }
}

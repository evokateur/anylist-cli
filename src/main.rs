mod adapter;
mod candidates;
mod login;
mod session;
mod storage;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use std::process::ExitCode;
use storage::{MissingState, Storage};

#[derive(Parser)]
#[command(about = "Work with your AnyList shopping lists")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sign in and cache your shopping lists.
    Login,
    /// Fetch a fresh copy of your shopping lists.
    Sync,
    /// Remove saved credentials and cached lists.
    Logout,
    /// Untick an item on the default list, by alias or item ID.
    Untick { item: String },
    #[command(hide = true)]
    Candidates { query: String },
    /// Print shell integration for zsh.
    Init { shell: Shell },
}

#[derive(Clone, clap::ValueEnum)]
enum Shell {
    Zsh,
}

#[tokio::main]
async fn run(command: Command) -> Result<()> {
    let storage = Storage::discover()?;
    let client = match command {
        Command::Logout => return storage.logout(),
        Command::Login => {
            let client = login::prompt().await?;
            storage.begin_session(&client.export_tokens()?)?;
            client
        }
        Command::Sync => login::restore(storage.tokens()?)?,
        Command::Untick { item } => {
            let client = login::restore(storage.tokens()?)?;
            println!("{}", session::untick(&client, &storage, &item).await?);
            return Ok(());
        }
        Command::Candidates { .. } | Command::Init { .. } => unreachable!(),
    };
    let current = session::sync(&client, &storage).await?;
    println!("Default list: {current}");
    Ok(())
}

fn main() -> ExitCode {
    let command = Cli::parse().command;
    let result = match command {
        Command::Candidates { query } => {
            let result = candidates::write(&query, std::io::stdout().lock());
            if result
                .as_ref()
                .is_err_and(|error| error.is::<MissingState>())
            {
                return ExitCode::FAILURE;
            }
            result
        }
        Command::Init { shell: Shell::Zsh } => {
            let cli = Cli::command();
            let public_commands = cli
                .get_subcommands()
                .filter(|command| !command.is_hide_set() && command.get_name() != "init")
                .map(clap::Command::get_name)
                .collect::<Vec<_>>()
                .join(" ");
            println!("typeset -ga _anylist_commands=({public_commands})");
            print!("{}", include_str!("anylist.zsh"));
            return ExitCode::SUCCESS;
        }
        command => run(command),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

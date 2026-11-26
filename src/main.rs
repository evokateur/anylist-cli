mod adapter;
mod login;
mod session;
mod storage;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::process::ExitCode;
use storage::Storage;

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
}

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
    };
    let current = session::sync(&client, &storage).await?;
    println!("Default list: {current}");
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse().command).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

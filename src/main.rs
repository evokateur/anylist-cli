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
        Command::Sync => anylist_rs::AnyListClient::from_tokens(storage.tokens()?)?,
    };
    let current = session::sync(&client, &storage).await?;
    println!("Current list: {current}");
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

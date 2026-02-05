mod config;

use crate::config::Config;
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mad")]
#[command(about = "Massive AD-tack: LDAP stress-testing and provisioning tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show current configuration
    Config,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Config => {
            let cfg = Config::load()?;
            println!("Configuration found:");
            println!("{:#?}", cfg);
        }
    }

    Ok(())
}

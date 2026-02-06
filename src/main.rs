mod commands;
mod config;
mod naming;
mod oids;

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
    /// Check connectivity and server information
    Check {
        /// Output results in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Manage users
    Users {
        #[command(subcommand)]
        command: UserCommands,
    },
}

#[derive(Subcommand)]
pub enum UserCommands {
    /// Add random users
    Add {
        /// Number of users to create
        #[arg(short, long, default_value_t = 1)]
        count: u32,

        /// Format for the username (overrides config)
        #[arg(short, long)]
        format: Option<String>,

        /// Container DN for new users (e.g., "ou=users,dc=example,dc=com")
        #[arg(short = 'C', long)]
        container: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Config => {
            commands::config::execute()?;
        }
        Commands::Check { json } => {
            commands::check::execute(json).await?;
        }
        Commands::Users { command } => {
            commands::users::execute(command).await?;
        }
    }

    Ok(())
}

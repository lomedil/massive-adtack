mod commands;
mod config;
mod dn;
mod naming;
mod oids;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::dn::DistinguishedName;

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
    /// Add multiple users to the directory
    Add {
        /// Number of users to create
        #[arg(short = 'n', long, default_value_t = 10)]
        count: u32,

        /// Username format template (e.g. "{first}.{last}{n}")
        #[arg(short, long)]
        format: Option<String>,

        /// Optional container DN where users will be created (relative to base DN)
        #[arg(short = 'C', long)]
        container: Option<DistinguishedName>,
    },

    /// List users from the directory
    List {
        /// Simple search filter (searches in multiple fields)
        #[arg(short, long)]
        filter: Option<String>,

        /// Optional container DN to scope the search (relative to base DN)
        #[arg(short = 'C', long)]
        container: Option<DistinguishedName>,

        /// Raw LDAP filter (e.g. "(objectCategory=person)")
        #[arg(short, long)]
        ldap_filter: Option<String>,
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

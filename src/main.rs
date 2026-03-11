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
    /// Manage groups
    Groups {
        #[command(subcommand)]
        command: GroupCommands,
    },
}

#[derive(Subcommand)]
pub enum GroupCommands {
    /// Add a new group to the directory
    Add {
        /// Name of the group to create
        #[arg()]
        groupname: String,

        /// Optional container DN where the group will be created (relative to base DN)
        #[arg(short = 'C', long)]
        container: Option<DistinguishedName>,
    },

    /// List groups from the directory
    List {
        /// Simple search filter (searches in cn and sAMAccountName)
        #[arg(short, long)]
        filter: Option<String>,

        /// Optional container DN to scope the search (relative to base DN)
        #[arg(short = 'C', long)]
        container: Option<DistinguishedName>,

        /// Raw LDAP filter (e.g. "(objectClass=group)")
        #[arg(short, long)]
        ldap_filter: Option<String>,
    },

    /// Remove a single group from the directory
    Rm {
        /// Group identifier: full DN, RDN literal like CN=My Group, or sAMAccountName
        #[arg()]
        name: String,

        /// Optional container DN to scope the search (relative to base DN)
        #[arg(short = 'C', long)]
        container: Option<DistinguishedName>,

        /// Dry run mode (do not delete the group, just show the match)
        #[arg(long)]
        dry_run: bool,

        /// Skip confirmation prompt
        #[arg(long)]
        no_confirm: bool,
    },

    /// Add users to a group in bulk
    Join {
        /// Group identifier: full DN, RDN literal like CN=My Group, or sAMAccountName
        #[arg()]
        name: String,

        /// Simple user search filter (searches in cn, sAMAccountName and mail)
        #[arg(short, long)]
        filter: Option<String>,

        /// Raw LDAP filter to select users (e.g. "(&(objectClass=user)(sAMAccountName=test_*))")
        #[arg(short, long)]
        ldap_filter: Option<String>,

        /// Optional container DN to scope both group resolution and user search (relative to base DN)
        #[arg(short = 'C', long)]
        container: Option<DistinguishedName>,

        /// Dry run mode (do not modify the group, just show what would change)
        #[arg(long)]
        dry_run: bool,
    },

    /// Remove users from a group in bulk
    Leave {
        /// Group identifier: full DN, RDN literal like CN=My Group, or sAMAccountName
        #[arg()]
        name: String,

        /// Simple user search filter (searches in cn, sAMAccountName and mail)
        #[arg(short, long)]
        filter: Option<String>,

        /// Raw LDAP filter to select users (e.g. "(&(objectClass=user)(sAMAccountName=test_*))")
        #[arg(short, long)]
        ldap_filter: Option<String>,

        /// Optional container DN to scope both group resolution and user search (relative to base DN)
        #[arg(short = 'C', long)]
        container: Option<DistinguishedName>,

        /// Dry run mode (do not modify the group, just show what would change)
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub enum UserCommands {
    /// Add multiple users to the directory
    Add {
        /// Number of users to create
        #[arg(short = 'n', long, default_value_t = 10)]
        count: u32,

        /// Username format template (e.g. "{first_name}.{last_name}{counter}")
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

    /// Remove users from the directory
    Rm {
        /// Filter to select users to remove (searches only in username field). Supports wildcards.
        #[arg()]
        filter: String,

        /// Optional container DN to scope the search (relative to base DN)
        #[arg(short = 'C', long)]
        container: Option<DistinguishedName>,

        /// Dry run mode (do not delete users, just list them)
        #[arg(long)]
        dry_run: bool,

        /// Skip confirmation prompt for multiple deletions
        #[arg(long)]
        no_confirm: bool,
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
        Commands::Groups { command } => {
            commands::groups::execute(command).await?;
        }
    }

    Ok(())
}

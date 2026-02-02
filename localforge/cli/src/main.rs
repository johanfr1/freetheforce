//! LocalForge CLI
//!
//! Command-line interface for interacting with the forge daemon.

use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod client;
mod commands;

use commands::{can, config, identity, init, logs, status};

#[derive(Parser)]
#[command(name = "forge")]
#[command(author, version, about = "LocalForge CLI - local-first developer infrastructure")]
#[command(propagate_version = true)]
struct Cli {
    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize identity and data directory
    Init,

    /// Manage identity
    #[command(subcommand)]
    Identity(IdentityCommands),

    /// Check if feature is allowed
    Can {
        /// Feature name to check
        feature: String,
    },

    /// Manage configuration
    #[command(subcommand)]
    Config(ConfigCommands),

    /// Show daemon status
    Status,

    /// View logs
    Logs {
        /// Number of lines to show
        #[arg(short = 'n', default_value = "20")]
        lines: usize,

        /// Follow log output
        #[arg(short = 'f', long)]
        follow: bool,

        /// Filter by log level
        #[arg(long)]
        level: Option<String>,
    },
}

#[derive(Subcommand)]
enum IdentityCommands {
    /// Show identity info
    Show,

    /// Set identity alias
    Alias {
        /// New alias name
        name: String,
    },

    /// Export identity for backup
    Export,

    /// Import identity from backup
    Import,
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Get a config value
    Get {
        /// Project namespace
        namespace: String,
        /// Config key
        key: String,
    },

    /// Set a config value
    Set {
        /// Project namespace
        namespace: String,
        /// Config key
        key: String,
        /// Config value
        value: String,
    },

    /// List all config values for a namespace
    List {
        /// Project namespace
        namespace: String,
    },

    /// Reset config to defaults
    Reset {
        /// Project namespace
        namespace: String,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => init::run(cli.json).await,
        Commands::Identity(cmd) => match cmd {
            IdentityCommands::Show => identity::show(cli.json).await,
            IdentityCommands::Alias { name } => identity::alias(&name, cli.json).await,
            IdentityCommands::Export => identity::export().await,
            IdentityCommands::Import => identity::import().await,
        },
        Commands::Can { feature } => can::run(&feature, cli.json).await,
        Commands::Config(cmd) => match cmd {
            ConfigCommands::Get { namespace, key } => {
                config::get(&namespace, &key, cli.json).await
            }
            ConfigCommands::Set { namespace, key, value } => {
                config::set(&namespace, &key, &value, cli.json).await
            }
            ConfigCommands::List { namespace } => config::list(&namespace, cli.json).await,
            ConfigCommands::Reset { namespace } => config::reset(&namespace, cli.json).await,
        },
        Commands::Status => status::run(cli.json).await,
        Commands::Logs { lines, follow, level } => {
            logs::run(lines, follow, level.as_deref()).await
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            e.exit_code()
        }
    }
}

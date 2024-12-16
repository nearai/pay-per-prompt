use clap::Parser;
use commands::{config_command, open_payment_channel_command};
use config::{data_storage, Config, ConfigUpdate};
use near_sdk::NearToken;
use std::path::PathBuf;

mod client;
mod commands;
mod config;
mod contract;
mod provider;
mod utils;

#[derive(Parser, Clone)]
enum Commands {
    /// Open new payment channel.
    Open {
        /// Amount to deposit in the payment channel.
        amount: NearToken,
    },
    /// List all payment channels opened on this device.
    List,
    /// Add extra balance to the payment channel.
    Topup,
    /// Close payment channel.
    Close,
    /// Show available information about user and payment channels.
    Info,
    /// Show and update configuration.
    #[command(subcommand)]
    Config(ConfigUpdate),
    /// Advanced commands.
    #[command(subcommand)]
    Advanced(AdvancedCommands),
}

#[derive(Parser, Clone)]
enum AdvancedCommands {
    Withdraw,
    ForceClose,
}

#[derive(Parser)]
struct CLI {
    /// Verbose mode.
    #[arg(short, long)]
    verbose: bool,
    /// Path to the config file. Default is <CONFIG_DIR>/.near_payment_channel/config.json
    #[arg(short, long)]
    config_file: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

impl CLI {
    fn config_file(&self) -> PathBuf {
        self.config_file
            .clone()
            .unwrap_or_else(|| data_storage().join("config.json"))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = CLI::parse();
    let config = Config::load(cli.config_file(), cli.verbose);

    match cli.command {
        Commands::Open { amount } => {
            open_payment_channel_command(&config, amount).await?;
        }
        Commands::List => {
            println!("List")
        }
        Commands::Topup => {
            println!("Topup")
        }
        Commands::Close => {
            println!("Close")
        }
        Commands::Info => {
            println!("Info")
        }
        Commands::Config(update) => {
            config_command(config, &update);
        }
        Commands::Advanced(advanced_commands) => println!("Advanced command"),
    }

    Ok(())
}

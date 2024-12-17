use clap::Parser;
use cli::commands::{
    close_command, close_payload_command, config_command, info_command,
    open_payment_channel_command, send_command, topup_command, withdraw_command,
};
use cli::config::{data_storage, Config, ConfigUpdate};
use near_sdk::NearToken;
use std::path::PathBuf;

#[derive(Parser, Clone)]
enum Commands {
    /// Open new payment channel.
    Open {
        /// Amount to deposit in the payment channel.
        amount: NearToken,
    },
    /// Add extra balance to the payment channel.
    Topup {
        channel_id: Option<String>,
        #[arg(short, long)]
        amount: NearToken,
    },
    /// Close payment channel.
    Close {
        channel_id: Option<String>,
        /// Manual payload to close the channel, if not specified we
        /// ask the provider to generate it.
        #[arg(short, long)]
        payload: Option<String>,
    },
    /// Show available information about user and payment channels.
    Info {
        channel_id: Option<String>,
        #[arg(short, long)]
        no_update: bool,
    },
    /// Show and update configuration.
    #[command(subcommand)]
    Config(ConfigUpdate),
    /// Advanced commands.
    #[command(subcommand)]
    Advanced(AdvancedCommands),
}

#[derive(Parser, Clone)]
enum AdvancedCommands {
    /// Withdraw balance, run this command from the point of view of the receiver.
    Withdraw {
        /// Signed state created by the sender encoded in base64
        payload: String,
    },
    /// Receiver generates the closing payload.
    ClosePayload { channel_id: Option<String> },
    /// Start a force close of a payment channel.
    StartForceClose,
    /// Finish a force close of a payment channel.
    FinishForceClose,
    /// Sign transaction to send money to the receiver. (Off-chain)
    Send {
        /// How much money to send.
        amount: NearToken,
        /// Id of the channel. If it is not specified we look if there is only one channel and use it.
        channel_id: Option<String>,
        /// If `update` is true, the local instance of the channel will be updated.
        #[arg(short, long)]
        no_update: bool,
    },
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
        Commands::Topup { channel_id, amount } => topup_command(&config, channel_id, amount).await,
        Commands::Close {
            channel_id,
            payload,
        } => close_command(&config, channel_id, payload).await,
        Commands::Info {
            channel_id,
            no_update,
        } => {
            info_command(&config, channel_id, !no_update).await;
        }
        Commands::Config(update) => {
            config_command(config, &update);
        }
        Commands::Advanced(advanced_commands) => match advanced_commands {
            AdvancedCommands::Withdraw { payload } => withdraw_command(&config, payload).await,
            AdvancedCommands::ClosePayload { channel_id } => {
                close_payload_command(&config, channel_id)
            }
            AdvancedCommands::StartForceClose => println!("StartForceClose"),
            AdvancedCommands::FinishForceClose => println!("FinishForceClose"),
            AdvancedCommands::Send {
                amount,
                channel_id,
                no_update,
            } => {
                send_command(&config, amount, channel_id, !no_update);
            }
        },
    }

    Ok(())
}

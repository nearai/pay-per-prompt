use base64::{prelude::BASE64_STANDARD, Engine};
use clap::Parser;
use near_sdk::{near, AccountId, NearToken};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{
    contract::{Contract, ContractChannel},
    provider::Details,
};

pub fn data_storage() -> PathBuf {
    dirs::config_dir().unwrap().join("near_payment_channel")
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    // Account id of the payment channel contract
    pub contract: AccountId,
    // Url to the provider RPC
    pub provider_url: String,
    // Url to NEAR RPC
    pub near_rpc_url: String,
    // Account id of the user
    pub account_id: Option<AccountId>,
    // Verbose mode
    #[serde(default, skip)]
    pub verbose: bool,
    // Path to the config file
    #[serde(skip)]
    pub config_file: PathBuf,
}

#[derive(Parser, Clone)]
pub enum ConfigUpdate {
    /// Update account id
    AccountId { account_id: String },
}

impl Default for Config {
    fn default() -> Self {
        Self {
            contract: "staging.paymentchannel.near".to_string().parse().unwrap(),
            provider_url: "https://payperprompt.near.ai".to_string(),
            near_rpc_url: "https://archival-rpc.mainnet.near.org/".to_string(),
            verbose: true,
            account_id: None,
            config_file: PathBuf::new(),
        }
    }
}

impl Config {
    pub fn load(config_file: PathBuf, verbose: bool) -> Self {
        if !config_file.exists() {
            if verbose {
                println!(
                    "Config file not found, creating a new one at {:?}\n",
                    config_file
                );
            }
            // Create folder if it doesn't exist
            let folder = config_file.parent().unwrap();
            if !folder.exists() {
                std::fs::create_dir_all(folder).unwrap();
            }

            // Write default config to file
            let mut config = Config::default();
            config.config_file = config_file.clone();
            config.save();
        }

        // Read config from file
        let config = std::fs::read_to_string(&config_file).unwrap();
        if verbose {
            println!("\nConfig file:\n{}\n", config);
        }

        let mut config: Config = serde_json::from_str(&config).unwrap();
        config.verbose = verbose;
        config.config_file = config_file;
        config
    }

    pub fn save(&self) {
        let config = serde_json::to_string_pretty(&self).unwrap();
        std::fs::write(&self.config_file, config).unwrap();
    }

    pub fn get_account_id(&self) -> AccountId {
        match &self.account_id {
            Some(account_id) => account_id.clone(),
            None => {
                eprintln!("User account id is required for this action. Set the account id using `payment-channel config account_id <account_id>`.");
                std::process::exit(1);
            }
        }
    }

    pub fn update_provider(&self, details: &Details) {
        let providers = data_storage().join("providers");
        if !providers.exists() {
            std::fs::create_dir_all(&providers).unwrap();
        }
        let provider_file = providers.join(format!("{}.json", &details.account_id));

        if provider_file.exists() {
            let prev_details = std::fs::read_to_string(&provider_file).unwrap();
            let prev_details = serde_json::from_str::<Details>(&prev_details).unwrap();
            if prev_details != *details {
                eprintln!(
                    "Provider details already exist and are different. {:?}.\nRemove the provider and make sure no active open channels exist with this provider.",
                    provider_file
                );
                std::process::exit(1);
            }
        } else {
            let details = serde_json::to_string_pretty(&details).unwrap();
            std::fs::write(&provider_file, details).unwrap();

            if self.verbose {
                println!("Provider information saved to {:?}", provider_file);
            }
        }
    }

    pub fn update_channel(&self, channel: &Channel) {
        channel.save(self.verbose);
    }

    pub fn near_contract(&self) -> Contract {
        Contract::new(self)
    }
}

#[near(serializers = [borsh, json])]
#[derive(Debug)]
pub struct State {
    channel_id: String,
    spent_balance: NearToken,
}

#[near(serializers = [borsh, json])]
#[derive(Debug)]
pub struct SignedState {
    state: State,
    signature: near_crypto::Signature,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Channel {
    pub channel_id: String,
    pub receiver: Details,
    pub sender: Details,
    pub sender_secret_key: near_crypto::SecretKey,
    pub spent_balance: NearToken,
    pub added_balance: NearToken,
    pub withdrawn_balance: NearToken,
    pub force_close_started: Option<near_sdk::Timestamp>,
}

impl Channel {
    pub fn load(channel_id: String, verbose: bool) -> Self {
        let channel_file = data_storage()
            .join("channels")
            .join(format!("{}.json", channel_id));
        let channel = std::fs::read_to_string(&channel_file).unwrap();

        let channel: Channel = serde_json::from_str(&channel).unwrap();
        if verbose {
            println!(
                "\nChannel details:\n{}\n",
                near_sdk::serde_json::to_string_pretty(&channel.redacted()).unwrap()
            );
        }
        channel
    }

    pub fn save(&self, verbose: bool) {
        let channels = data_storage().join("channels");
        if !channels.exists() {
            std::fs::create_dir_all(&channels).unwrap();
        }

        let channel_file = channels.join(format!("{}.json", &self.channel_id));

        let channel = serde_json::to_string_pretty(self).unwrap();
        std::fs::write(&channel_file, channel).unwrap();

        if verbose {
            println!("\nChannel information saved to:\n{:?}\n", channel_file);
        }
    }

    pub fn available_balance(&self) -> NearToken {
        self.added_balance.saturating_sub(self.spent_balance)
    }

    pub fn info(&self) -> State {
        State {
            channel_id: self.channel_id.clone(),
            spent_balance: self.spent_balance,
        }
    }

    pub fn payload(&self) -> SignedState {
        let state = self.info();
        let message = near_sdk::borsh::to_vec(&state).unwrap();
        let signature = self.sender_secret_key.sign(&message);
        SignedState { state, signature }
    }

    pub fn payload_b64(&self) -> String {
        let payload = self.payload();
        let payload_bytes = near_sdk::borsh::to_vec(&payload).unwrap();
        BASE64_STANDARD.encode(&payload_bytes)
    }

    pub fn redacted(&self) -> serde_json::Value {
        let mut value = near_sdk::serde_json::to_value(&self).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .entry("sender_secret_key")
            .and_modify(|e| {
                *e = serde_json::Value::String("-- REDACTED --".to_string());
            });
        value
    }

    fn newer(&self, contract_channel: &ContractChannel) -> bool {
        if contract_channel.added_balance > self.added_balance {
            return true;
        }

        if contract_channel.withdrawn_balance > self.withdrawn_balance {
            return true;
        }

        if contract_channel.force_close_started.is_some() && self.force_close_started.is_none() {
            return true;
        }

        if contract_channel.receiver.account_id != self.receiver.account_id
            || contract_channel.sender.account_id != self.sender.account_id
            || contract_channel.receiver.public_key != self.receiver.public_key
            || contract_channel.sender.public_key != self.sender.public_key
        {
            eprintln!("Channel details have changed in unexpected ways.");
            std::process::exit(1);
        }

        false
    }

    pub fn update_if_newer(&mut self, contract_channel: ContractChannel, verbose: bool) -> bool {
        if self.newer(&contract_channel) {
            self.added_balance = contract_channel.added_balance;
            self.withdrawn_balance = contract_channel.withdrawn_balance;
            self.force_close_started = contract_channel.force_close_started;
            self.save(verbose);

            true
        } else {
            false
        }
    }
}

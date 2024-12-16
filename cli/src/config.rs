use clap::Parser;
use near_sdk::AccountId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{contract::Contract, provider::Details};

pub fn data_storage() -> PathBuf {
    dirs::config_dir().unwrap().join("near_payment_channel")
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    // Account id of the payment channel contract
    pub contract: AccountId,
    // Url to the provider RPC
    pub provider_url: String,
    // Account id of the user
    pub account_id: Option<AccountId>,
    #[serde(default, skip)]
    pub verbose: bool,
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
        }
    }

    pub fn near_contract(&self) -> Contract {
        Contract::new(self)
    }
}

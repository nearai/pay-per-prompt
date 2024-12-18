use std::str::FromStr;
use std::sync::Arc;

use anyhow::Error;
use borsh::to_vec;
use borsh::BorshSerialize;
use cli::config::{
    Config as NearPaymentChannelContractClientConfig, SignedState as NearSignedState,
    State as NearState,
};
use cli::contract::Contract as NearPaymentChannelContractClient;
use near_cli_rs::common::KeyPairProperties;
use near_cli_rs::config::Config as NearConfig;
use near_cli_rs::config::NetworkConfig as NearNetworkConfig;
use near_crypto::InMemorySigner;
use near_crypto::Signature;
use near_crypto::Signer;
use near_crypto::{PublicKey as NearPublicKey, SecretKey as NearSecretKey};
use near_jsonrpc_client::JsonRpcClient;
use near_jsonrpc_primitives::types::query::QueryResponseKind;
use near_primitives::types::AccountId;
use near_primitives::types::BlockReference;
use near_sdk::json_types::U128;
use near_sdk::NearToken;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::RwLock;
use tracing::info;

use crate::{ProviderDb, MODEL_DELIMITER};

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderConfig {
    pub providers: Vec<Provider>,
    pub account_id: AccountId,
    pub network: String,
    pub db_url: String,
    pub cost_per_completion: U128,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Provider {
    pub canonical_name: String,
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    pub provider: String,
    pub model_name: String,
}

impl ModelInfo {
    pub fn new(provider: String, model_name: String) -> Self {
        Self {
            provider,
            model_name,
        }
    }

    pub fn from_str(input: &str) -> anyhow::Result<ModelInfo> {
        if let Some((provider, model_name)) = input.split_once(MODEL_DELIMITER) {
            let provider = provider.trim();
            let model_name = model_name.trim();
            if provider.is_empty() || model_name.is_empty() {
                return Err(Error::msg(
                    "Invalid input format. Provider or model name cannot be empty.",
                ));
            }
            Ok(ModelInfo {
                provider: provider.to_string(),
                model_name: model_name.to_string(),
            })
        } else {
            Err(Error::msg(
                "Invalid input format. Expected exactly one '::' delimiter.",
            ))
        }
    }
}

// Reminder: this is private information, do not expose or serialize this struct
#[derive(Deserialize)]
pub struct AccountInfoPrivate {
    pub account_id: AccountId,
    pub network_config: NearNetworkConfig,
    pub public_key: NearPublicKey,
    pub private_key: NearSecretKey,
}

#[derive(Clone, Serialize)]
pub struct AccountInfoPublic {
    pub account_id: AccountId,
    pub network: String,
    pub public_key: NearPublicKey,
}

impl AccountInfoPrivate {
    pub fn new(
        credentials_home_dir: &std::path::Path,
        account_id: AccountId,
        network_config: NearNetworkConfig,
    ) -> Self {
        // Get the key pair from the credentials home dir
        let file_name = format!("{}.json", account_id);
        let mut path = std::path::PathBuf::from(credentials_home_dir);
        path.push(network_config.network_name.clone());
        path.push(file_name);
        let data = std::fs::read_to_string(&path).expect("Access key file not found!");
        let key_pair: KeyPairProperties =
            serde_json::from_str(&data).expect("Error reading data from file");
        let private_key = NearSecretKey::from_str(&key_pair.secret_keypair_str)
            .expect("Error reading data from file");
        let public_key = NearPublicKey::from_str(&key_pair.public_key_str)
            .expect("Error reading data from file");

        Self {
            account_id,
            network_config,
            public_key,
            private_key,
        }
    }

    pub fn public_view(&self) -> AccountInfoPublic {
        AccountInfoPublic {
            account_id: self.account_id.clone(),
            network: self.network_config.network_name.clone(),
            public_key: self.public_key.clone(),
        }
    }

    pub fn as_signer(&self) -> Signer {
        Signer::InMemory(InMemorySigner::from_secret_key(
            self.account_id.clone(),
            self.private_key.clone(),
        ))
    }
}

#[derive(Clone, Serialize, BorshSerialize, Deserialize)]
pub struct State {
    pub channel_name: String,
    pub spent_balance: U128,
}

#[derive(Clone, Serialize)]
pub struct PaymentChannelState {
    pub channel_name: String,
    pub sender: String,
    pub receiver: String,
    pub spent_balance: U128,
    pub added_balance: U128,
    pub withdraw_balance: U128,
}

#[derive(Clone)]
pub struct ProviderCtx {
    pub config: ProviderConfig,
    pub db: ProviderDb,
    pub pc_client: NearPaymentChannelContractClient,
    _account_info: Arc<RwLock<AccountInfoPrivate>>,
}

impl ProviderCtx {
    pub fn new(config: ProviderConfig) -> Self {
        info!("Loading near config with network: {}", config.network);
        let near_config = NearConfig::default();
        let near_network_config = near_config
            .network_connection
            .get(&config.network.clone())
            .expect(&format!("Network not found: {}", config.network))
            .clone();

        info!("Loading account info: {}", config.account_id);
        let account_info = AccountInfoPrivate::new(
            &near_config.credentials_home_dir,
            config.account_id.clone(),
            near_network_config.clone(),
        );

        info!("Validating account info");
        let also_account_id = config.account_id.clone();
        let also_near_network_config = near_network_config.clone();
        let result = std::thread::spawn(move || {
            let rpc = JsonRpcClient::connect(also_near_network_config.rpc_url.as_ref());
            let query_view_method_request = near_jsonrpc_client::methods::query::RpcQueryRequest {
                block_reference: BlockReference::latest(),
                request: near_primitives::views::QueryRequest::ViewAccount {
                    account_id: also_account_id.clone(),
                },
            };
            let result = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(rpc.call(query_view_method_request))
                .unwrap();
            match result.kind {
                QueryResponseKind::ViewAccount(account_view) => account_view,
                _ => unreachable!(),
            }
        });
        result.join().expect("Thread panicked");

        info!("Creating payment channel client");
        let mut pc_client_config = NearPaymentChannelContractClientConfig::default();
        pc_client_config.account_id = Some(config.account_id.clone());
        let pc_client = NearPaymentChannelContractClient::new_with_signer(
            &pc_client_config,
            InMemorySigner::from_secret_key(
                account_info.account_id.clone(),
                account_info.private_key.clone(),
            ),
        );

        info!("Creating database");
        let db = ProviderDb::new(&config.db_url, pc_client.clone());

        Self {
            config,
            db,
            pc_client,
            _account_info: Arc::new(RwLock::new(account_info)),
        }
    }

    pub async fn public_account_info(&self) -> AccountInfoPublic {
        self._account_info.read().await.public_view()
    }

    pub async fn get_pc_state(
        &self,
        channel_name: &str,
    ) -> Result<Option<PaymentChannelState>, sqlx::Error> {
        let channel_row =
            if let Some(row) = self.db.get_channel_row_or_refresh(channel_name).await? {
                row
            } else {
                return Ok(None);
            };

        // Get the spent balance from the latest signed state
        // If no signed state is found, the spent balance is 0
        let spent_balance = match self.db.latest_signed_state(channel_name).await? {
            Some(signed_state) => U128::from(signed_state.spent_balance().as_yoctonear()),
            None => U128::from(0),
        };

        let added_balance = channel_row.added_balance();
        let withdraw_balance = channel_row.withdraw_balance();
        Ok(Some(PaymentChannelState {
            channel_name: channel_row.name,
            sender: channel_row.sender,
            receiver: channel_row.receiver,
            spent_balance,
            added_balance: U128::from(added_balance.as_yoctonear()),
            withdraw_balance: U128::from(withdraw_balance.as_yoctonear()),
        }))
    }

    pub async fn validate_insert_signed_state(
        &self,
        min_cost: u128,
        signed_state: &NearSignedState,
    ) -> Result<(), anyhow::Error> {
        match self
            .db
            .get_channel_row_or_refresh(&signed_state.state.channel_id)
            .await?
        {
            None => Err(anyhow::anyhow!("Channel not found")),
            Some(channel_row) => {
                // Get the sender public key registered in the channel
                let sender_public_key = NearPublicKey::from_str(&channel_row.sender_pk)
                    .map_err(|e| anyhow::anyhow!("Invalid public key format: {}", e))?;

                // validate signature with the senders public key
                // this assumes that the sender is the only one who can sign the state
                let data = to_vec(&signed_state.state)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize state: {}", e))?;
                let signature_valid = signed_state.signature.verify(&data, &sender_public_key);
                if !signature_valid {
                    return Err(anyhow::anyhow!("Invalid signature"));
                }

                // Check that the sender is monotonically increasing their spent balance
                let most_recent_spent_balance = match self
                    .db
                    .latest_signed_state(&signed_state.state.channel_id)
                    .await?
                {
                    Some(signed_state) => signed_state.spent_balance().as_yoctonear(),
                    None => 0_u128,
                };
                let new_spent_balance = signed_state.state.spent_balance.as_yoctonear();
                if new_spent_balance <= most_recent_spent_balance {
                    return Err(anyhow::anyhow!(
                        "New spent balance is less than or equal to the most recent spent balance"
                    ));
                }

                // Check that the sender has authorized an amount above the minimum cost
                let new_spent_balance = signed_state.state.spent_balance.as_yoctonear();
                let prev_spend_balance = most_recent_spent_balance;
                if new_spent_balance < prev_spend_balance + min_cost {
                    return Err(anyhow::anyhow!(
                        "New spent balance is less than the minimum cost of {} yoctoNEAR",
                        min_cost
                    ));
                }

                // Check that the users new spend balance is not greater than the added balance.
                // If it is, resync the channel and check again (unhappy path)
                // If it still is, tell the user they need to top up the channel
                let added_balance = channel_row.added_balance().as_yoctonear();
                if new_spent_balance > added_balance {
                    // in case the channel is out of sync with the blockchain, resync and check again
                    match self
                        .db
                        .refresh_channel_row(&signed_state.state.channel_id)
                        .await?
                    {
                        Some(channel_row) => {
                            let resynced_spent_balance = channel_row.added_balance().as_yoctonear();
                            if new_spent_balance > resynced_spent_balance {
                                return Err(anyhow::anyhow!(
                                    "New spent balance is greater than the added balance by {} units. Please top up the channel.",
                                    new_spent_balance - resynced_spent_balance
                                ));
                            }
                        }
                        None => {
                            return Err(anyhow::anyhow!("Channel not found"));
                        }
                    }
                }

                self.db.insert_signed_state(signed_state).await?;

                Ok(())
            }
        }
    }

    pub async fn close_pc(&self, channel_name: &str) -> Result<NearSignedState, anyhow::Error> {
        // TODO: Require signature from the sender to close the channel, so no other user can send the close request

        match self.db.get_channel_row_or_refresh(channel_name).await? {
            None => Err(anyhow::anyhow!("Channel not found")),
            Some(channel_row) => {
                info!("Closing channel: {}", channel_name);

                // Check if there is the sender has spent money that we haven't withdrawn yet
                if let Some(signed_state) = self.db.latest_signed_state(channel_name).await? {
                    info!(
                        "There is a signed state: {:?}",
                        signed_state.spent_balance()
                    );
                    // TODO: Check the amount of money available is large enough so that it makes sense to withdraw it
                    let state = NearSignedState {
                        state: NearState {
                            channel_id: channel_row.name.clone(),
                            spent_balance: signed_state.spent_balance(),
                        },
                        signature: Signature::from_str(&signed_state.signature).unwrap(),
                    };

                    info!("Withdrawing: {:?}", state);

                    self.pc_client.withdraw(state).await;
                }

                let state = NearState {
                    channel_id: channel_name.to_string(),
                    spent_balance: NearToken::from_yoctonear(0),
                };

                let message = borsh::to_vec(&state).unwrap();
                let signer = self._account_info.read().await.as_signer();
                let signature = signer.sign(&message);

                // TODO: Update db reflecting that the channel is now closed

                Ok(NearSignedState { state, signature })
            }
        }
    }
}

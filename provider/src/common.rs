use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

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
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::ChannelError;
use crate::ChannelRow;
use crate::ProviderError;
use crate::ProviderResult;
use crate::SignedStateError;
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
    pub cancel_token: CancellationToken,
    pc_client: NearPaymentChannelContractClient,
    db: ProviderDb,
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
            cancel_token: CancellationToken::new(),
            _account_info: Arc::new(RwLock::new(account_info)),
        }
    }

    pub async fn public_account_info(&self) -> AccountInfoPublic {
        self._account_info.read().await.public_view()
    }

    pub async fn get_pc_state(&self, channel_name: &str) -> ProviderResult<PaymentChannelState> {
        let channel_row = self.db.get_channel_row(channel_name).await?;

        // Get the spent balance from the latest signed state
        // If no signed state is found, the spent balance is 0
        let spent_balance = match self.db.latest_signed_state(channel_name).await? {
            Some(signed_state) => U128::from(signed_state.spent_balance().as_yoctonear()),
            None => U128::from(0),
        };

        let added_balance = channel_row.added_balance();
        let withdraw_balance = channel_row.withdraw_balance();
        Ok(PaymentChannelState {
            channel_name: channel_row.name,
            sender: channel_row.sender,
            receiver: channel_row.receiver,
            spent_balance,
            added_balance: U128::from(added_balance.as_yoctonear()),
            withdraw_balance: U128::from(withdraw_balance.as_yoctonear()),
        })
    }

    pub async fn validate_insert_signed_state(
        &self,
        min_cost: u128,
        signed_state: &NearSignedState,
    ) -> ProviderResult<()> {
        let channel_row = self
            .db
            .get_channel_row(&signed_state.state.channel_id)
            .await?;

        // Get the receiver public key registered in the channel,
        // Check that 'we' are the receiver, otherwise return an error
        let receiver_public_key =
            NearPublicKey::from_str(&channel_row.receiver_pk).map_err(|e| {
                ProviderError::Channel(ChannelError::InvalidPublicKey(format!(
                    "Error deserializing receiver public key in contract: {}",
                    e
                )))
            })?;
        if receiver_public_key != self._account_info.read().await.public_key {
            return Err(ProviderError::Channel(ChannelError::InvalidOwner(format!(
                "Receiver public key {} of channel {} does not match public key {}",
                channel_row.name,
                receiver_public_key,
                self._account_info.read().await.public_key
            ))));
        }

        // Get the sender public key registered in the channel
        let sender_public_key = NearPublicKey::from_str(&channel_row.sender_pk).map_err(|e| {
            ProviderError::Channel(ChannelError::InvalidPublicKey(format!(
                "Error deserializing sender public key in contract: {}",
                e
            )))
        })?;

        // validate signature with the senders public key
        // this assumes that the sender is the only one who can sign the state
        let data = to_vec(&signed_state.state).map_err(|e| {
            ProviderError::SignedState(SignedStateError::SerializationError(e.to_string()))
        })?;
        if !signed_state.signature.verify(&data, &sender_public_key) {
            return Err(ProviderError::SignedState(
                SignedStateError::InvalidSignature,
            ));
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
            return Err(ProviderError::SignedState(
                SignedStateError::NonMonotonicSpentBalance(format!(
                    "New spent balance must monotonically increase. Current spent balance: {} <= Previous spent balance: {}",
                    new_spent_balance, most_recent_spent_balance
                )),
            ));
        }

        // Check that the sender has authorized an amount above the minimum cost
        let new_spent_balance = signed_state.state.spent_balance.as_yoctonear();
        let prev_spend_balance = most_recent_spent_balance;
        if new_spent_balance < prev_spend_balance + min_cost {
            return Err(ProviderError::SignedState(
                SignedStateError::AmountTooSmall(format!(
                    "New spent balance {} is less than the minimum cost of {}",
                    NearToken::from_yoctonear(new_spent_balance).exact_amount_display(),
                    NearToken::from_yoctonear(min_cost).exact_amount_display()
                )),
            ));
        }

        // Check that the user does not have insufficient funds.
        // Insufficient funds means that the user has spent more than the added balance.
        // If insufficient funds, resync the channel and check again (unhappy path)
        // If still insufficient funds, tell the user they need to top up the channel
        let new_spent_balance = signed_state.state.spent_balance.as_yoctonear();
        let added_balance = channel_row.added_balance().as_yoctonear();
        if added_balance < new_spent_balance {
            // in case the channel is out of sync with the blockchain, resync and check again
            let resynced_channel_row = self
                .db
                .refresh_channel_row(&signed_state.state.channel_id)
                .await?;

            let resynced_spent_balance = resynced_channel_row.added_balance().as_yoctonear();
            if new_spent_balance > resynced_spent_balance {
                return Err(ProviderError::SignedState(
                    SignedStateError::InsufficientFunds(format!(
                        "New spent balance is greater than the added balance by {} units. Please top up the channel.",
                        new_spent_balance - resynced_spent_balance
                    )),
                ));
            }
        }

        self.db.insert_signed_state(signed_state).await?;

        Ok(())
    }

    pub async fn close_pc(&self, channel_name: &str) -> ProviderResult<NearSignedState> {
        // TODO: Require signature from the sender to close the channel, so no other user can send the close request
        let channel_row = self.db.get_channel_row(channel_name).await?;

        info!("Closing channel: {}", channel_row.name);

        // Check if there is the sender has spent money that we haven't withdrawn yet
        if let Some(signed_state) = self.db.latest_signed_state(&channel_row.name).await? {
            info!(
                "There is a signed state: {:?}",
                signed_state.spent_balance()
            );
            // TODO: Check the amount of money available is large enough so that it makes sense to withdraw it
            let state: NearSignedState = signed_state.into();

            info!("Withdrawing: {:?}", state);

            self.pc_client.withdraw(state).await;
        }

        // Payload to send to user to close the channel
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

    pub async fn get_stale_channels(
        &self,
        stale_threshold: Duration,
        batch_size: Option<u32>,
    ) -> ProviderResult<Vec<ChannelRow>> {
        self.db
            .get_stale_channels(stale_threshold, batch_size)
            .await
    }

    pub async fn refresh_channel_row(&self, channel_name: &str) -> ProviderResult<ChannelRow> {
        self.db.refresh_channel_row(channel_name).await
    }
}

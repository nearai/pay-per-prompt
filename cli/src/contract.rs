use crate::{client::Client, config::Config, provider::Details, utils::find_signer};
use near_crypto::InMemorySigner;
use near_primitives::types::AccountId;
use near_sdk::{Gas, NearToken};
use serde_json::json;

pub struct Contract {
    client: Client,
    signer: InMemorySigner,
    contract: AccountId,
}

impl Contract {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(&config.near_rpc_url),
            signer: find_signer(config.get_account_id()),
            contract: config.contract.clone(),
        }
    }

    pub async fn open_payment_channel(
        &self,
        channel_id: &String,
        receiver: &Details,
        sender: &Details,
        amount: NearToken,
    ) {
        self.client
            .change_call(
                &self.signer,
                self.contract.clone(),
                "open_channel",
                json!({
                    "channel_id": channel_id,
                    "receiver": receiver,
                    "sender": sender,
                }),
                Gas::from_tgas(40),
                amount,
            )
            .await;
    }
}

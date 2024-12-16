use crate::{
    client::Client,
    config::{Config, SignedState},
    provider::Details,
    utils::find_signer,
};
use near_crypto::{InMemorySigner, PublicKey};
use near_primitives::types::AccountId;
use near_sdk::{near, Gas, NearToken, Timestamp};
use serde_json::json;

#[near(serializers = [json])]
pub struct ContractAccount {
    pub account_id: AccountId,
    pub public_key: PublicKey,
}

#[near(serializers = [json])]
pub struct ContractChannel {
    pub receiver: ContractAccount,
    pub sender: ContractAccount,
    pub added_balance: NearToken,
    pub withdrawn_balance: NearToken,
    pub force_close_started: Option<Timestamp>,
}

pub struct Contract {
    client: Client,
    signer: InMemorySigner,
    contract: AccountId,
}

impl Contract {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(&config.near_rpc_url, config.verbose),
            signer: find_signer(config.get_account_id()),
            contract: config.contract.clone(),
        }
    }

    pub async fn open_payment_channel(
        &self,
        channel_id: &str,
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
                // TODO: Adjust this amount (make sure it is enough)
                Gas::from_tgas(40),
                amount,
            )
            .await;
    }

    pub async fn withdraw(&self, state: SignedState) {
        self.client
            .change_call(
                &self.signer,
                self.contract.clone(),
                "withdraw",
                json!({"state" : state}),
                // TODO: Adjust this amount (make sure it is enough)
                Gas::from_tgas(40),
                NearToken::from_yoctonear(0),
            )
            .await;
    }

    pub async fn channel(&self, channel_id: &str) -> ContractChannel {
        self.client
            .view_call(
                self.contract.clone(),
                "channel",
                json!({"channel_id": channel_id}),
            )
            .await
    }
}

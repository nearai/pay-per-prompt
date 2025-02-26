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
#[derive(Clone, Debug)]
pub struct ContractAccount {
    pub account_id: AccountId,
    pub public_key: PublicKey,
}

#[near(serializers = [json])]
#[derive(Debug)]
pub struct ContractChannel {
    pub receiver: ContractAccount,
    pub sender: ContractAccount,
    pub added_balance: NearToken,
    pub withdrawn_balance: NearToken,
    pub force_close_started: Option<Timestamp>,
}

impl ContractChannel {
    pub fn is_closed(&self) -> bool {
        self.added_balance.is_zero()
            && self.sender.account_id
                == "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse::<AccountId>()
                    .unwrap()
    }
}

#[derive(Clone)]
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

    pub fn new_with_signer(config: &Config, signer: InMemorySigner) -> Self {
        Self {
            client: Client::new(&config.near_rpc_url, config.verbose),
            signer,
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

    pub async fn channel(&self, channel_id: &str) -> Option<ContractChannel> {
        self.client
            .view_call(
                self.contract.clone(),
                "channel",
                json!({"channel_id": channel_id}),
            )
            .await
    }

    pub async fn close(&self, state: SignedState) {
        self.client
            .change_call(
                &self.signer,
                self.contract.clone(),
                "close",
                json!({"state" : state}),
                // TODO: Adjust this amount (make sure it is enough)
                Gas::from_tgas(15),
                NearToken::from_yoctonear(0),
            )
            .await;
    }

    pub async fn withdraw_and_close(&self, state: SignedState, close: SignedState) {
        self.client
            .change_call(
                &self.signer,
                self.contract.clone(),
                "withdraw_and_close",
                json!({"state" : state, "close" : close}),
                Gas::from_tgas(15),
                NearToken::from_yoctonear(0),
            )
            .await;
    }

    pub async fn topup(&self, channel_id: &str, amount: NearToken) {
        self.client
            .change_call(
                &self.signer,
                self.contract.clone(),
                "topup",
                json!({"channel_id": channel_id}),
                Gas::from_tgas(15),
                amount,
            )
            .await;
    }
}

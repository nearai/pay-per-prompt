use near_crypto::PublicKey;
use near_sdk::{json_types::U128, AccountId};
use serde::{Deserialize, Serialize};

use crate::config::SignedState;

pub struct Provider {
    provider_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Details {
    pub account_id: AccountId,
    pub public_key: PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SpentBalance {
    pub spent_balance: U128,
}

impl Provider {
    pub fn new(provider_url: String) -> Self {
        Self { provider_url }
    }

    pub async fn receiver_details(&self) -> Details {
        reqwest::get(format!("{}/info", self.provider_url))
            .await
            .unwrap()
            .json::<Details>()
            .await
            .unwrap()
    }

    pub async fn spent_balance(&self, channel_id: &str) -> SpentBalance {
        reqwest::get(format!("{}/pc/state/{}", self.provider_url, channel_id))
            .await
            .unwrap()
            .json::<SpentBalance>()
            .await
            .unwrap()
    }

    pub async fn close_payload(&self, channel_id: &str, signed_state_payload: &str) -> SignedState {
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/pc/close/{}", self.provider_url, channel_id))
            .body(signed_state_payload.to_string())
            .send()
            .await
            .unwrap();
        if response.status().is_success() {
            return response.json::<SignedState>().await.unwrap();
        } else {
            panic!(
                "Failed to close channel: {}",
                response.text().await.unwrap()
            );
        }
    }
}

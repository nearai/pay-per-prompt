use near_crypto::PublicKey;
use near_sdk::AccountId;
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

    pub async fn close_payload(&self, channel_id: &str) -> SignedState {
        reqwest::get(format!("{}/pc/close/{}", self.provider_url, channel_id))
            .await
            .unwrap()
            .json::<SignedState>()
            .await
            .unwrap()
    }
}

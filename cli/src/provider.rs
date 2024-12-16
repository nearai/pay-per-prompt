use near_crypto::PublicKey;
use near_sdk::AccountId;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

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

    pub async fn receiver_details(&self) -> Result<Details, Box<dyn std::error::Error>> {
        // TODO: Call provider to get receiver details
        Ok(Details {
            account_id: "efa5fa531cdd76056d33fc6928af85fcc4b0c0cb06bee88be14f3f18b7ca2a4a"
                .parse()?,
            public_key: PublicKey::from_str(
                "ed25519:H8VERRt55YvExnvRP2yjqWeYzQvgGcq3RLi2utZGvwpM",
            )?,
        })
    }

    pub async fn close_payload(&self, channel_id: &str) -> SignedState {
        // TODO: Call provider to get close payload
        todo!()
    }
}

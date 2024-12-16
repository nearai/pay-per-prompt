use crate::{
    config::{Config, ConfigUpdate},
    provider::{Details, Provider},
};
use near_sdk::{AccountId, NearToken};

pub async fn open_payment_channel_command(
    config: &Config,
    amount: NearToken,
) -> Result<(), Box<dyn std::error::Error>> {
    let account_id = config.get_account_id();
    let provider = Provider::new(config.provider_url.clone());

    // Fetch provider details and update local storage with the new information
    let details = provider.receiver_details().await?;
    config.update_provider(&details);

    // Generate new key pair for the channel
    let sk = near_crypto::SecretKey::from_random(near_crypto::KeyType::ED25519);
    let pk = sk.public_key();
    let sender = Details {
        account_id,
        public_key: pk,
    };

    let channel_id = uuid::Uuid::new_v4().to_string();

    let near_contract = config.near_contract();
    near_contract
        .open_payment_channel(channel_id, details, sender, amount)
        .await;

    Ok(())
}

pub fn config_command(mut config: Config, update: &ConfigUpdate) {
    match update {
        ConfigUpdate::AccountId { account_id } => {
            config.account_id = Some(account_id.parse::<AccountId>().unwrap())
        }
    }
    config.save();
}

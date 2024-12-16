use crate::{
    config::{Channel, Config, ConfigUpdate},
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
        .open_payment_channel(&channel_id, &details, &sender, amount)
        .await;

    let channel = Channel {
        channel_id,
        receiver: details,
        sender,
        sender_secret_key: sk,
        spent_balance: NearToken::from_yoctonear(0),
        added_balance: NearToken::from_yoctonear(0),
        withdrawn_balance: NearToken::from_yoctonear(0),
    };

    // Save channel information to local storage
    config.update_channel(&channel);

    Ok(())
}

pub fn config_command(mut config: Config, update: &ConfigUpdate) {
    match update {
        ConfigUpdate::AccountId { account_id } => {
            config.account_id = Some(account_id.parse::<AccountId>().unwrap())
        }
    }
    config.save();

    println!("\nConfig updated:");
    serde_json::to_writer_pretty(std::io::stdout(), &config).unwrap();
}

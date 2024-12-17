use crate::{
    config::{Channel, Config, ConfigUpdate, SignedState},
    provider::{Details, Provider},
    utils::{find_only_channel_id, find_signer},
};
use base64::{prelude::BASE64_STANDARD, Engine};
use near_sdk::{AccountId, NearToken};

pub async fn open_payment_channel_command(
    config: &Config,
    amount: NearToken,
) -> Result<(), Box<dyn std::error::Error>> {
    let account_id = config.get_account_id();
    let provider = Provider::new(config.provider_url.clone());

    // Fetch provider details and update local storage with the new information
    let details = provider.receiver_details().await;
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
        added_balance: amount,
        withdrawn_balance: NearToken::from_yoctonear(0),
        force_close_started: None,
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

pub async fn info_command(config: &Config, channel_id: Option<String>, update: bool) {
    let channel_id = channel_id.unwrap_or_else(find_only_channel_id);
    let mut channel = Channel::load(&channel_id, true);

    if update {
        let contract = config.near_contract();
        let updated_channel = contract.channel(&channel_id).await;
        if let Some(updated_channel) = updated_channel {
            if config.verbose {
                println!(
                    "\nChannel details from the contract:\n{}\n",
                    near_sdk::serde_json::to_string_pretty(&updated_channel).unwrap()
                );
            }

            if updated_channel.is_closed() {
                eprintln!("Channel {} is closed. Removing it.", channel_id);
                let source = crate::config::channel_file(&channel_id);
                let target = crate::config::closed_channel_file(&channel_id);
                let folder = target.parent().unwrap();
                if !folder.exists() {
                    std::fs::create_dir_all(folder).unwrap();
                }

                // Remove channel from local
                std::fs::copy(&source, &target).unwrap();
                std::fs::remove_file(&source).unwrap();
                std::process::exit(1);
            }

            if channel.update_if_newer(updated_channel, config.verbose) {
                if config.verbose {
                    println!(
                        "\nChannel details:\n{}\n",
                        near_sdk::serde_json::to_string_pretty(&channel.redacted()).unwrap()
                    );
                }
            }
        } else {
            eprintln!("Channel {} not found", channel_id);
        }
    }
}

pub fn send_command(config: &Config, amount: NearToken, channel_id: Option<String>, update: bool) {
    let channel_id = channel_id.unwrap_or_else(find_only_channel_id);
    let mut channel = Channel::load(&channel_id, config.verbose);

    let new_balance = channel.spent_balance.saturating_add(amount);

    if new_balance > channel.added_balance {
        eprintln!(
            "Amount exceeds the available balance. Current balance: {}, Sending: {}",
            channel.available_balance(),
            amount
        );
        std::process::exit(1);
    }

    channel.spent_balance = new_balance;

    if config.verbose {
        println!(
            "\nState of the channel signed:\n{}\n",
            serde_json::to_string_pretty(&channel.payload()).unwrap()
        );
    }

    println!("\nPayload:\n{}\n", channel.payload_b64());

    if update {
        channel.save(config.verbose);
    }
}

pub async fn withdraw_command(config: &Config, payload: String) {
    let contract = config.near_contract();
    let raw = BASE64_STANDARD.decode(payload).unwrap();
    let state: SignedState = near_sdk::borsh::from_slice(&raw).unwrap();

    let channel = Channel::load(&state.state.channel_id, config.verbose);

    if config.verbose {
        println!(
            "\nWithdrawing from the channel:\n{}\n",
            serde_json::to_string_pretty(&state).unwrap()
        );
    }

    if state.state.spent_balance <= channel.withdrawn_balance {
        eprintln!("Nothing to withdraw");
        std::process::exit(1);
    }

    contract.withdraw(state).await;
}

pub fn close_payload_command(config: &Config, channel_id: Option<String>) {
    let channel_id = channel_id.unwrap_or_else(find_only_channel_id);
    let channel = Channel::load(&channel_id, config.verbose);

    let receiver_id = channel.receiver.account_id.clone();
    let signer: near_crypto::InMemorySigner = find_signer(receiver_id);

    let state = crate::config::State {
        channel_id: channel_id.clone(),
        spent_balance: NearToken::from_near(0),
    };

    let raw_state = near_sdk::borsh::to_vec(&state).unwrap();
    let signed_state = crate::config::SignedState {
        state,
        signature: signer.sign(&raw_state),
    };

    if config.verbose {
        println!(
            "\nState of the channel signed:\n{}\n",
            serde_json::to_string_pretty(&signed_state).unwrap()
        );
    }

    println!(
        "\nPayload:\n{}\n",
        BASE64_STANDARD.encode(&near_sdk::borsh::to_vec(&signed_state).unwrap())
    );
}

pub async fn close_command(config: &Config, channel_id: Option<String>, payload: Option<String>) {
    let channel_id = channel_id.unwrap_or_else(find_only_channel_id);
    let _ = Channel::load(&channel_id, config.verbose);

    let signed_state = if let Some(payload) = payload {
        let raw = BASE64_STANDARD.decode(payload);
        near_sdk::borsh::from_slice(&raw.unwrap()).unwrap()
    } else {
        let provider = Provider::new(config.provider_url.clone());
        provider.close_payload(&channel_id).await
    };

    let contract = config.near_contract();
    contract.close(signed_state).await;

    println!("\nChannel closed. Use `info` to check the channel was closed locally.")
}

pub async fn topup_command(config: &Config, channel_id: Option<String>, amount: NearToken) {
    let channel_id = channel_id.unwrap_or_else(find_only_channel_id);
    let mut channel = Channel::load(&channel_id, config.verbose);

    if channel.force_close_started.is_some() {
        eprintln!("\nChannel is already closing\n");
        std::process::exit(1);
    }

    let contract = config.near_contract();
    contract.topup(&channel_id, amount).await;

    channel.added_balance = channel.added_balance.saturating_add(amount);
    channel.save(config.verbose);

    println!("\nChannel topped up\n");
}

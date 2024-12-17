use near_crypto::{InMemorySigner, SecretKey};
use near_sdk::AccountId;
use std::{path::PathBuf, str::FromStr};

use crate::config::{data_storage, Channel};

fn find_on_path(path: PathBuf, target: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap().path();
        if entry.is_file() && entry.file_name().unwrap().to_str() == Some(target) {
            return Some(entry);
        }

        if entry.is_dir() {
            if let Some(found) = find_on_path(entry, target) {
                return Some(found);
            }
        }
    }
    None
}

pub fn find_signer(account_id: AccountId) -> near_crypto::InMemorySigner {
    let path = dirs::home_dir().unwrap().join(".near-credentials");
    find_on_path(path, &format!("{}.json", account_id))
        .map(|path| load_memory_signer(account_id, path))
        .unwrap()
}

fn load_memory_signer(account_id: AccountId, path: PathBuf) -> InMemorySigner {
    let value =
        serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(path).unwrap()).unwrap();

    let sk = value.get("private_key").unwrap().as_str().unwrap();
    let sk = SecretKey::from_str(sk).unwrap();

    InMemorySigner::from_secret_key(account_id, sk)
}

pub fn find_only_channel_id() -> String {
    let mut channels = std::fs::read_dir(data_storage().join("channels"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|e| e.is_file() && e.extension() == Some("json".as_ref()))
        .map(|e| serde_json::from_str::<Channel>(&std::fs::read_to_string(&e).unwrap()).unwrap());

    let first = channels.next().expect("No channels found");

    if channels.next().is_some() {
        eprintln!("Multiple channels found. Please specify the channel id.");
        std::process::exit(1);
    }

    first.channel_id
}

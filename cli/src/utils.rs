use near_sdk::AccountId;
use std::path::PathBuf;

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
        .and_then(|path| near_crypto::InMemorySigner::from_file(&path).ok())
        .unwrap()
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

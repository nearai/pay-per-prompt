use near_sdk::AccountId;
use std::path::PathBuf;

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
        .and_then(|path| {
            println!("{:?}", path);
            near_crypto::InMemorySigner::from_file(&path).ok()
        })
        .unwrap()
}

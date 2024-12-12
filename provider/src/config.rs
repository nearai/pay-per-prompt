use anyhow::Error;
use serde::Deserialize;

use crate::MODEL_DELIMITER;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ProviderConfig {
    #[serde(default)]
    pub providers: Vec<Provider>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Provider {
    pub canonical_name: String,
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    pub provider: String,
    pub model_name: String,
}

impl ModelInfo {
    pub fn new(provider: String, model_name: String) -> Self {
        Self {
            provider,
            model_name,
        }
    }

    pub fn from_str(input: &str) -> anyhow::Result<ModelInfo> {
        if let Some((provider, model_name)) = input.split_once(MODEL_DELIMITER) {
            let provider = provider.trim();
            let model_name = model_name.trim();
            if provider.is_empty() || model_name.is_empty() {
                return Err(Error::msg(
                    "Invalid input format. Provider or model name cannot be empty.",
                ));
            }
            Ok(ModelInfo {
                provider: provider.to_string(),
                model_name: model_name.to_string(),
            })
        } else {
            Err(Error::msg(
                "Invalid input format. Expected exactly one '::' delimiter.",
            ))
        }
    }
}

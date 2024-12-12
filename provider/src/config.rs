use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ProviderConfig {
    #[serde(default)]
    pub providers: Vec<Provider>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Provider {
    canonical_name: String,
    url: String,
    api_key: String,
}

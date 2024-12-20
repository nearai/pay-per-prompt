use http::StatusCode;
use std::fmt::Display;

#[derive(Debug)]
pub enum ProviderError {
    Channel(ChannelError),
    SignedState(SignedStateError),
    DBError(sqlx::Error),
}

#[derive(Debug)]
pub enum ChannelError {
    NotFound,
    Closed,
    InvalidOwner(String),
    InvalidPublicKey(String),
    WithdrawTooSmall,
    NonMonotonicWithdraw,
}

#[derive(Debug)]
pub enum SignedStateError {
    SerializationError(String),
    InvalidSignature,
    NonMonotonicSpentBalance(String),
    AmountTooSmall(String),
    InsufficientFunds(String),
    InvalidAddedBalance(String),
}

#[derive(Debug)]
pub struct UserFacingError(String);

impl Display for UserFacingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&ProviderError> for UserFacingError {
    fn from(error: &ProviderError) -> Self {
        match error {
            ProviderError::Channel(ChannelError::NotFound) => {
                UserFacingError("Payment channel not found".to_string())
            }
            ProviderError::Channel(ChannelError::Closed) => {
                UserFacingError("Payment channel closed".to_string())
            }
            ProviderError::Channel(ChannelError::InvalidPublicKey(e)) => {
                UserFacingError(format!("Invalid public key: {}", e))
            }
            ProviderError::Channel(ChannelError::InvalidOwner(e)) => {
                UserFacingError(format!("Invalid owner: {}", e))
            }

            ProviderError::SignedState(SignedStateError::SerializationError(e)) => {
                UserFacingError(format!(
                    "Unable to deserialize borsh serialized SignedState from payment header: {}",
                    e
                ))
            }
            ProviderError::SignedState(SignedStateError::InvalidSignature) => {
                UserFacingError("Invalid signature".to_string())
            }
            ProviderError::SignedState(SignedStateError::NonMonotonicSpentBalance(e)) => {
                UserFacingError(format!("Non-monotonic spent balance: {}", e))
            }
            ProviderError::SignedState(SignedStateError::AmountTooSmall(e)) => {
                UserFacingError(format!("Amount too small: {}", e))
            }
            ProviderError::SignedState(SignedStateError::InsufficientFunds(e)) => {
                UserFacingError(format!("Insufficient funds: {}", e))
            }
            ProviderError::SignedState(SignedStateError::InvalidAddedBalance(e)) => {
                UserFacingError(format!("Invalid added balance: {}", e))
            }

            // Probobally not the best idea to expose the internal database error to users
            ProviderError::DBError(e) => UserFacingError(format!("Internal database error: {}", e)),

            _ => UserFacingError("Internal server error".to_string()),
        }
    }
}

impl From<&ProviderError> for StatusCode {
    fn from(error: &ProviderError) -> Self {
        match error {
            ProviderError::Channel(ChannelError::NotFound) => StatusCode::NOT_FOUND,
            ProviderError::Channel(ChannelError::Closed) => StatusCode::BAD_REQUEST,
            ProviderError::Channel(ChannelError::InvalidOwner(_)) => StatusCode::BAD_REQUEST,
            ProviderError::Channel(ChannelError::InvalidPublicKey(_)) => StatusCode::BAD_REQUEST,
            ProviderError::SignedState(SignedStateError::InvalidSignature) => {
                StatusCode::BAD_REQUEST
            }
            ProviderError::SignedState(SignedStateError::NonMonotonicSpentBalance(_)) => {
                StatusCode::BAD_REQUEST
            }
            ProviderError::SignedState(SignedStateError::AmountTooSmall(_)) => {
                StatusCode::BAD_REQUEST
            }
            ProviderError::SignedState(SignedStateError::InsufficientFunds(_)) => {
                StatusCode::BAD_REQUEST
            }
            ProviderError::SignedState(SignedStateError::InvalidAddedBalance(_)) => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

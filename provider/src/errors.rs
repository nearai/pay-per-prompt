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
    // Missing channel errors
    NotFoundInDB,
    NotFoundInContract,

    // Closed channel errors
    HardClosed(String),
    SoftClosed(String),
    Closing(String),

    // Withdraw errors
    WithdrawTooSmall(String),
    WithdrawNonMonotonic,

    // Invalid errors
    InvalidOwner(String),
    InvalidPublicKey(String),
}

#[derive(Debug)]
pub enum SignedStateError {
    // Validation errors
    SerializationError(String),
    InvalidSignature,
    InvalidClosedSignedState(String),

    // Spend errors
    NonMonotonicSpentBalance(String),
    PaymentTooSmall(String),
    InsufficientFunds(String),
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
            //
            // Channel errors
            //
            ProviderError::Channel(ChannelError::NotFoundInDB) => {
                UserFacingError("Payment channel not found".to_string())
            }
            ProviderError::Channel(ChannelError::NotFoundInContract) => {
                UserFacingError("Payment channel not found".to_string())
            }
            ProviderError::Channel(ChannelError::HardClosed(e)) => {
                UserFacingError(format!("Payment channel hard closed: {}", e))
            }
            ProviderError::Channel(ChannelError::SoftClosed(e)) => {
                UserFacingError(format!("Payment channel soft closed: {}", e))
            }
            ProviderError::Channel(ChannelError::Closing(e)) => {
                UserFacingError(format!("Payment channel closing: {}", e))
            }
            ProviderError::Channel(ChannelError::InvalidOwner(e)) => {
                UserFacingError(format!("Invalid owner: {}", e))
            }
            ProviderError::Channel(ChannelError::InvalidPublicKey(e)) => {
                UserFacingError(format!("Invalid public key: {}", e))
            }
            ProviderError::Channel(ChannelError::WithdrawTooSmall(e)) => {
                UserFacingError(format!("Withdraw too small: {}", e))
            }
            ProviderError::Channel(ChannelError::WithdrawNonMonotonic) => {
                UserFacingError("Non-monotonic withdraw".to_string())
            }

            //
            // SignedState errors
            //
            ProviderError::SignedState(SignedStateError::SerializationError(e)) => {
                UserFacingError(format!("Unable to deserialize SignedState: {}", e))
            }
            ProviderError::SignedState(SignedStateError::InvalidSignature) => {
                UserFacingError("Invalid signature".to_string())
            }
            ProviderError::SignedState(SignedStateError::NonMonotonicSpentBalance(e)) => {
                UserFacingError(format!("Non-monotonic spent balance: {}", e))
            }
            ProviderError::SignedState(SignedStateError::PaymentTooSmall(e)) => {
                UserFacingError(format!("Payment too small: {}", e))
            }
            ProviderError::SignedState(SignedStateError::InsufficientFunds(e)) => {
                UserFacingError(format!("Insufficient funds: {}", e))
            }
            ProviderError::SignedState(SignedStateError::InvalidClosedSignedState(e)) => {
                UserFacingError(format!("Invalid signed state: {}", e))
            }

            // Probobally not the best idea to expose the internal database error to users
            ProviderError::DBError(e) => UserFacingError(format!("Internal database error: {}", e)),
        }
    }
}

impl From<&ProviderError> for StatusCode {
    fn from(error: &ProviderError) -> Self {
        match error {
            ProviderError::Channel(ChannelError::NotFoundInDB) => StatusCode::NOT_FOUND,
            ProviderError::Channel(ChannelError::NotFoundInContract) => StatusCode::NOT_FOUND,
            ProviderError::Channel(ChannelError::Closing(_)) => StatusCode::BAD_REQUEST,
            ProviderError::Channel(ChannelError::HardClosed(_)) => StatusCode::BAD_REQUEST,
            ProviderError::Channel(ChannelError::SoftClosed(_)) => StatusCode::BAD_REQUEST,
            ProviderError::Channel(ChannelError::InvalidOwner(_)) => StatusCode::BAD_REQUEST,
            ProviderError::Channel(ChannelError::InvalidPublicKey(_)) => StatusCode::BAD_REQUEST,
            ProviderError::Channel(ChannelError::WithdrawTooSmall(_)) => StatusCode::BAD_REQUEST,
            ProviderError::Channel(ChannelError::WithdrawNonMonotonic) => StatusCode::BAD_REQUEST,
            ProviderError::SignedState(SignedStateError::InvalidSignature) => {
                StatusCode::BAD_REQUEST
            }
            ProviderError::SignedState(SignedStateError::NonMonotonicSpentBalance(_)) => {
                StatusCode::BAD_REQUEST
            }
            ProviderError::SignedState(SignedStateError::PaymentTooSmall(_)) => {
                StatusCode::BAD_REQUEST
            }
            ProviderError::SignedState(SignedStateError::InsufficientFunds(_)) => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

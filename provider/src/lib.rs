pub mod background;
pub mod common;
pub mod db;
pub mod errors;
pub mod service;

pub use crate::background::*;
pub use crate::common::*;
pub use crate::db::*;
pub use crate::service::*;

use crate::errors::*;

pub type ProviderResult<T> = Result<T, ProviderError>;

pub const MODEL_DELIMITER: &str = "::";
pub const BAD_REQUEST: &str = "Bad Request";
pub const FOUR_HUNDRED: &str = "400";

pub const PAYMENTS_HEADER_NAME: &str = "X-Payments-Signature";

// When a channel is closed, the receiver / sender account id is set to this value
pub const CLOSED_CHANNEL_ACCOUNT_ID: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

pub mod background;
pub mod common;
pub mod db;
pub mod errors;
pub mod service;

use std::time::Duration;

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

// Amount of time until a channel is considered stale and the state should be
// refreshed from the contract
pub const STALE_CHANNEL_THRESHOLD: Duration = Duration::from_secs(30); // 30 seconds

// Copied from the contract code
pub const SECOND: u64 = 1_000_000_000;
pub const DAY: u64 = 24 * 60 * 60 * SECOND;
pub const HARD_CLOSE_TIMEOUT: u64 = 7 * DAY;

pub mod common;
pub mod db;
pub mod service;

pub use crate::common::*;
pub use crate::db::*;
pub use crate::service::*;

pub const MODEL_DELIMITER: &str = "::";
pub const BAD_REQUEST: &str = "Bad Request";
pub const FOUR_HUNDRED: &str = "400";

#![feature(const_trait_impl)]

pub mod config;
pub mod service;

pub use crate::config::*;
pub use crate::service::*;

pub const MODEL_DELIMITER: &str = "::";
pub const BAD_REQUEST: &str = "Bad Request";
pub const FOUR_HUNDRED: &str = "400";

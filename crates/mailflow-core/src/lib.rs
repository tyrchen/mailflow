/// Mailflow Core - Shared library for Mailflow email dispatching system
///
/// This crate contains shared types, traits, and utilities used across
/// the Mailflow worker and API Lambda functions.
pub mod constants;
pub mod email;
pub mod error;
pub mod models;
pub mod routing;
pub mod services;
pub mod utils;

// Re-export commonly used types
pub use error::MailflowError;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Mailflow Worker - Email processing Lambda
///
/// This module contains the email processing handlers for the Mailflow system.
pub mod handlers;

// Re-export commonly used items
pub use handlers::handler;
pub use mailflow_core::*;

// Library root - exports public API

pub mod constants;
pub mod email;
pub mod error;
pub mod handlers;
pub mod models;
pub mod routing;
pub mod services;
pub mod utils;

// Re-export commonly used types
pub use error::MailflowError;
pub use handlers::handler;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod config;
/// Data models for Mailflow system
pub mod email;
pub mod events;
pub mod messages;

// Re-export commonly used types
pub use config::*;
pub use email::*;
pub use events::*;
pub use messages::*;

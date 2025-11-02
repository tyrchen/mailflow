//! Common test utilities and helpers for integration tests
#![allow(dead_code)]

use std::path::PathBuf;

pub mod mock_aws;
pub mod test_data;

/// Get path to test fixtures directory
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Load a test email fixture
pub fn load_email_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join("emails").join(name);
    std::fs::read(&path).unwrap_or_else(|_| panic!("Failed to read fixture: {:?}", path))
}

/// Load a test attachment fixture
pub fn load_attachment_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join("attachments").join(name);
    std::fs::read(&path).unwrap_or_else(|_| panic!("Failed to read fixture: {:?}", path))
}

/// Load a test message JSON fixture
pub fn load_message_fixture(name: &str) -> String {
    let path = fixtures_dir().join("messages").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Failed to read fixture: {:?}", path))
}

/// Generate a unique test message ID
pub fn generate_test_message_id() -> String {
    format!("test-{}", uuid::Uuid::new_v4())
}

/// Generate a unique correlation ID for tests
pub fn generate_correlation_id() -> String {
    format!("test-corr-{}", uuid::Uuid::new_v4())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixtures_dir() {
        let dir = fixtures_dir();
        assert!(dir.to_str().unwrap().contains("tests/fixtures"));
    }

    #[test]
    fn test_generate_test_message_id() {
        let id1 = generate_test_message_id();
        let id2 = generate_test_message_id();
        assert!(id1.starts_with("test-"));
        assert!(id2.starts_with("test-"));
        assert_ne!(id1, id2);
    }
}

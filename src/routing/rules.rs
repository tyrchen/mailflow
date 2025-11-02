/// Routing rules and destination

#[derive(Debug, Clone)]
pub struct RouteDestination {
    pub app_name: String,
    pub queue_url: String,
}

/// Extract app name from email address (e.g., _app1@acme.com -> app1)
pub fn extract_app_name(email: &str) -> Option<String> {
    let local_part = email.split('@').next()?;

    local_part.strip_prefix('_').map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_app_name() {
        assert_eq!(extract_app_name("_app1@acme.com"), Some("app1".to_string()));
        assert_eq!(
            extract_app_name("_invoices@example.com"),
            Some("invoices".to_string())
        );
        assert_eq!(extract_app_name("user@acme.com"), None);
        assert_eq!(extract_app_name("invalid"), None);
    }
}

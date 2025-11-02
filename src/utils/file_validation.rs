/// File type validation using magic bytes and extension checking
use crate::error::MailflowError;

/// Allowed file types with their magic bytes signatures
/// Format: (mime_type, extension, magic_bytes)
const ALLOWED_FILE_SIGNATURES: &[(&str, &str, &[u8])] = &[
    // Images
    ("image/jpeg", "jpg", &[0xFF, 0xD8, 0xFF]),
    ("image/jpeg", "jpeg", &[0xFF, 0xD8, 0xFF]),
    ("image/png", "png", &[0x89, 0x50, 0x4E, 0x47]),
    ("image/gif", "gif", &[0x47, 0x49, 0x46, 0x38]),
    ("image/webp", "webp", &[0x52, 0x49, 0x46, 0x46]), // RIFF
    ("image/bmp", "bmp", &[0x42, 0x4D]),
    ("image/tiff", "tiff", &[0x49, 0x49, 0x2A, 0x00]),
    ("image/tiff", "tif", &[0x49, 0x49, 0x2A, 0x00]),
    // Documents
    ("application/pdf", "pdf", &[0x25, 0x50, 0x44, 0x46]), // %PDF
    // Office formats (ZIP-based)
    (
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "docx",
        &[0x50, 0x4B, 0x03, 0x04],
    ), // PK..
    (
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "xlsx",
        &[0x50, 0x4B, 0x03, 0x04],
    ),
    (
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "pptx",
        &[0x50, 0x4B, 0x03, 0x04],
    ),
    // Archives (but verify extension to distinguish from Office)
    ("application/zip", "zip", &[0x50, 0x4B, 0x03, 0x04]),
    // Text
    ("text/plain", "txt", &[]), // No magic bytes, any content allowed
    ("text/csv", "csv", &[]),
    ("text/html", "html", &[]),
    ("text/xml", "xml", &[]),
    ("application/json", "json", &[]),
];

/// Blocked file extensions (executables, scripts, etc.)
const BLOCKED_EXTENSIONS: &[&str] = &[
    "exe", "bat", "cmd", "com", "pif", "scr", "vbs", "js", "jar", "msi", "app", "deb", "rpm",
    "dmg", "pkg", "sh", "bash", "ps1", "dll", "so", "dylib", "sys", "ocx",
];

/// Check if file extension is blocked
pub fn is_extension_blocked(filename: &str) -> bool {
    if let Some(ext) = get_extension(filename) {
        BLOCKED_EXTENSIONS.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}

/// Get file extension from filename
fn get_extension(filename: &str) -> Option<String> {
    if !filename.contains('.') {
        return None;
    }
    filename
        .rsplit('.')
        .next()
        .filter(|ext| !ext.is_empty())
        .map(|ext| ext.to_lowercase())
}

/// Validate file type based on magic bytes and extension
///
/// Returns Ok(mime_type) if valid, Err if invalid
pub fn validate_file_type(
    filename: &str,
    content: &[u8],
    declared_content_type: &str,
) -> Result<String, MailflowError> {
    // Check if extension is blocked
    if is_extension_blocked(filename) {
        return Err(MailflowError::Validation(format!(
            "File type not allowed: {} (blocked extension)",
            filename
        )));
    }

    // Get extension
    let ext = get_extension(filename).ok_or_else(|| {
        MailflowError::Validation(format!("No file extension found: {}", filename))
    })?;

    // Check if extension is in allowed list
    let allowed_signatures: Vec<_> = ALLOWED_FILE_SIGNATURES
        .iter()
        .filter(|(_, allowed_ext, _)| *allowed_ext == ext)
        .collect();

    if allowed_signatures.is_empty() {
        return Err(MailflowError::Validation(format!(
            "File type not allowed: .{} extension",
            ext
        )));
    }

    // For files with magic bytes, verify them
    for (mime_type, _, magic_bytes) in allowed_signatures {
        if magic_bytes.is_empty() {
            // Text files without magic bytes - allow
            return Ok(mime_type.to_string());
        }

        if content.len() >= magic_bytes.len() && &content[..magic_bytes.len()] == *magic_bytes {
            tracing::debug!(
                filename = %filename,
                detected_mime = %mime_type,
                declared_mime = %declared_content_type,
                "File type validated via magic bytes"
            );
            return Ok(mime_type.to_string());
        }
    }

    // Magic bytes didn't match
    Err(MailflowError::Validation(format!(
        "File type mismatch: {} has extension .{} but magic bytes don't match expected signature",
        filename, ext
    )))
}

/// Get allowed file types for configuration/documentation
pub fn get_allowed_extensions() -> Vec<&'static str> {
    ALLOWED_FILE_SIGNATURES
        .iter()
        .map(|(_, ext, _)| *ext)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocked_extensions() {
        assert!(is_extension_blocked("virus.exe"));
        assert!(is_extension_blocked("script.bat"));
        assert!(is_extension_blocked("malware.vbs"));
        assert!(is_extension_blocked("file.EXE")); // Case insensitive
        assert!(!is_extension_blocked("document.pdf"));
        assert!(!is_extension_blocked("image.jpg"));
    }

    #[test]
    fn test_get_extension() {
        assert_eq!(get_extension("file.txt"), Some("txt".to_string()));
        assert_eq!(get_extension("file.PDF"), Some("pdf".to_string()));
        assert_eq!(get_extension("file.tar.gz"), Some("gz".to_string()));
        assert_eq!(get_extension("noextension"), None);
        assert_eq!(get_extension("file."), None);
    }

    #[test]
    fn test_validate_pdf() {
        let pdf_magic = vec![0x25, 0x50, 0x44, 0x46, 0x2D, 0x31, 0x2E, 0x34]; // %PDF-1.4
        let result = validate_file_type("document.pdf", &pdf_magic, "application/pdf");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "application/pdf");
    }

    #[test]
    fn test_validate_jpeg() {
        let jpeg_magic = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]; // JPEG header
        let result = validate_file_type("photo.jpg", &jpeg_magic, "image/jpeg");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "image/jpeg");
    }

    #[test]
    fn test_validate_png() {
        let png_magic = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let result = validate_file_type("image.png", &png_magic, "image/png");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "image/png");
    }

    #[test]
    fn test_validate_text_file() {
        let content = b"This is plain text content";
        let result = validate_file_type("notes.txt", content, "text/plain");
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_wrong_magic_bytes() {
        let fake_pdf = vec![0x00, 0x00, 0x00, 0x00]; // Wrong magic bytes
        let result = validate_file_type("fake.pdf", &fake_pdf, "application/pdf");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("magic bytes"));
    }

    #[test]
    fn test_reject_blocked_extension() {
        let content = vec![0x4D, 0x5A]; // EXE header
        let result = validate_file_type("virus.exe", &content, "application/x-msdownload");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("blocked extension")
        );
    }

    #[test]
    fn test_reject_unknown_extension() {
        let content = vec![0x00, 0x00];
        let result = validate_file_type("file.xyz", &content, "application/octet-stream");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not allowed"));
    }

    #[test]
    fn test_get_allowed_extensions() {
        let exts = get_allowed_extensions();
        assert!(exts.contains(&"pdf"));
        assert!(exts.contains(&"jpg"));
        assert!(exts.contains(&"png"));
        assert!(exts.contains(&"docx"));
    }
}

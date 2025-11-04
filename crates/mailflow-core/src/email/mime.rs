/// MIME utilities
pub fn detect_content_type(filename: &str) -> &'static str {
    let extension = filename.rsplit('.').next().unwrap_or("");

    match extension.to_lowercase().as_str() {
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "html" => "text/html",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_content_type() {
        assert_eq!(detect_content_type("document.pdf"), "application/pdf");
        assert_eq!(detect_content_type("image.png"), "image/png");
        assert_eq!(
            detect_content_type("unknown.xyz"),
            "application/octet-stream"
        );
    }
}

/// Basic integration tests for fixture loading and common utilities
#[path = "common/mod.rs"]
mod common;

#[test]
fn test_load_simple_email_fixture() {
    let email = common::load_email_fixture("simple.eml");
    assert!(!email.is_empty());
    assert!(String::from_utf8_lossy(&email).contains("Test Email"));
}

#[test]
fn test_load_multipart_email_fixture() {
    let email = common::load_email_fixture("multipart.eml");
    assert!(!email.is_empty());
    assert!(String::from_utf8_lossy(&email).contains("multipart/alternative"));
}

#[test]
fn test_load_html_inline_images_fixture() {
    let email = common::load_email_fixture("html-inline-images.eml");
    assert!(!email.is_empty());
    assert!(String::from_utf8_lossy(&email).contains("inline"));
}

#[test]
fn test_load_threading_reply_fixture() {
    let email = common::load_email_fixture("threading-reply.eml");
    assert!(!email.is_empty());
    assert!(String::from_utf8_lossy(&email).contains("In-Reply-To"));
}

#[test]
fn test_load_utf8_special_chars_fixture() {
    let email = common::load_email_fixture("utf8-special-chars.eml");
    assert!(!email.is_empty());
    let content = String::from_utf8_lossy(&email);
    assert!(content.contains("🎉") || content.contains("UTF-8"));
}

#[test]
fn test_load_attachment_fixtures() {
    let pdf = common::load_attachment_fixture("test.pdf");
    assert!(!pdf.is_empty());
    assert_eq!(&pdf[0..4], b"%PDF");

    let png = common::load_attachment_fixture("test.png");
    assert!(!png.is_empty());
    assert_eq!(&png[0..4], &[0x89, 0x50, 0x4E, 0x47]);

    let jpg = common::load_attachment_fixture("test.jpg");
    assert!(!jpg.is_empty());
    assert_eq!(&jpg[0..2], &[0xFF, 0xD8]);
}

#[test]
fn test_load_message_fixtures() {
    let simple = common::load_message_fixture("outbound-simple.json");
    assert!(!simple.is_empty());
    assert!(simple.contains("version"));
    assert!(simple.contains("1.0"));

    let with_attachment = common::load_message_fixture("outbound-with-attachment.json");
    assert!(!with_attachment.is_empty());
    assert!(with_attachment.contains("attachments"));
}

#[test]
fn test_generate_unique_ids() {
    let id1 = common::generate_test_message_id();
    let id2 = common::generate_test_message_id();

    assert_ne!(id1, id2);
    assert!(id1.starts_with("test-"));
    assert!(id2.starts_with("test-"));

    let corr1 = common::generate_correlation_id();
    let corr2 = common::generate_correlation_id();

    assert_ne!(corr1, corr2);
    assert!(corr1.starts_with("test-corr-"));
    assert!(corr2.starts_with("test-corr-"));
}

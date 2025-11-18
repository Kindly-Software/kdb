//! Phase 4 Compilation Test
//!
//! Verifies that Phase 4 features compile correctly.

#[cfg(all(feature = "pdf-binary", feature = "async-pdf"))]
#[test]
fn test_async_pdf_compiles() {
    // This test just verifies the async module compiles
    use kindly_dedup::pdf_export::PdfExportProgressCapsule;

    let progress = PdfExportProgressCapsule::new();
    assert_eq!(progress.get_progress(), 0);
}

#[cfg(all(feature = "pdf-binary", feature = "pdf-a"))]
#[test]
fn test_pdfa_compiles() {
    // This test just verifies the PDF/A module compiles
    // Actual conversion requires ghostscript, so we don't test that here
}

#[cfg(all(feature = "pdf-binary", feature = "email-delivery"))]
#[test]
fn test_email_compiles() {
    // This test just verifies the email module compiles
    use kindly_dedup::pdf_export::EmailDeliveryConfig;

    let config = EmailDeliveryConfig::gmail("test@example.com", "password");
    assert_eq!(config.from_address, "test@example.com");
}

//! Phase 4 Async PDF Generation Example
//!
//! Demonstrates async PDF generation with progress tracking.

#[cfg(all(feature = "pdf-binary", feature = "async-pdf"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use kindly_dedup::pdf_export::{generate_pdf_async, PdfExportProgressCapsule};
    use kindly_dedup::protection::audit::SecurityAuditLogger;
    use std::path::Path;
    use std::sync::Arc;

    println!("Phase 4 Async PDF Generation Test");
    println!("==================================\n");

    // Create audit logger with some events
    use kindly_dedup::protection::audit::SecurityEventType;

    let logger = SecurityAuditLogger::new();
    logger.log_event(
        SecurityEventType::LicenseValidation,
        "test-customer",
        None,
        0,
        "Test event 1",
    )?;
    logger.log_event(
        SecurityEventType::LicenseValidation,
        "test-customer",
        None,
        0,
        "Test event 2",
    )?;
    logger.log_event(
        SecurityEventType::LicenseValidation,
        "test-customer",
        None,
        0,
        "Test event 3",
    )?;

    // Create progress tracker
    let progress = Arc::new(PdfExportProgressCapsule::new());

    // Output path
    let output = Path::new("/tmp/phase4_async_test.pdf");

    println!("Starting async PDF generation...");

    // Clone progress for monitoring
    let progress_monitor = progress.clone();

    // Spawn monitoring task
    let monitor_task = tokio::spawn(async move {
        loop {
            let current = progress_monitor.get_progress();
            println!("Progress: {}%", current);
            if current >= 100 {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    });

    // Generate PDF asynchronously
    let result = generate_pdf_async(logger, output, progress.clone()).await;

    // Wait for monitor to finish
    monitor_task.await?;

    match result {
        Ok(_) => {
            println!("\n✅ PDF generated successfully: {}", output.display());
            println!("Final progress: {}%", progress.get_progress());
        }
        Err(e) => {
            println!("\n❌ PDF generation failed: {}", e);
        }
    }

    Ok(())
}

#[cfg(not(all(feature = "pdf-binary", feature = "async-pdf")))]
fn main() {
    println!("This example requires features: pdf-binary, async-pdf");
    println!("Run with: cargo run --example test_phase4_async --features \"pdf-binary,async-pdf\"");
}

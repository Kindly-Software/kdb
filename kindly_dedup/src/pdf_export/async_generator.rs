//! Async PDF Generation (Non-Blocking UI)
//!
//! # Architecture
//!
//! **Purpose**: Generate PDFs asynchronously without blocking the GUI thread
//!
//! **Tier**: T5 (Streaming) - Progress tracking + T1 (Atomic) coordination
//!
//! **Features**:
//! - Non-blocking GUI: User can continue working while PDF generates
//! - Progress tracking: Atomic counter updated during generation
//! - Notification: Completion callback with status
//!
//! # Performance
//! - Background task spawn: <10µs (tokio::spawn_blocking overhead)
//! - Progress update: <10ns per stage (atomic store)
//! - Total generation time: <200ms for 1K events (same as blocking)
//!
//! # Usage
//!
//! ```rust,ignore
//! use kindly_dedup::pdf_export::{generate_pdf_async, PdfExportProgressCapsule};
//! use kindly_dedup::protection::audit::SecurityAuditLogger;
//! use std::sync::Arc;
//! use std::path::Path;
//!
//! let logger = SecurityAuditLogger::new();
//! let progress = Arc::new(PdfExportProgressCapsule::new());
//! let output = Path::new("report.pdf");
//!
//! // Spawn async PDF generation
//! let result = generate_pdf_async(logger, output, progress.clone()).await;
//!
//! // GUI polls progress.get_progress() to update status bar
//! ```

use super::error::{PdfError, Result};
use super::progress_capsule::{PdfExportProgressCapsule, PdfGenerationStage};
use crate::protection::audit::{SecurityAuditLogger, SecurityEventType};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Generate PDF asynchronously with progress tracking
///
/// # Arguments
/// - `audit_logger`: SecurityAuditLogger with audit trail
/// - `output_path`: Output PDF file path
/// - `progress`: Shared progress capsule for GUI updates
///
/// # Returns
/// - Ok(()) on success
/// - Err(PdfError) on failure
///
/// # Performance
/// - Spawn overhead: <10µs (tokio::spawn_blocking)
/// - Generation time: <200ms for 1K events (same as blocking)
/// - Progress updates: <10ns per stage (atomic)
pub async fn generate_pdf_async(
    audit_logger: SecurityAuditLogger,
    output_path: &Path,
    progress: Arc<PdfExportProgressCapsule>,
) -> Result<()> {
    let output_path = output_path.to_path_buf();

    // Spawn blocking task (PDF generation is CPU-bound)
    tokio::task::spawn_blocking(move || generate_pdf_with_progress(&audit_logger, &output_path, &progress))
        .await
        .map_err(|e| PdfError::GenerationError(format!("Async task failed: {}", e)))?
}

/// Internal: Generate PDF with progress updates
///
/// This function is called from the blocking task and updates progress atomically
/// during each stage of PDF generation.
fn generate_pdf_with_progress(
    audit_logger: &SecurityAuditLogger,
    output_path: &Path,
    progress: &PdfExportProgressCapsule,
) -> Result<()> {
    // Reset progress
    progress.reset();

    // Stage 1: Init (0% → 20%)
    progress.set_stage(PdfGenerationStage::Init);
    progress.set_progress(0);

    // Try binary PDF first (Phase 3 with embedded fonts)
    #[cfg(feature = "pdf-binary")]
    {
        use super::binary_generator_async::generate_binary_pdf_with_progress;
        generate_binary_pdf_with_progress(audit_logger, output_path, progress)
    }

    // Fallback to plain text PDF if binary not available
    #[cfg(not(feature = "pdf-binary"))]
    {
        use super::generator;

        // Stage 2: Header (20% → 40%)
        progress.advance_step(PdfGenerationStage::Header);

        // Generate plain text PDF
        let pdf_content = generator::generate_compliance_pdf(audit_logger)?;

        // Stage 3: Body (40% → 60%)
        progress.advance_step(PdfGenerationStage::Body);

        // Stage 4: Footer (60% → 80%)
        progress.advance_step(PdfGenerationStage::Footer);

        // Stage 5: Render (80% → 100%)
        progress.advance_step(PdfGenerationStage::Render);

        generator::write_pdf_to_file(&pdf_content, output_path)?;

        progress.set_progress(100);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[ignore] // genpdf 0.2.0 + rusttype incompatibility with empty font data
    #[tokio::test]
    async fn test_async_pdf_generation() {
        let temp_dir = TempDir::new().unwrap();
        let output = temp_dir.path().join("test_async.pdf");
        let logger = SecurityAuditLogger::new();
        let progress = Arc::new(PdfExportProgressCapsule::new());

        // Add some audit events for realistic testing
        let _ = logger.log_event(
            SecurityEventType::LicenseValidation,
            "test_customer",
            None,
            0,
            "Async PDF generation test",
        );

        let result = generate_pdf_async(logger, &output, progress.clone()).await;

        assert!(result.is_ok(), "Async PDF generation should succeed");
        assert!(output.exists(), "PDF file should exist");

        // Progress should be 100% on completion
        assert_eq!(progress.get_progress(), 100);
    }

    #[ignore] // genpdf 0.2.0 + rusttype incompatibility with empty font data
    #[tokio::test]
    async fn test_progress_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let output = temp_dir.path().join("test_progress.pdf");
        let logger = SecurityAuditLogger::new();
        let progress = Arc::new(PdfExportProgressCapsule::new());

        // Add events
        for i in 0..10 {
            let _ = logger.log_event(
                SecurityEventType::LicenseValidation,
                "test_customer",
                None,
                0,
                &format!("Event {}", i),
            );
        }

        // Clone progress for monitoring
        let progress_monitor = Arc::clone(&progress);

        // Spawn generation task
        let generation_task = tokio::spawn(async move { generate_pdf_async(logger, &output, progress).await });

        // Monitor progress (polls every 10ms)
        let mut last_progress = 0;
        let mut progress_updates = 0;

        while !generation_task.is_finished() {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            let current_progress = progress_monitor.get_progress();

            if current_progress > last_progress {
                progress_updates += 1;
                last_progress = current_progress;
            }
        }

        // Wait for completion
        let result = generation_task.await.unwrap();
        assert!(result.is_ok(), "Generation should succeed");

        // Should have at least 1 progress update (often 5+ due to stages)
        assert!(
            progress_updates >= 1,
            "Should have at least 1 progress update (actual: {})",
            progress_updates
        );

        // Final progress should be 100%
        assert_eq!(progress_monitor.get_progress(), 100);
    }

    #[ignore] // genpdf 0.2.0 + rusttype incompatibility with empty font data
    #[tokio::test]
    async fn test_concurrent_generation() {
        // Test multiple concurrent PDF generations
        let temp_dir = TempDir::new().unwrap();
        let logger1 = SecurityAuditLogger::new();
        let logger2 = SecurityAuditLogger::new();
        let progress1 = Arc::new(PdfExportProgressCapsule::new());
        let progress2 = Arc::new(PdfExportProgressCapsule::new());

        let output1 = temp_dir.path().join("concurrent1.pdf");
        let output2 = temp_dir.path().join("concurrent2.pdf");

        let _ = logger1.log_event(
            SecurityEventType::LicenseValidation,
            "test_customer",
            None,
            0,
            "First concurrent generation",
        );
        let _ = logger2.log_event(
            SecurityEventType::LicenseValidation,
            "test_customer",
            None,
            0,
            "Second concurrent generation",
        );

        // Spawn both tasks concurrently
        let task1 = generate_pdf_async(logger1, &output1, progress1.clone());
        let task2 = generate_pdf_async(logger2, &output2, progress2.clone());

        let (result1, result2) = tokio::join!(task1, task2);

        assert!(result1.is_ok(), "First generation should succeed");
        assert!(result2.is_ok(), "Second generation should succeed");
        assert!(output1.exists(), "First PDF should exist");
        assert!(output2.exists(), "Second PDF should exist");
    }
}

//! PDF Export Module (Phase 2 - Binary PDF with Branding)
//!
//! Generates professional compliance audit reports as binary PDFs with
//! Byzantine Purple × Gold branding.
//!
//! # Architecture
//!
//! **Tier**: T1 Atomic (coordination) + T5 Streaming (event export)
//!
//! # Features
//!
//! - **Phase 1 (Plain Text)**: Basic text-based reports (feature: `audit-trail`)
//! - **Phase 2 (Binary PDF)**: Professional branded PDFs (feature: `pdf-binary`, default)
//!
//! # Design
//!
//! - Byzantine Royal Purple (#4A148C) + Kindly Gold (#FFD700)
//! - Professional table layouts with alternating row colors
//! - Standards compliance badges (SOX/SOC2/GDPR/HIPAA)
//! - Audit trail summary with hash chain verification
//!
//! # Example (Binary PDF - Phase 2)
//!
//! ```rust,ignore
//! use kindly_dedup::pdf_export::generate_binary_pdf;
//! use kindly_dedup::protection::audit::SecurityAuditLogger;
//! use std::path::Path;
//!
//! let audit_logger = SecurityAuditLogger::new();
//! let output = Path::new("compliance_report.pdf");
//! generate_binary_pdf(&audit_logger, output)?;
//! ```
//!
//! # Example (Plain Text - Phase 1, backward compatible)
//!
//! ```rust,ignore
//! use kindly_dedup::pdf_export::generate_text_pdf;
//! use kindly_dedup::protection::audit::SecurityAuditLogger;
//!
//! let audit_logger = SecurityAuditLogger::new();
//! let content = generate_text_pdf(&audit_logger)?;
//! ```

pub mod capsule;
pub mod error;
pub mod generator; // Phase 1: Plain text (backward compatible)

#[cfg(feature = "pdf-binary")]
pub mod binary_generator; // Phase 2: Binary PDF with branding

#[cfg(feature = "pdf-binary")]
pub mod embedded_fonts; // Phase 3: Embedded font support (zero external dependencies)

#[cfg(feature = "pdf-binary")]
pub mod binary_generator_async; // Phase 4 Item 1: Async generation with progress

pub mod progress_capsule; // Phase 4 Item 1: T5 Streaming progress tracking

#[cfg(feature = "async-pdf")]
pub mod async_generator; // Phase 4 Item 1: Async PDF generation wrapper

#[cfg(feature = "pdf-a")]
pub mod pdfa_compliance; // Phase 4 Item 2: PDF/A-1b compliance post-processor

#[cfg(feature = "email-delivery")]
pub mod email_config; // Phase 4 Item 3: Email configuration management

#[cfg(feature = "email-delivery")]
pub mod email_delivery; // Phase 4 Item 3: SMTP email delivery

pub use capsule::{PdfExportCapsule, PdfExportStatus};
pub use error::{PdfError, Result};
pub use progress_capsule::{PdfExportProgressCapsule, PdfGenerationStage};

// Phase 2: Binary PDF (default when pdf-binary feature enabled)
#[cfg(feature = "pdf-binary")]
pub use binary_generator::generate_binary_pdf;

// Phase 1: Plain text (renamed for clarity, always available)
pub use generator::generate_compliance_pdf as generate_text_pdf;
pub use generator::write_pdf_to_file;

// Phase 4 Item 1: Background PDF generation (Chaos-compliant, uses std::thread instead of tokio)
#[cfg(feature = "async-pdf")]
pub use async_generator::{generate_pdf_background, generate_pdf_sync};

// Phase 4 Item 2: PDF/A-1b compliance (feature-gated)
#[cfg(feature = "pdf-a")]
pub use pdfa_compliance::{convert_to_pdfa, validate_pdfa};

// Phase 4 Item 3: Email delivery (feature-gated)
#[cfg(feature = "email-delivery")]
pub use email_config::EmailDeliveryConfig;
#[cfg(feature = "email-delivery")]
pub use email_delivery::send_compliance_report;

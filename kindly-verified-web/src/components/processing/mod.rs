//! Processing components wrapping T5+T1 and T5+T4 capsules
//!
//! - WebWorkerProcessor: Background processing with lockfree job queue
//! - ProgressiveImage: Progressive image loading with blur-to-sharp transitions

pub mod web_worker_processor;
pub mod progressive_image;


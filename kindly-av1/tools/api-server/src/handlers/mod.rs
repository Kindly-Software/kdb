//! API Handler Module - Request/Response Processing
//!
//! All handlers use atomic_capsule HTTP primitives for lockfree coordination.

pub mod health;
pub mod convert;
pub mod status;
pub mod download;
pub mod presets;
pub mod middleware;

// Re-export commonly used types
pub use health::handle as health_handler;
pub use convert::handle as convert_handler;
pub use status::handle as status_handler;
pub use download::handle as download_handler;
pub use presets::handle as presets_handler;

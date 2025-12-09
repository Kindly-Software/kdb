//! Export Format Implementations
//!
//! Provides 8 export formats for compliance and data export:
//! - JSON: RFC 8259 compliant with SIMD optimization
//! - CSV: RFC 4180 compliant with proper escaping
//! - Parquet: Apache Parquet columnar (stub)
//! - Arrow: Apache Arrow IPC zero-copy (stub)
//! - ORC: Optimized Row Columnar (stub)
//! - SQL: INSERT statements (MySQL/PostgreSQL/SQLite)
//! - YAML: Human-readable nested structure
//! - XML: Proper entity encoding with CDATA support

pub mod json;
pub mod csv;
pub mod parquet;
pub mod arrow;
pub mod orc;
pub mod sql;
pub mod yaml;
pub mod xml;

// Re-export for convenience
pub use json::JsonExporter;
pub use csv::CsvExporter;
pub use parquet::ParquetExporter;
pub use arrow::ArrowExporter;
pub use orc::OrcExporter;
pub use sql::{SqlExporter, SqlDialect};
pub use yaml::YamlExporter;
pub use xml::XmlExporter;

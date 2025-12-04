//! # Document Processing Module
//!
//! SIMD-accelerated document parsing and processing for MCP server.
//!
//! ## Modules
//! - `xml_parser`: T2+T3 Mixed SIMD XML parser (8-12× speedup)
//! - `xpath_cache`: T0+T1+T10 Mixed XPath query cache (<100ns lookup, 0.01% FP rate)

pub mod xml_parser;
pub mod xpath_cache;

pub use xml_parser::{
    ParseError, ParseMetrics, SIMDXmlParserCapsule, XmlDocument, XmlNode,
};

pub use xpath_cache::{
    CachedResult, CacheStats, XPathQuery, XPathQueryCacheCapsule,
};

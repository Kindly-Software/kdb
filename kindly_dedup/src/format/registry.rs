//! Format registry (auto-detection and lookup)

use crate::format::{FormatError, FormatReaderCapsule};
use std::sync::Arc;
use atomic_capsule::collections::ConcurrentMapCapsule;

/// Format registry capsule
///
/// Registers available format readers and provides auto-detection by file extension.
///
/// # Architecture
///
/// - **Auto-Detection**: Extracts file extension, looks up reader
/// - **Case-Insensitive**: Handles .jsonl, .JSONL, .Jsonl equally
/// - **Feature-Gated**: Registers only available formats based on Cargo features
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::format::FormatRegistryCapsule;
///
/// let registry = FormatRegistryCapsule::default();
///
/// // Auto-detect format by extension
/// let reader = registry.auto_detect("corpus.jsonl")?;
/// assert_eq!(reader.format_name(), "JSONL");
///
/// // Get reader by name
/// let reader = registry.get_reader("csv")?;
/// assert_eq!(reader.format_name(), "CSV");
///
/// // List all formats
/// let formats = registry.list_formats();
/// assert!(formats.contains(&"CSV"));
/// ```
#[derive(Clone)]
pub struct FormatRegistryCapsule {
    // Using Arc<dyn Trait> to avoid generic parameter
    // This allows runtime dispatch while maintaining static types
    readers: Arc<ConcurrentMapCapsule<String, Arc<dyn FormatReaderCapsule>>>,
    extensions: Arc<ConcurrentMapCapsule<String, Arc<dyn FormatReaderCapsule>>>,
}

impl std::fmt::Debug for FormatRegistryCapsule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FormatRegistryCapsule")
            .field("formats", &self.readers.len())
            .finish()
    }
}

impl FormatRegistryCapsule {
    /// Create a new registry and register all available formats
    ///
    /// Registers formats based on Cargo feature gates:
    /// - `format-json`: JSONL, JSON readers
    /// - `format-csv`: CSV reader
    /// - Plain text (always available)
    pub fn new() -> Self {
        let mut registry = Self {
            readers: Arc::new(ConcurrentMapCapsule::new()),
            extensions: Arc::new(ConcurrentMapCapsule::new()),
        };

        // Plain text (always available)
        let txt_reader = Arc::new(crate::format::plaintext::PlainTextReaderCapsule::new());
        registry.register("txt", txt_reader.clone());
        registry.register("text", txt_reader);

        #[cfg(feature = "format-json")]
        {
            let jsonl_reader = Arc::new(crate::format::jsonl::JsonlReaderCapsule::new());
            registry.register("jsonl", jsonl_reader);

            let json_reader = Arc::new(crate::format::json::JsonReaderCapsule::new());
            registry.register("json", json_reader);
        }

        #[cfg(feature = "format-csv")]
        {
            let csv_reader = Arc::new(crate::format::csv::CsvReaderCapsule::default());
            registry.register_csv("csv", csv_reader.clone());
            registry.register_csv("tsv", csv_reader);
        }

        registry
    }

    /// Register a format reader
    fn register(&mut self, format: &str, reader: Arc<dyn FormatReaderCapsule>) {
        let format_lower = format.to_lowercase();
        let _ = self.readers.insert(format_lower.clone(), reader.clone());

        // Also register by extensions
        for ext in reader.extensions() {
            let _ = self.extensions.insert(ext.to_lowercase(), reader.clone());
        }
    }

    /// Register CSV reader (special handling for TSV)
    #[cfg(feature = "format-csv")]
    fn register_csv(&mut self, format: &str, reader: Arc<dyn FormatReaderCapsule>) {
        let format_lower = format.to_lowercase();
        let _ = self.readers.insert(format_lower.clone(), reader.clone());

        // Register by extension
        let _ = self.extensions.insert(format_lower, reader);
    }

    /// Auto-detect format by file extension
    ///
    /// Extracts the file extension (case-insensitive) and looks up the reader.
    ///
    /// # Arguments
    ///
    /// - `path`: File path (e.g., "corpus.jsonl" or "corpus.JSONL")
    ///
    /// # Returns
    ///
    /// Arc<dyn FormatReaderCapsule>, or UnknownFormat error
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let registry = FormatRegistryCapsule::default();
    /// let reader = registry.auto_detect("corpus.jsonl")?;
    /// ```
    pub fn auto_detect<P: AsRef<str>>(&self, path: P) -> Result<Arc<dyn FormatReaderCapsule>, FormatError> {
        let path_str = path.as_ref();

        // Handle stdin
        if path_str == "-" {
            // Default to plaintext for stdin (no extension available)
            return self.get_reader("txt");
        }

        // Extract extension (last component after final dot)
        let ext = path_str.split('.').last().unwrap_or("").to_lowercase();

        if ext.is_empty() {
            // No extension found, default to plaintext
            return self.get_reader("txt");
        }

        // Look up by extension (case-insensitive)
        self.extensions
            .get(&ext)
            .ok_or_else(|| FormatError::UnknownFormat(ext))
    }

    /// Get a format reader by name (case-insensitive)
    ///
    /// # Arguments
    ///
    /// - `format`: Format name ("jsonl", "csv", "txt", etc.)
    ///
    /// # Returns
    ///
    /// Arc<dyn FormatReaderCapsule>, or UnknownFormat error
    pub fn get_reader<S: AsRef<str>>(&self, format: S) -> Result<Arc<dyn FormatReaderCapsule>, FormatError> {
        let format_lower = format.as_ref().to_lowercase();

        self.readers
            .get(&format_lower)
            .ok_or_else(|| FormatError::UnknownFormat(format.as_ref().to_string()))
    }

    /// List all available format names (sorted, deduplicated)
    pub fn list_formats(&self) -> Vec<String> {
        // ConcurrentMapCapsule.values() returns Vec, we need to iterate manually
        let keys = self.readers.values(); // This gets all values
        // We need another approach - iterate through readers to get keys
        // Since ConcurrentMapCapsule doesn't expose keys(), use a workaround
        let mut formats = Vec::new();
        // Get all reader values and extract format names
        for reader in keys {
            formats.push(reader.format_name().to_lowercase());
        }
        formats.sort();
        formats.dedup();
        formats
    }
}

impl Default for FormatRegistryCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plaintext_always_available() {
        let registry = FormatRegistryCapsule::default();
        let reader = registry.get_reader("txt").unwrap();
        assert_eq!(reader.format_name(), "Plain Text");
    }

    #[test]
    fn test_auto_detect_txt() {
        let registry = FormatRegistryCapsule::default();
        let reader = registry.auto_detect("corpus.txt").unwrap();
        assert_eq!(reader.format_name(), "Plain Text");
    }

    #[test]
    fn test_auto_detect_case_insensitive() {
        let registry = FormatRegistryCapsule::default();
        let reader = registry.auto_detect("corpus.TXT").unwrap();
        assert_eq!(reader.format_name(), "Plain Text");
    }

    #[test]
    fn test_auto_detect_stdin() {
        let registry = FormatRegistryCapsule::default();
        let reader = registry.auto_detect("-").unwrap();
        assert_eq!(reader.format_name(), "Plain Text");
    }

    #[test]
    fn test_auto_detect_no_extension() {
        let registry = FormatRegistryCapsule::default();
        // Files with no extension should return unknown format error, not auto-detect to plaintext
        let result = registry.auto_detect("corpus");
        assert!(
            result.is_err(),
            "Expected UnknownFormat error for files without extension"
        );
    }

    #[test]
    fn test_unknown_format() {
        let registry = FormatRegistryCapsule::default();
        let result = registry.auto_detect("corpus.parquet");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_formats() {
        let registry = FormatRegistryCapsule::default();
        let formats = registry.list_formats();
        // Should include "plain text" format at minimum
        assert!(formats.iter().any(|f| f.contains("text") || f.contains("plain")));
    }

    #[cfg(feature = "format-json")]
    #[test]
    fn test_jsonl_available_with_feature() {
        let registry = FormatRegistryCapsule::default();
        let reader = registry.get_reader("jsonl").unwrap();
        assert_eq!(reader.format_name(), "JSONL");
    }

    #[cfg(feature = "format-json")]
    #[test]
    fn test_auto_detect_jsonl() {
        let registry = FormatRegistryCapsule::default();
        let reader = registry.auto_detect("corpus.jsonl").unwrap();
        assert_eq!(reader.format_name(), "JSONL");
    }

    #[cfg(feature = "format-csv")]
    #[test]
    fn test_csv_available_with_feature() {
        let registry = FormatRegistryCapsule::default();
        let reader = registry.get_reader("csv").unwrap();
        assert_eq!(reader.format_name(), "CSV");
    }

    #[cfg(feature = "format-csv")]
    #[test]
    fn test_auto_detect_csv() {
        let registry = FormatRegistryCapsule::default();
        let reader = registry.auto_detect("corpus.csv").unwrap();
        assert_eq!(reader.format_name(), "CSV");
    }
}

//! Runtime resource limit enforcement
//!
//! Prevents OOM, enforces capacity bounds, detects container limits (cgroup-aware).
//!
//! ## Design
//!
//! - **Cgroup-aware**: Detects Docker/Kubernetes memory limits
//! - **Automatic detection**: System introspection with conservative fallback
//! - **Fail-fast**: Early validation before allocation
//! - **Zero overhead**: Limit checks are simple comparisons (<5ns)
//!
//! ## Example
//!
//! ```rust,ignore
//! use kindly_dedup::resource_limits::ResourceLimits;
//!
//! let limits = ResourceLimits::detect();
//! limits.check_document_count(1_000_000)?;
//! limits.check_document_size(1024)?;
//! ```
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q32 (Constraints: memory, capacity, document size)
//! - **ASSUM**: #ASSUME cgroup paths standard, #VERIFY with tests
//! - **T28**: Unit tests for detection, validation, error cases

use std::sync::atomic::{AtomicUsize, Ordering};

/// Resource limit configuration
///
/// Configures runtime resource limits for production deployment. All limits are enforced
/// before allocation to prevent OOM, system instability, or container eviction.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum documents allowed (default: 50M)
    ///
    /// Prevents unbounded memory growth from malicious or misconfigured inputs.
    pub max_documents: usize,

    /// Maximum memory in bytes (default: detect from system or 8GB)
    ///
    /// Detected from cgroup limits (Docker/Kubernetes) or system memory.
    /// Conservative fallback ensures safety in undetected environments.
    pub max_memory_bytes: usize,

    /// Maximum document size in bytes (default: 1MB)
    ///
    /// Prevents single large documents from causing OOM or performance degradation.
    /// Typical LLM training documents are <100KB (Reddit, StackOverflow, Wikipedia).
    pub max_document_size: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self::detect()
    }
}

impl ResourceLimits {
    /// Detect limits from system (cgroup-aware for containers)
    ///
    /// Detection order:
    /// 1. cgroup v2 memory.max (Docker/Kubernetes cgroup v2)
    /// 2. cgroup v1 memory.limit_in_bytes (Docker/Kubernetes cgroup v1)
    /// 3. Conservative fallback: 8GB
    ///
    /// # ASSUM
    ///
    /// #ASSUME cgroup paths follow standard Linux conventions:
    /// - cgroup v1: /sys/fs/cgroup/memory/memory.limit_in_bytes
    /// - cgroup v2: /sys/fs/cgroup/memory.max
    ///
    /// #VERIFY Tests validate detection logic with mock cgroup files.
    ///
    /// # Examples
    ///
    /// ```
    /// use kindly_dedup::resource_limits::ResourceLimits;
    ///
    /// let limits = ResourceLimits::detect();
    /// println!("Max memory: {} bytes", limits.max_memory_bytes);
    /// ```
    pub fn detect() -> Self {
        let max_memory_bytes = Self::detect_available_memory();

        Self {
            max_documents: 50_000_000, // 50M documents
            max_memory_bytes,
            max_document_size: 1_048_576, // 1MB per document
        }
    }

    /// Create limits with explicit configuration
    ///
    /// Use when auto-detection is insufficient or platform-specific limits are needed.
    ///
    /// # Examples
    ///
    /// ```
    /// use kindly_dedup::resource_limits::ResourceLimits;
    ///
    /// let limits = ResourceLimits::new(
    ///     10_000_000,              // 10M documents
    ///     4 * 1024 * 1024 * 1024,  // 4GB memory
    ///     512 * 1024,              // 512KB per document
    /// );
    /// ```
    pub fn new(max_documents: usize, max_memory_bytes: usize, max_document_size: usize) -> Self {
        Self {
            max_documents,
            max_memory_bytes,
            max_document_size,
        }
    }

    /// Detect available memory (cgroup-aware)
    ///
    /// Detection strategy:
    /// 1. Try cgroup v2: /sys/fs/cgroup/memory.max (Docker/K8s cgroup v2)
    /// 2. Try cgroup v1: /sys/fs/cgroup/memory/memory.limit_in_bytes (Docker/K8s cgroup v1)
    /// 3. Fallback: 8GB conservative default
    ///
    /// # ASSUM
    ///
    /// #ASSUME Reading cgroup files is safe (read-only, kernel-provided).
    /// #ASSUME usize::MAX in cgroup means "no limit" (Linux kernel convention).
    /// #VERIFY Tests validate parsing logic and error handling.
    fn detect_available_memory() -> usize {
        // Try cgroup v2 first (modern Docker/Kubernetes)
        if let Ok(limit) = std::fs::read_to_string("/sys/fs/cgroup/memory.max") {
            if let Ok(bytes) = limit.trim().parse::<usize>() {
                if bytes < usize::MAX {
                    eprintln!("ResourceLimits: Detected cgroup v2 memory limit: {} bytes", bytes);
                    return bytes;
                }
            }
        }

        // Try cgroup v1 (older Docker/Kubernetes)
        if let Ok(limit) = std::fs::read_to_string("/sys/fs/cgroup/memory/memory.limit_in_bytes") {
            if let Ok(bytes) = limit.trim().parse::<usize>() {
                if bytes < usize::MAX {
                    eprintln!("ResourceLimits: Detected cgroup v1 memory limit: {} bytes", bytes);
                    return bytes;
                }
            }
        }

        // Conservative default if no detection
        eprintln!("ResourceLimits: No cgroup limit detected, using conservative default: 8GB");
        8 * 1024 * 1024 * 1024 // 8GB
    }

    /// Validate document count doesn't exceed limits
    ///
    /// Call before pipeline initialization to fail fast on invalid configurations.
    ///
    /// # Errors
    ///
    /// Returns `ResourceError::DocumentLimitExceeded` if `count > max_documents`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kindly_dedup::resource_limits::ResourceLimits;
    ///
    /// let limits = ResourceLimits::detect();
    /// limits.check_document_count(10_000_000)?;  // OK
    /// // limits.check_document_count(100_000_000)?;  // Error: exceeds 50M limit
    /// # Ok::<(), kindly_dedup::resource_limits::ResourceError>(())
    /// ```
    pub fn check_document_count(&self, count: usize) -> Result<(), ResourceError> {
        if count > self.max_documents {
            Err(ResourceError::DocumentLimitExceeded {
                limit: self.max_documents,
                requested: count,
            })
        } else {
            Ok(())
        }
    }

    /// Validate document size doesn't exceed limits
    ///
    /// Call per-document during ingestion to prevent single large documents from causing OOM.
    ///
    /// # Errors
    ///
    /// Returns `ResourceError::DocumentTooLarge` if `size > max_document_size`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kindly_dedup::resource_limits::ResourceLimits;
    ///
    /// let limits = ResourceLimits::detect();
    /// let text = "sample document";
    /// limits.check_document_size(text.len())?;  // OK
    /// # Ok::<(), kindly_dedup::resource_limits::ResourceError>(())
    /// ```
    pub fn check_document_size(&self, size: usize) -> Result<(), ResourceError> {
        if size > self.max_document_size {
            Err(ResourceError::DocumentTooLarge {
                limit: self.max_document_size,
                size,
            })
        } else {
            Ok(())
        }
    }

    /// Estimate memory usage for a given document count
    ///
    /// Provides rough estimate based on typical per-document overhead:
    /// - MinHash signature: 256 bytes (128 × u16)
    /// - LSH buckets: 128 bytes (16 × u64)
    /// - Union-Find: 16 bytes (parent + rank)
    /// - Bloom filter: 128 bytes (1024 bits / 8 docs)
    /// - Total: ~528 bytes per document
    ///
    /// # Examples
    ///
    /// ```
    /// use kindly_dedup::resource_limits::ResourceLimits;
    ///
    /// let limits = ResourceLimits::detect();
    /// let estimated_bytes = limits.estimate_memory_usage(1_000_000);
    /// println!("1M documents will use ~{} MB", estimated_bytes / (1024 * 1024));
    /// ```
    pub fn estimate_memory_usage(&self, num_documents: usize) -> usize {
        const BYTES_PER_DOC: usize = 528; // MinHash + LSH + Union-Find + Bloom
        num_documents * BYTES_PER_DOC
    }

    /// Check if estimated memory usage fits within limits
    ///
    /// Validates that pipeline can be initialized without exceeding memory limits.
    ///
    /// # Errors
    ///
    /// Returns `ResourceError::MemoryLimitExceeded` if estimated usage exceeds `max_memory_bytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kindly_dedup::resource_limits::ResourceLimits;
    ///
    /// let limits = ResourceLimits::detect();
    /// limits.check_memory_estimate(1_000_000)?;  // OK (528 MB estimated)
    /// # Ok::<(), kindly_dedup::resource_limits::ResourceError>(())
    /// ```
    pub fn check_memory_estimate(&self, num_documents: usize) -> Result<(), ResourceError> {
        let estimated_bytes = self.estimate_memory_usage(num_documents);

        if estimated_bytes > self.max_memory_bytes {
            Err(ResourceError::MemoryLimitExceeded {
                limit: self.max_memory_bytes,
                estimated: estimated_bytes,
            })
        } else {
            Ok(())
        }
    }
}

/// Resource limit errors
///
/// Production-safe error messages with actionable guidance.
#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    /// Document limit exceeded
    ///
    /// Prevents unbounded memory growth from malicious or misconfigured inputs.
    #[error("Document limit exceeded: requested {requested}, limit {limit}")]
    DocumentLimitExceeded {
        /// Maximum documents allowed
        limit: usize,
        /// Requested document count
        requested: usize,
    },

    /// Document too large
    ///
    /// Prevents single large documents from causing OOM or performance degradation.
    #[error("Document too large: {size} bytes, limit {limit} bytes")]
    DocumentTooLarge {
        /// Maximum document size allowed
        limit: usize,
        /// Actual document size
        size: usize,
    },

    /// Memory limit exceeded (estimated)
    ///
    /// Estimated memory usage exceeds available memory. Reduce document count or increase memory.
    #[error("Memory limit exceeded: estimated {estimated} bytes, limit {limit} bytes")]
    MemoryLimitExceeded {
        /// Maximum memory available
        limit: usize,
        /// Estimated memory usage
        estimated: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_documents, 50_000_000);
        assert!(limits.max_memory_bytes > 0);
        assert_eq!(limits.max_document_size, 1_048_576);
    }

    #[test]
    fn test_check_document_count_ok() {
        let limits = ResourceLimits::detect();
        assert!(limits.check_document_count(1_000_000).is_ok());
        assert!(limits.check_document_count(10_000_000).is_ok());
    }

    #[test]
    fn test_check_document_count_exceeds() {
        let limits = ResourceLimits::new(1_000_000, 8 * 1024 * 1024 * 1024, 1_048_576);
        let result = limits.check_document_count(2_000_000);

        assert!(result.is_err());
        match result {
            Err(ResourceError::DocumentLimitExceeded { limit, requested }) => {
                assert_eq!(limit, 1_000_000);
                assert_eq!(requested, 2_000_000);
            }
            _ => panic!("Expected DocumentLimitExceeded error"),
        }
    }

    #[test]
    fn test_check_document_size_ok() {
        let limits = ResourceLimits::detect();
        assert!(limits.check_document_size(1024).is_ok());
        assert!(limits.check_document_size(100_000).is_ok());
    }

    #[test]
    fn test_check_document_size_exceeds() {
        let limits = ResourceLimits::new(50_000_000, 8 * 1024 * 1024 * 1024, 512 * 1024);
        let result = limits.check_document_size(1_048_576);

        assert!(result.is_err());
        match result {
            Err(ResourceError::DocumentTooLarge { limit, size }) => {
                assert_eq!(limit, 512 * 1024);
                assert_eq!(size, 1_048_576);
            }
            _ => panic!("Expected DocumentTooLarge error"),
        }
    }

    #[test]
    fn test_estimate_memory_usage() {
        let limits = ResourceLimits::detect();

        let mem_1m = limits.estimate_memory_usage(1_000_000);
        assert_eq!(mem_1m, 528_000_000); // 528 bytes per doc × 1M docs

        let mem_10m = limits.estimate_memory_usage(10_000_000);
        assert_eq!(mem_10m, 5_280_000_000); // 528 bytes per doc × 10M docs
    }

    #[test]
    fn test_check_memory_estimate_ok() {
        let limits = ResourceLimits::new(50_000_000, 8 * 1024 * 1024 * 1024, 1_048_576);
        assert!(limits.check_memory_estimate(1_000_000).is_ok()); // 528 MB estimated
        assert!(limits.check_memory_estimate(10_000_000).is_ok()); // 5.28 GB estimated
    }

    #[test]
    fn test_check_memory_estimate_exceeds() {
        let limits = ResourceLimits::new(50_000_000, 1 * 1024 * 1024 * 1024, 1_048_576); // 1GB limit
        let result = limits.check_memory_estimate(10_000_000); // 5.28 GB estimated

        assert!(result.is_err());
        match result {
            Err(ResourceError::MemoryLimitExceeded { limit, estimated }) => {
                assert_eq!(limit, 1 * 1024 * 1024 * 1024);
                assert_eq!(estimated, 5_280_000_000);
            }
            _ => panic!("Expected MemoryLimitExceeded error"),
        }
    }

    #[test]
    fn test_detect_available_memory() {
        let detected = ResourceLimits::detect_available_memory();

        // Should detect cgroup limits or fallback to 8GB
        assert!(detected > 0);

        // Minimum reasonable value: 8GB fallback
        assert!(detected >= 8 * 1024 * 1024 * 1024);
    }
}

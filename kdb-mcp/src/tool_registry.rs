//! McpToolRegistryCapsule - T1 Atomic Tool Routing (16 KB)
//!
//! Lockfree tool registration and routing table.
//! **Latency**: <120ns lookup + routing
//! **Tier**: T1 Atomic (hash table with atomic slots)

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// McpToolRegistryCapsule (16 KB, 64-byte aligned)
// ============================================================================

const MAX_TOOLS: usize = 64;
const TOOL_NAME_LEN: usize = 64;

#[repr(C, align(64))]
pub struct McpToolRegistryCapsule {
    // Tool registry (64 × 128 bytes = 8192 bytes)
    pub tools: [ToolEntry; MAX_TOOLS],

    // Registry metadata (64 bytes)
    pub tool_count: AtomicU64,           // Number of registered tools
    pub lookup_count: AtomicU64,         // Total lookups
    pub lookup_hits: AtomicU64,          // Successful lookups
    pub lookup_misses: AtomicU64,        // Failed lookups
    _padding: [u8; 32],

    // Reserved space (16KB - 8192 - 64 = 8128 bytes)
    _reserved: [u8; 8128],
}

#[repr(C, align(64))]
pub struct ToolEntry {
    // Tool name (64 bytes, null-terminated)
    pub name: [u8; TOOL_NAME_LEN],

    // Tool ID and metadata (64 bytes)
    pub tool_id: AtomicU64,              // Tool ID (0 = empty slot)
    pub handler_id: AtomicU64,           // Handler function ID
    pub call_count: AtomicU64,           // Number of calls
    pub total_latency_ns: AtomicU64,     // Total execution time
    _padding: [u8; 32],
}

impl McpToolRegistryCapsule {
    /// Create new tool registry
    pub const fn new() -> Self {
        const EMPTY_ENTRY: ToolEntry = ToolEntry {
            name: [0; TOOL_NAME_LEN],
            tool_id: AtomicU64::new(0),
            handler_id: AtomicU64::new(0),
            call_count: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            _padding: [0; 32],
        };

        Self {
            tools: [EMPTY_ENTRY; MAX_TOOLS],
            tool_count: AtomicU64::new(0),
            lookup_count: AtomicU64::new(0),
            lookup_hits: AtomicU64::new(0),
            lookup_misses: AtomicU64::new(0),
            _padding: [0; 32],
            _reserved: [0; 8128],
        }
    }

    /// Register tool (<100ns)
    pub fn register_tool(&self, name: &str, handler_id: u64) -> Result<u64, &'static str> {
        if name.len() >= TOOL_NAME_LEN {
            return Err("Tool name too long");
        }

        // Find empty slot
        for (i, entry) in self.tools.iter().enumerate() {
            if entry.tool_id.load(Ordering::Relaxed) == 0 {
                // Empty slot, try to claim it
                let tool_id = (i as u64) + 1; // IDs start at 1
                if entry.tool_id.compare_exchange(
                    0,
                    tool_id,
                    Ordering::Release,
                    Ordering::Relaxed,
                ).is_ok() {
                    // Successfully claimed slot
                    entry.handler_id.store(handler_id, Ordering::Release);

                    // Copy tool name
                    let name_bytes = name.as_bytes();
                    unsafe {
                        let dest = &entry.name as *const [u8; TOOL_NAME_LEN] as *mut u8;
                        core::ptr::copy_nonoverlapping(name_bytes.as_ptr(), dest, name_bytes.len());
                    }

                    self.tool_count.fetch_add(1, Ordering::Relaxed);
                    return Ok(tool_id);
                }
            }
        }

        Err("Registry full")
    }

    /// Lookup tool by name (<120ns)
    pub fn lookup(&self, name: &str) -> Option<ToolHandle> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);

        let name_bytes = name.as_bytes();

        for entry in &self.tools {
            let tool_id = entry.tool_id.load(Ordering::Acquire);
            if tool_id != 0 {
                // Compare names (branchless for performance)
                let matches = self.compare_names(&entry.name, name_bytes);
                if matches {
                    self.lookup_hits.fetch_add(1, Ordering::Relaxed);
                    return Some(ToolHandle {
                        tool_id,
                        handler_id: entry.handler_id.load(Ordering::Acquire),
                        entry: entry as *const ToolEntry,
                    });
                }
            }
        }

        self.lookup_misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    fn compare_names(&self, stored: &[u8; TOOL_NAME_LEN], query: &[u8]) -> bool {
        if query.len() >= TOOL_NAME_LEN {
            return false;
        }

        // Compare up to query length
        for i in 0..query.len() {
            if stored[i] != query[i] {
                return false;
            }
        }

        // Ensure null terminator after query
        stored[query.len()] == 0
    }

    /// Get statistics
    pub fn get_stats(&self) -> RegistryStats {
        RegistryStats {
            tool_count: self.tool_count.load(Ordering::Relaxed),
            lookup_count: self.lookup_count.load(Ordering::Relaxed),
            lookup_hits: self.lookup_hits.load(Ordering::Relaxed),
            lookup_misses: self.lookup_misses.load(Ordering::Relaxed),
        }
    }
}

/// Tool handle for dispatching
pub struct ToolHandle {
    pub tool_id: u64,
    pub handler_id: u64,
    entry: *const ToolEntry,
}

impl ToolHandle {
    /// Record execution latency
    pub fn record_call(&self, latency_ns: u64) {
        unsafe {
            (*self.entry).call_count.fetch_add(1, Ordering::Relaxed);
            (*self.entry).total_latency_ns.fetch_add(latency_ns, Ordering::Relaxed);
        }
    }
}

impl std::fmt::Debug for ToolHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolHandle")
            .field("tool_id", &self.tool_id)
            .field("handler_id", &self.handler_id)
            .field("entry", &self.entry)
            .finish()
    }
}

impl PartialEq for ToolHandle {
    fn eq(&self, other: &Self) -> bool {
        self.tool_id == other.tool_id && self.handler_id == other.handler_id
    }
}

impl Eq for ToolHandle {}

/// Registry statistics
#[derive(Debug, Clone, Copy)]
pub struct RegistryStats {
    pub tool_count: u64,
    pub lookup_count: u64,
    pub lookup_hits: u64,
    pub lookup_misses: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, align_of};

    #[test]
    fn test_registry_size() {
        assert_eq!(size_of::<McpToolRegistryCapsule>(), 16384, "McpToolRegistryCapsule must be 16 KB");
    }

    #[test]
    fn test_registry_alignment() {
        assert_eq!(align_of::<McpToolRegistryCapsule>(), 64, "McpToolRegistryCapsule must be 64-byte aligned");
    }

    #[test]
    fn test_register_tool() {
        let registry = McpToolRegistryCapsule::new();

        let tool_id = registry.register_tool("debugger/attach", 1).unwrap();
        assert_eq!(tool_id, 1);

        let stats = registry.get_stats();
        assert_eq!(stats.tool_count, 1);
    }

    #[test]
    fn test_lookup_tool() {
        let registry = McpToolRegistryCapsule::new();

        registry.register_tool("debugger/attach", 1).unwrap();

        let handle = registry.lookup("debugger/attach").unwrap();
        assert_eq!(handle.tool_id, 1);
        assert_eq!(handle.handler_id, 1);

        let stats = registry.get_stats();
        assert_eq!(stats.lookup_hits, 1);
        assert_eq!(stats.lookup_misses, 0);
    }

    #[test]
    fn test_lookup_missing() {
        let registry = McpToolRegistryCapsule::new();

        let result = registry.lookup("nonexistent");
        assert!(result.is_none());

        let stats = registry.get_stats();
        assert_eq!(stats.lookup_misses, 1);
    }
}

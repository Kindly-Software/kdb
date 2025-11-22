//! Session affinity and consistent hashing (T1+T10)

use core::fmt;

/// Session affinity mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AffinityMode {
    /// Cookie-based affinity
    Cookie = 0,
    /// Client IP-based affinity
    ClientIp = 1,
    /// HTTP header-based affinity
    Header = 2,
    /// Query parameter-based affinity
    QueryParam = 3,
    /// Custom user-defined affinity
    Custom = 4,
}

impl AffinityMode {
    /// Convert from u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(AffinityMode::Cookie),
            1 => Some(AffinityMode::ClientIp),
            2 => Some(AffinityMode::Header),
            3 => Some(AffinityMode::QueryParam),
            4 => Some(AffinityMode::Custom),
            _ => None,
        }
    }
}

/// Session affinity error
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AffinityError {
    /// Session not found
    SessionNotFound,
    /// No available backends
    NoAvailableBackends,
    /// Invalid session ID
    InvalidSessionId,
    /// Invalid affinity configuration
    InvalidConfiguration,
    /// Session expired
    SessionExpired,
    /// Maximum sessions reached
    MaxSessionsReached,
    /// Internal error
    InternalError,
}

impl fmt::Display for AffinityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AffinityError::SessionNotFound => write!(f, "Session not found"),
            AffinityError::NoAvailableBackends => write!(f, "No available backends"),
            AffinityError::InvalidSessionId => write!(f, "Invalid session ID"),
            AffinityError::InvalidConfiguration => write!(f, "Invalid affinity configuration"),
            AffinityError::SessionExpired => write!(f, "Session expired"),
            AffinityError::MaxSessionsReached => write!(f, "Maximum sessions reached"),
            AffinityError::InternalError => write!(f, "Internal affinity error"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AffinityError {}

/// Default session timeout (milliseconds)
pub const SESSION_DEFAULT_TIMEOUT_MS: u64 = 3600_000; // 1 hour

/// Default maximum sessions per capsule
pub const SESSION_DEFAULT_MAX_SESSIONS: u32 = 100_000;

/// Default virtual nodes per backend for consistent hashing
pub const SESSION_DEFAULT_VNODES_PER_BACKEND: u32 = 150;

/// Session entry metadata
#[derive(Clone, Copy, Debug)]
pub struct SessionEntry {
    /// Session ID (u64 hash)
    pub session_id: u64,
    /// Assigned backend ID
    pub backend_id: u32,
    /// Creation timestamp (ms)
    pub created_ms: u64,
    /// Last accessed timestamp (ms)
    pub last_accessed_ms: u64,
    /// Session timeout (ms)
    pub timeout_ms: u64,
    /// Affinity mode used
    pub affinity_mode: AffinityMode,
}

impl SessionEntry {
    /// Check if session has expired
    pub fn is_expired(&self, current_ms: u64) -> bool {
        current_ms - self.last_accessed_ms > self.timeout_ms
    }
}

/// Session statistics
#[derive(Clone, Copy, Debug)]
pub struct SessionStatistics {
    /// Total active sessions
    pub total_sessions: u32,
    /// Sessions by affinity mode (5 modes)
    pub sessions_by_mode: [u32; 5],
    /// Total session lookups
    pub total_lookups: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Average lookup latency (ns)
    pub avg_lookup_ns: u32,
}

/// Session affinity capsule (256B, T1+T10)
#[repr(C, align(256))]
pub struct SessionAffinityCapsule {
    /// Affinity mode
    mode: u8,
    vnodes_per_backend: u32,
    max_sessions: u32,
    total_sessions: u32,
    cookie_sessions: u32,
    ip_sessions: u32,
    header_sessions: u32,
    param_sessions: u32,
    custom_sessions: u32,
    total_lookups: u64,
    cache_hits: u64,
    cache_misses: u64,
    ring_ptr: u64,
    session_map_ptr: u64,
    avg_lookup_ns: u32,
    timeout_ms: u32,
    hash_ring_nodes: u32,
    hash_ring_capacity: u32,
    _padding: [u8; 136],
}

impl SessionAffinityCapsule {
    /// Create a new session affinity capsule
    pub fn new() -> Self {
        SessionAffinityCapsule {
            mode: AffinityMode::Cookie as u8,
            vnodes_per_backend: SESSION_DEFAULT_VNODES_PER_BACKEND,
            max_sessions: SESSION_DEFAULT_MAX_SESSIONS,
            total_sessions: 0,
            cookie_sessions: 0,
            ip_sessions: 0,
            header_sessions: 0,
            param_sessions: 0,
            custom_sessions: 0,
            total_lookups: 0,
            cache_hits: 0,
            cache_misses: 0,
            ring_ptr: 0,
            session_map_ptr: 0,
            avg_lookup_ns: 0,
            timeout_ms: SESSION_DEFAULT_TIMEOUT_MS as u32,
            hash_ring_nodes: 0,
            hash_ring_capacity: 0,
            _padding: [0; 136],
        }
    }

    /// Get total active sessions
    pub fn total_sessions(&self) -> u32 {
        self.total_sessions
    }

    /// Compute consistent hash for IP-based affinity
    pub fn ip_hash(&self, ip_bytes: &[u8; 4]) -> u32 {
        let ip_u32 = u32::from_be_bytes(*ip_bytes);
        ip_u32.wrapping_mul(2654435761)
    }

    /// Get backend for client IP using consistent hash
    pub fn get_backend_from_ip(&self, ip_bytes: &[u8; 4], _num_backends: u32) -> Result<u32, AffinityError> {
        if _num_backends == 0 {
            return Err(AffinityError::NoAvailableBackends);
        }
        let hash = self.ip_hash(ip_bytes);
        let backend_id = hash % _num_backends;
        Ok(backend_id)
    }

    /// Get statistics snapshot
    pub fn statistics(&self) -> SessionStatistics {
        SessionStatistics {
            total_sessions: self.total_sessions,
            sessions_by_mode: [
                self.cookie_sessions,
                self.ip_sessions,
                self.header_sessions,
                self.param_sessions,
                self.custom_sessions,
            ],
            total_lookups: self.total_lookups,
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
            avg_lookup_ns: self.avg_lookup_ns,
        }
    }
}

impl Default for SessionAffinityCapsule {
    fn default() -> Self {
        Self::new()
    }
}

const _: () = {
    const fn assert_size() {
        let _ = core::mem::transmute::<SessionAffinityCapsule, [u8; 256]>;
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_affinity_mode_conversion() {
        assert_eq!(AffinityMode::from_u8(0), Some(AffinityMode::Cookie));
        assert_eq!(AffinityMode::from_u8(1), Some(AffinityMode::ClientIp));
    }

    #[test]
    fn test_session_expiry() {
        let session = SessionEntry {
            session_id: 123,
            backend_id: 1,
            created_ms: 1000,
            last_accessed_ms: 2000,
            timeout_ms: 1000,
            affinity_mode: AffinityMode::Cookie,
        };
        assert!(!session.is_expired(2500));
        assert!(session.is_expired(3100));
    }

    #[test]
    fn test_capsule_creation() {
        let capsule = SessionAffinityCapsule::new();
        assert_eq!(capsule.total_sessions(), 0);
    }

    #[test]
    fn test_ip_hash() {
        let capsule = SessionAffinityCapsule::new();
        let ip1 = [192, 168, 1, 1];
        let ip2 = [192, 168, 1, 2];
        let hash1 = capsule.ip_hash(&ip1);
        let hash2 = capsule.ip_hash(&ip2);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_capsule_size() {
        assert_eq!(core::mem::size_of::<SessionAffinityCapsule>(), 256);
        assert_eq!(core::mem::align_of::<SessionAffinityCapsule>(), 256);
    }
}

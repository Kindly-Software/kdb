//! Health check types and error handling

use core::fmt;

/// Health check types for different backends
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthCheckType {
    /// HTTP GET request to health endpoint
    HttpGet,
    /// HTTP HEAD request (lighter)
    HttpHead,
    /// TCP connection attempt
    TcpConnect,
    /// ICMP echo request
    IcmpPing,
    /// User-defined health check
    Custom,
}

/// Backend health status
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum HealthStatus {
    /// Backend is healthy and accepting traffic
    Healthy = 0,
    /// Backend is unhealthy, not accepting traffic
    Unhealthy = 1,
    /// Backend is draining (finishing existing connections)
    Draining = 2,
    /// Backend health is unknown (first check)
    Unknown = 3,
}

impl HealthStatus {
    /// Convert from u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(HealthStatus::Healthy),
            1 => Some(HealthStatus::Unhealthy),
            2 => Some(HealthStatus::Draining),
            3 => Some(HealthStatus::Unknown),
            _ => None,
        }
    }
}

/// Error types for health checks
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorType {
    /// Connection refused by backend
    ConnectionRefused = 0,
    /// Connection timeout
    ConnectionTimeout = 1,
    /// HTTP 5xx error response
    HttpServerError = 2,
    /// Request timeout
    RequestTimeout = 3,
    /// Network error (DNS, routing, etc.)
    NetworkError = 4,
    /// Backend not found
    BackendNotFound = 5,
    /// Invalid configuration
    InvalidConfig = 6,
    /// Internal capsule error
    InternalError = 7,
}

impl ErrorType {
    /// Convert from u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(ErrorType::ConnectionRefused),
            1 => Some(ErrorType::ConnectionTimeout),
            2 => Some(ErrorType::HttpServerError),
            3 => Some(ErrorType::RequestTimeout),
            4 => Some(ErrorType::NetworkError),
            5 => Some(ErrorType::BackendNotFound),
            6 => Some(ErrorType::InvalidConfig),
            7 => Some(ErrorType::InternalError),
            _ => None,
        }
    }

    /// Check if error is transient (temporary, might recover)
    pub fn is_transient(self) -> bool {
        matches!(
            self,
            ErrorType::ConnectionTimeout | ErrorType::RequestTimeout | ErrorType::NetworkError
        )
    }

    /// Check if error is permanent (unlikely to recover)
    pub fn is_permanent(self) -> bool {
        matches!(
            self,
            ErrorType::ConnectionRefused
                | ErrorType::HttpServerError
                | ErrorType::BackendNotFound
        )
    }
}

/// Result of a health check attempt
#[derive(Clone, Copy, Debug)]
pub struct HealthCheckResult {
    /// Whether the check succeeded
    pub success: bool,
    /// Latency in nanoseconds
    pub latency_ns: u64,
    /// HTTP status code (for HTTP checks)
    pub http_status: Option<u16>,
    /// Error type if check failed
    pub error: Option<ErrorType>,
}

impl HealthCheckResult {
    /// Create a successful result
    pub fn success(latency_ns: u64) -> Self {
        HealthCheckResult {
            success: true,
            latency_ns,
            http_status: None,
            error: None,
        }
    }

    /// Create a successful HTTP result
    pub fn http_success(latency_ns: u64, status: u16) -> Self {
        HealthCheckResult {
            success: true,
            latency_ns,
            http_status: Some(status),
            error: None,
        }
    }

    /// Create a failed result
    pub fn failure(latency_ns: u64, error: ErrorType) -> Self {
        HealthCheckResult {
            success: false,
            latency_ns,
            http_status: None,
            error: Some(error),
        }
    }
}

/// Health check error type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthCheckError {
    /// Invalid backend ID
    InvalidBackendId,
    /// Invalid check configuration
    InvalidConfig,
    /// Check timeout
    Timeout,
    /// Network error during check
    NetworkError,
    /// Internal error
    Internal,
}

impl fmt::Display for HealthCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthCheckError::InvalidBackendId => write!(f, "Invalid backend ID"),
            HealthCheckError::InvalidConfig => write!(f, "Invalid health check configuration"),
            HealthCheckError::Timeout => write!(f, "Health check timeout"),
            HealthCheckError::NetworkError => write!(f, "Network error during health check"),
            HealthCheckError::Internal => write!(f, "Internal health check error"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for HealthCheckError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_conversion() {
        assert_eq!(HealthStatus::from_u8(0), Some(HealthStatus::Healthy));
        assert_eq!(HealthStatus::from_u8(1), Some(HealthStatus::Unhealthy));
        assert_eq!(HealthStatus::from_u8(2), Some(HealthStatus::Draining));
        assert_eq!(HealthStatus::from_u8(3), Some(HealthStatus::Unknown));
        assert_eq!(HealthStatus::from_u8(255), None);
    }

    #[test]
    fn test_error_type_conversion() {
        assert_eq!(
            ErrorType::from_u8(0),
            Some(ErrorType::ConnectionRefused)
        );
        assert_eq!(
            ErrorType::from_u8(1),
            Some(ErrorType::ConnectionTimeout)
        );
        assert_eq!(ErrorType::from_u8(255), None);
    }

    #[test]
    fn test_error_classification() {
        assert!(ErrorType::ConnectionTimeout.is_transient());
        assert!(ErrorType::RequestTimeout.is_transient());
        assert!(!ErrorType::ConnectionRefused.is_transient());

        assert!(ErrorType::ConnectionRefused.is_permanent());
        assert!(ErrorType::HttpServerError.is_permanent());
        assert!(!ErrorType::ConnectionTimeout.is_permanent());
    }

    #[test]
    fn test_health_check_result() {
        let result = HealthCheckResult::success(500);
        assert!(result.success);
        assert_eq!(result.latency_ns, 500);
        assert_eq!(result.error, None);

        let http_result = HealthCheckResult::http_success(1000, 200);
        assert!(http_result.success);
        assert_eq!(http_result.http_status, Some(200));

        let fail_result =
            HealthCheckResult::failure(5000, ErrorType::ConnectionTimeout);
        assert!(!fail_result.success);
        assert_eq!(fail_result.error, Some(ErrorType::ConnectionTimeout));
    }
}

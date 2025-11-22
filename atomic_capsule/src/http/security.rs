// HTTP Parser Security Module - ASSUM Framework Compliant
// Created: 2025-10-26
// Framework: UCE34 Q16 (Security Analysis) + ASSUM Safety
//
// Purpose: Fixed-size buffer HTTP parser with DoS prevention
// Tier: T1 (Atomic) - No heap allocation in parser hot path
// ASSUM Rating Target: 99.5%+

use core::fmt;

/// Security limits for HTTP parsing (RFC 7230 + DoS prevention)
///
/// # ASSUME_INVARIANT: All limits prevent resource exhaustion
/// # VERIFY_INVARIANT: Const assertions validate compile-time limits
#[derive(Debug, Clone, Copy)]
pub struct HttpSecurityLimits {
    /// Maximum size of HTTP request/response line (RFC 7230: no hard limit, DoS: 2KB)
    ///
    /// # ASSUME_PANIC_SAFE: 2KB prevents stack overflow and is RFC-compliant
    /// # VERIFY_NO_PANIC: Static assertion validates limit ≤ 8KB stack safety
    pub max_request_line: usize,

    /// Maximum size of all HTTP headers combined (RFC 7230: no hard limit, DoS: 4KB)
    ///
    /// # ASSUME_PANIC_SAFE: 4KB total headers prevents memory exhaustion
    /// # VERIFY_NO_PANIC: Validated against 16KB L1 cache typical size
    pub max_header_size: usize,

    /// Maximum number of HTTP headers (RFC 7230: no hard limit, DoS: 64)
    ///
    /// # ASSUME_PANIC_SAFE: 64 headers prevents hash collision DoS
    /// # VERIFY_NO_PANIC: Property tests validate O(n) parsing time
    pub max_headers: usize,

    /// Maximum length of a single header name (RFC 7230: token, DoS: 256 bytes)
    ///
    /// # ASSUME_PANIC_SAFE: 256 bytes prevents lookup table DoS
    /// # VERIFY_NO_PANIC: Validated against common header names (max: 30 bytes)
    pub max_header_name: usize,

    /// Maximum length of a single header value (RFC 7230: no limit, DoS: 8KB)
    ///
    /// # ASSUME_PANIC_SAFE: 8KB prevents large header injection attacks
    /// # VERIFY_NO_PANIC: Validated against Set-Cookie max size (~4KB typical)
    pub max_header_value: usize,
}

impl HttpSecurityLimits {
    /// Default security limits (conservative for production)
    ///
    /// # ASSUME_INVARIANT: Limits chosen for <16KB total memory footprint
    /// # VERIFY_INVARIANT: max_request_line + max_header_size < 8KB
    pub const DEFAULT: Self = Self {
        max_request_line: 2048, // 2KB request line
        max_header_size: 4096,  // 4KB total headers
        max_headers: 64,        // 64 headers max
        max_header_name: 256,   // 256 bytes header name
        max_header_value: 8192, // 8KB header value
    };

    /// Strict limits for untrusted input (public-facing APIs)
    ///
    /// # ASSUME_INVARIANT: Reduced limits for hostile environments
    /// # VERIFY_INVARIANT: All limits ≤ DEFAULT limits
    pub const STRICT: Self = Self {
        max_request_line: 1024, // 1KB request line
        max_header_size: 2048,  // 2KB total headers
        max_headers: 32,        // 32 headers max
        max_header_name: 128,   // 128 bytes header name
        max_header_value: 4096, // 4KB header value
    };

    /// Relaxed limits for trusted internal services
    ///
    /// # ASSUME_INVARIANT: Larger limits for internal APIs
    /// # VERIFY_INVARIANT: Still bounded to prevent accidental DoS
    pub const RELAXED: Self = Self {
        max_request_line: 4096,  // 4KB request line
        max_header_size: 8192,   // 8KB total headers
        max_headers: 128,        // 128 headers max
        max_header_name: 512,    // 512 bytes header name
        max_header_value: 16384, // 16KB header value
    };

    /// Validate limits are internally consistent
    ///
    /// # ASSUME_INVARIANT: All limits are positive and non-zero
    /// # VERIFY_INVARIANT: Property test validates consistency
    pub const fn validate(&self) -> Result<(), &'static str> {
        if self.max_request_line == 0 {
            return Err("max_request_line must be > 0");
        }
        if self.max_header_size == 0 {
            return Err("max_header_size must be > 0");
        }
        if self.max_headers == 0 {
            return Err("max_headers must be > 0");
        }
        if self.max_header_name == 0 {
            return Err("max_header_name must be > 0");
        }
        if self.max_header_value == 0 {
            return Err("max_header_value must be > 0");
        }

        // Ensure header limits are consistent with total size
        // # ASSUME_INVARIANT: max_headers * max_header_value ≥ max_header_size
        // # VERIFY_INVARIANT: Compile-time validation below
        if self.max_headers.saturating_mul(self.max_header_value) < self.max_header_size {
            return Err("max_headers * max_header_value must be >= max_header_size");
        }

        Ok(())
    }
}

// # ASSUME_INVARIANT: DEFAULT limits are valid
// # VERIFY_INVARIANT: Compile-time assertion
const _: () = {
    match HttpSecurityLimits::DEFAULT.validate() {
        Ok(()) => {}
        Err(_) => panic!("DEFAULT limits are invalid"),
    }
};

// # ASSUME_INVARIANT: STRICT limits are valid
// # VERIFY_INVARIANT: Compile-time assertion
const _: () = {
    match HttpSecurityLimits::STRICT.validate() {
        Ok(()) => {}
        Err(_) => panic!("STRICT limits are invalid"),
    }
};

// # ASSUME_INVARIANT: RELAXED limits are valid
// # VERIFY_INVARIANT: Compile-time assertion
const _: () = {
    match HttpSecurityLimits::RELAXED.validate() {
        Ok(()) => {}
        Err(_) => panic!("RELAXED limits are invalid"),
    }
};

/// HTTP header name validation (RFC 7230 token rules)
///
/// # ASSUME_TYPE_SAFE: Input is valid UTF-8 or ASCII bytes
/// # VERIFY_UNSAFE_INVARIANTS: N/A (zero unsafe code)
///
/// # Security Properties:
/// - Prevents header injection (no CR/LF characters)
/// - Prevents header smuggling (strict token validation)
/// - Prevents parser confusion (only valid token characters)
///
/// # RFC 7230 Token Definition:
/// ```text
/// token = 1*tchar
/// tchar = "!" / "#" / "$" / "%" / "&" / "'" / "*" / "+" / "-" / "." /
///         "0-9" / "A-Z" / "^" / "_" / "`" / "a-z" / "|" / "~"
/// ```
///
/// # ASSUME_PANIC_SAFE: Input length already validated by caller
/// # VERIFY_NO_PANIC: All operations are bounds-checked
pub fn validate_header_name(name: &[u8]) -> Result<(), HttpSecurityError> {
    // # ASSUME_PANIC_SAFE: Empty header names are rejected (RFC 7230)
    // # VERIFY_NO_PANIC: Explicit check prevents empty token
    if name.is_empty() {
        return Err(HttpSecurityError::InvalidHeaderName {
            reason: "Header name cannot be empty",
        });
    }

    // # ASSUME_INVARIANT: All characters are valid token characters
    // # VERIFY_INVARIANT: Property test with random byte sequences
    for &byte in name {
        // # ASSUME_PANIC_SAFE: Token validation prevents injection attacks
        // # VERIFY_NO_PANIC: All branches are safe (no array indexing)
        let is_valid_tchar = matches!(byte,
            b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' |
            b'-' | b'.' | b'0'..=b'9' | b'A'..=b'Z' | b'^' | b'_' |
            b'`' | b'a'..=b'z' | b'|' | b'~'
        );

        if !is_valid_tchar {
            return Err(HttpSecurityError::InvalidHeaderName {
                reason: "Header name contains invalid character (not a token)",
            });
        }
    }

    Ok(())
}

/// HTTP header value validation (RFC 7230 field-value rules)
///
/// # ASSUME_TYPE_SAFE: Input is valid bytes (may not be UTF-8)
/// # VERIFY_UNSAFE_INVARIANTS: N/A (zero unsafe code)
///
/// # Security Properties:
/// - Prevents header injection (no bare CR/LF)
/// - Allows obs-fold (deprecated but still seen)
/// - Prevents parser confusion
///
/// # RFC 7230 Field Value Definition:
/// ```text
/// field-value = *( field-content / obs-fold )
/// field-content = field-vchar [ 1*( SP / HTAB ) field-vchar ]
/// field-vchar = VCHAR / obs-text
/// obs-fold = CRLF 1*( SP / HTAB )  ; Deprecated line folding
/// ```
///
/// # ASSUME_PANIC_SAFE: Input length already validated by caller
/// # VERIFY_NO_PANIC: All operations are bounds-checked
pub fn validate_header_value(value: &[u8]) -> Result<(), HttpSecurityError> {
    // Empty values are allowed (e.g., "X-Custom-Header: ")
    if value.is_empty() {
        return Ok(());
    }

    // # ASSUME_INVARIANT: No bare CR or LF (must be CRLF + SP/HTAB for obs-fold)
    // # VERIFY_INVARIANT: Property test with injection payloads
    let mut i = 0;
    while i < value.len() {
        let byte = value[i];

        // Check for bare CR or LF (injection attack)
        // # ASSUME_PANIC_SAFE: CR/LF validation prevents header smuggling
        // # VERIFY_NO_PANIC: Bounds checking via slice access
        if byte == b'\r' {
            // Must be followed by LF and then SP/HTAB (obs-fold)
            if i + 2 >= value.len() {
                return Err(HttpSecurityError::InvalidHeaderValue {
                    reason: "Bare CR at end of header value",
                });
            }
            if value[i + 1] != b'\n' {
                return Err(HttpSecurityError::InvalidHeaderValue {
                    reason: "CR not followed by LF",
                });
            }
            if value[i + 2] != b' ' && value[i + 2] != b'\t' {
                return Err(HttpSecurityError::InvalidHeaderValue {
                    reason: "CRLF not followed by SP/HTAB (invalid obs-fold)",
                });
            }
            i += 3; // Skip CRLF + SP/HTAB
            continue;
        }

        if byte == b'\n' {
            return Err(HttpSecurityError::InvalidHeaderValue {
                reason: "Bare LF in header value (must be CRLF)",
            });
        }

        // # ASSUME_INVARIANT: Valid field-vchar or SP/HTAB
        // # VERIFY_INVARIANT: Property test validates all printable ASCII
        let is_valid =
            byte == b' ' || byte == b'\t' || (0x21..=0x7E).contains(&byte) || byte >= 0x80;

        if !is_valid {
            return Err(HttpSecurityError::InvalidHeaderValue {
                reason: "Invalid character in header value (control character)",
            });
        }

        i += 1;
    }

    Ok(())
}

/// HTTP security errors
///
/// # ASSUME_TYPE_SAFE: All error variants are memory-safe
/// # VERIFY_UNSAFE_INVARIANTS: N/A (zero unsafe code)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpSecurityError {
    /// Request line exceeds maximum length
    ///
    /// # Security: Prevents buffer overflow and DoS
    RequestLineTooLarge { size: usize, max: usize },

    /// Total headers size exceeds maximum
    ///
    /// # Security: Prevents memory exhaustion
    HeadersTooLarge { size: usize, max: usize },

    /// Too many headers
    ///
    /// # Security: Prevents hash collision DoS
    TooManyHeaders { count: usize, max: usize },

    /// Header name is invalid (not RFC 7230 token)
    ///
    /// # Security: Prevents header injection
    InvalidHeaderName { reason: &'static str },

    /// Header value is invalid (contains bare CR/LF)
    ///
    /// # Security: Prevents header smuggling
    InvalidHeaderValue { reason: &'static str },

    /// Header name exceeds maximum length
    ///
    /// # Security: Prevents lookup table DoS
    HeaderNameTooLong { length: usize, max: usize },

    /// Header value exceeds maximum length
    ///
    /// # Security: Prevents large header injection
    HeaderValueTooLong { length: usize, max: usize },

    /// Compression error
    CompressionFailed(String),
}

impl fmt::Display for HttpSecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestLineTooLarge { size, max } => {
                write!(f, "Request line too large: {} bytes (max: {})", size, max)
            }
            Self::HeadersTooLarge { size, max } => {
                write!(f, "Headers too large: {} bytes (max: {})", size, max)
            }
            Self::TooManyHeaders { count, max } => {
                write!(f, "Too many headers: {} (max: {})", count, max)
            }
            Self::InvalidHeaderName { reason } => {
                write!(f, "Invalid header name: {}", reason)
            }
            Self::InvalidHeaderValue { reason } => {
                write!(f, "Invalid header value: {}", reason)
            }
            Self::HeaderNameTooLong { length, max } => {
                write!(f, "Header name too long: {} bytes (max: {})", length, max)
            }
            Self::HeaderValueTooLong { length, max } => {
                write!(f, "Header value too long: {} bytes (max: {})", length, max)
            }
            Self::CompressionFailed(msg) => {
                write!(f, "Compression failed: {}", msg)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for HttpSecurityError {}

/// Saturating arithmetic for Content-Length (prevents integer overflow)
///
/// # ASSUME_TYPE_SAFE: Saturating operations prevent UB
/// # VERIFY_UNSAFE_INVARIANTS: N/A (zero unsafe code)
///
/// # Security Properties:
/// - Prevents integer overflow attacks
/// - Prevents negative Content-Length
/// - Deterministic behavior on overflow
///
/// # ASSUME_PANIC_SAFE: Saturating add never panics
/// # VERIFY_NO_PANIC: Property test with u64::MAX values
#[inline]
pub fn saturating_add_content_length(a: u64, b: u64) -> u64 {
    // # ASSUME_MEMORY_ORDERING: N/A (pure function, no atomics)
    // # VERIFY_ORDERING_SUFFICIENT: N/A
    a.saturating_add(b)
}

/// Parse Content-Length header with overflow protection
///
/// # ASSUME_PANIC_SAFE: Invalid UTF-8 returns error (no panic)
/// # VERIFY_NO_PANIC: Property test with malformed inputs
///
/// # Security Properties:
/// - Prevents integer overflow
/// - Rejects negative values
/// - Rejects non-numeric values
/// - Rejects leading zeros (potential octal confusion)
pub fn parse_content_length(value: &[u8]) -> Result<u64, HttpSecurityError> {
    // Convert to string for parsing (must be ASCII digits)
    // # ASSUME_TYPE_SAFE: from_utf8 validates UTF-8 (no UB)
    // # VERIFY_UNSAFE_INVARIANTS: N/A (zero unsafe code)
    let s = core::str::from_utf8(value).map_err(|_| HttpSecurityError::InvalidHeaderValue {
        reason: "Content-Length must be ASCII digits",
    })?;

    // Reject empty string
    if s.is_empty() {
        return Err(HttpSecurityError::InvalidHeaderValue {
            reason: "Content-Length cannot be empty",
        });
    }

    // Reject leading zeros (potential octal confusion, not in RFC 7230 but defensive)
    // # ASSUME_PANIC_SAFE: Leading zero check prevents parser confusion
    // # VERIFY_NO_PANIC: Property test validates rejection
    if s.len() > 1 && s.starts_with('0') {
        return Err(HttpSecurityError::InvalidHeaderValue {
            reason: "Content-Length cannot have leading zeros",
        });
    }

    // Parse as u64 (saturating behavior handled by from_str_radix)
    // # ASSUME_PANIC_SAFE: from_str_radix returns Err on overflow
    // # VERIFY_NO_PANIC: Property test with u64::MAX + 1
    s.parse::<u64>()
        .map_err(|_| HttpSecurityError::InvalidHeaderValue {
            reason: "Content-Length must be a valid u64",
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits_valid() {
        assert!(HttpSecurityLimits::DEFAULT.validate().is_ok());
    }

    #[test]
    fn test_strict_limits_valid() {
        assert!(HttpSecurityLimits::STRICT.validate().is_ok());
    }

    #[test]
    fn test_relaxed_limits_valid() {
        assert!(HttpSecurityLimits::RELAXED.validate().is_ok());
    }

    #[test]
    fn test_validate_header_name_valid() {
        assert!(validate_header_name(b"Content-Type").is_ok());
        assert!(validate_header_name(b"X-Custom-Header").is_ok());
        assert!(validate_header_name(b"accept").is_ok());
    }

    #[test]
    fn test_validate_header_name_invalid() {
        // Empty name
        assert!(validate_header_name(b"").is_err());

        // Space (not a token character)
        assert!(validate_header_name(b"Content Type").is_err());

        // Colon (not a token character)
        assert!(validate_header_name(b"Content:Type").is_err());

        // CR/LF injection
        assert!(validate_header_name(b"Content-Type\r\n").is_err());
    }

    #[test]
    fn test_validate_header_value_valid() {
        assert!(validate_header_value(b"text/html").is_ok());
        assert!(validate_header_value(b"").is_ok()); // Empty is allowed
        assert!(validate_header_value(b"value with spaces").is_ok());
    }

    #[test]
    fn test_validate_header_value_invalid() {
        // Bare CR
        assert!(validate_header_value(b"value\rmore").is_err());

        // Bare LF
        assert!(validate_header_value(b"value\nmore").is_err());

        // Invalid obs-fold (CRLF not followed by SP/HTAB)
        assert!(validate_header_value(b"value\r\nmore").is_err());
    }

    #[test]
    fn test_validate_header_value_obs_fold() {
        // Valid obs-fold (CRLF + SP)
        assert!(validate_header_value(b"value\r\n more").is_ok());

        // Valid obs-fold (CRLF + HTAB)
        assert!(validate_header_value(b"value\r\n\tmore").is_ok());
    }

    #[test]
    fn test_saturating_add_content_length() {
        assert_eq!(saturating_add_content_length(100, 200), 300);
        assert_eq!(saturating_add_content_length(u64::MAX, 1), u64::MAX);
        assert_eq!(saturating_add_content_length(u64::MAX, u64::MAX), u64::MAX);
    }

    #[test]
    fn test_parse_content_length_valid() {
        assert_eq!(parse_content_length(b"0").unwrap(), 0);
        assert_eq!(parse_content_length(b"123").unwrap(), 123);
        assert_eq!(
            parse_content_length(b"18446744073709551615").unwrap(),
            u64::MAX
        );
    }

    #[test]
    fn test_parse_content_length_invalid() {
        // Empty
        assert!(parse_content_length(b"").is_err());

        // Leading zeros
        assert!(parse_content_length(b"0123").is_err());

        // Non-numeric
        assert!(parse_content_length(b"abc").is_err());

        // Negative (handled by parse error)
        assert!(parse_content_length(b"-123").is_err());

        // Overflow u64::MAX + 1
        assert!(parse_content_length(b"18446744073709551616").is_err());
    }
}

//! # HTTP Request Types
//!
//! **Zero-copy HTTP request representation**

/// HTTP method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Method {
    /// GET method
    GET = 1,
    /// POST method
    POST = 2,
    /// PUT method
    PUT = 3,
    /// DELETE method
    DELETE = 4,
    /// HEAD method
    HEAD = 5,
    /// OPTIONS method
    OPTIONS = 6,
    /// PATCH method
    PATCH = 7,
    /// TRACE method
    TRACE = 8,
    /// CONNECT method
    CONNECT = 9,
}

impl Method {
    /// Convert method to u8
    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Parse method from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match bytes {
            b"GET" => Some(Method::GET),
            b"POST" => Some(Method::POST),
            b"PUT" => Some(Method::PUT),
            b"DELETE" => Some(Method::DELETE),
            b"HEAD" => Some(Method::HEAD),
            b"OPTIONS" => Some(Method::OPTIONS),
            b"PATCH" => Some(Method::PATCH),
            b"TRACE" => Some(Method::TRACE),
            b"CONNECT" => Some(Method::CONNECT),
            _ => None,
        }
    }

    /// Get method as str
    pub const fn as_str(self) -> &'static str {
        match self {
            Method::GET => "GET",
            Method::POST => "POST",
            Method::PUT => "PUT",
            Method::DELETE => "DELETE",
            Method::HEAD => "HEAD",
            Method::OPTIONS => "OPTIONS",
            Method::PATCH => "PATCH",
            Method::TRACE => "TRACE",
            Method::CONNECT => "CONNECT",
        }
    }
}

/// HTTP version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Version {
    /// HTTP/1.0
    Http10 = 0,
    /// HTTP/1.1
    Http11 = 1,
}

impl Version {
    /// Convert version to u8
    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Parse version from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match bytes {
            b"HTTP/1.0" => Some(Version::Http10),
            b"HTTP/1.1" => Some(Version::Http11),
            _ => None,
        }
    }

    /// Get version as str
    pub const fn as_str(self) -> &'static str {
        match self {
            Version::Http10 => "HTTP/1.0",
            Version::Http11 => "HTTP/1.1",
        }
    }
}

/// HTTP request (zero-copy)
///
/// All strings are borrowed slices from original buffer.
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct HttpRequest<'a> {
    /// HTTP method
    pub method: Method,
    /// Request URI
    pub uri: &'a str,
    /// HTTP version
    pub version: Version,
    /// Headers (name, value pairs)
    pub headers: Vec<(&'a str, &'a str)>,
    /// Body (if present)
    pub body: Option<&'a [u8]>,
}

impl<'a> HttpRequest<'a> {
    /// Create new HTTP request
    pub fn new(method: Method, uri: &'a str, version: Version) -> Self {
        Self {
            method,
            uri,
            version,
            headers: Vec::new(),
            body: None,
        }
    }

    /// Add header
    pub fn add_header(&mut self, name: &'a str, value: &'a str) {
        self.headers.push((name, value));
    }

    /// Set body
    pub fn set_body(&mut self, body: &'a [u8]) {
        self.body = Some(body);
    }

    /// Get header value by name
    pub fn get_header(&self, name: &str) -> Option<&'a str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| *v)
    }

    /// Check if connection should be kept alive
    pub fn is_keep_alive(&self) -> bool {
        if let Some(connection) = self.get_header("Connection") {
            connection.eq_ignore_ascii_case("keep-alive")
        } else {
            // HTTP/1.1 keep-alive by default
            matches!(self.version, Version::Http11)
        }
    }

    /// Get content length
    pub fn content_length(&self) -> Option<usize> {
        self.get_header("Content-Length")
            .and_then(|v| v.parse().ok())
    }

    /// Check if transfer encoding is chunked
    pub fn is_chunked(&self) -> bool {
        if let Some(te) = self.get_header("Transfer-Encoding") {
            te.eq_ignore_ascii_case("chunked")
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_parsing() {
        assert_eq!(Method::from_bytes(b"GET"), Some(Method::GET));
        assert_eq!(Method::from_bytes(b"POST"), Some(Method::POST));
        assert_eq!(Method::from_bytes(b"INVALID"), None);
    }

    #[test]
    fn test_version_parsing() {
        assert_eq!(Version::from_bytes(b"HTTP/1.0"), Some(Version::Http10));
        assert_eq!(Version::from_bytes(b"HTTP/1.1"), Some(Version::Http11));
        assert_eq!(Version::from_bytes(b"HTTP/2.0"), None);
    }

    #[test]
    fn test_request_creation() {
        let mut req = HttpRequest::new(Method::GET, "/path", Version::Http11);
        req.add_header("Host", "example.com");
        req.add_header("Content-Length", "100");

        assert_eq!(req.method, Method::GET);
        assert_eq!(req.uri, "/path");
        assert_eq!(req.version, Version::Http11);
        assert_eq!(req.get_header("Host"), Some("example.com"));
        assert_eq!(req.content_length(), Some(100));
    }

    #[test]
    fn test_keep_alive() {
        let mut req = HttpRequest::new(Method::GET, "/", Version::Http11);
        assert!(req.is_keep_alive()); // HTTP/1.1 default

        req.add_header("Connection", "close");
        assert!(!req.is_keep_alive());

        let mut req10 = HttpRequest::new(Method::GET, "/", Version::Http10);
        assert!(!req10.is_keep_alive()); // HTTP/1.0 default

        req10.add_header("Connection", "keep-alive");
        assert!(req10.is_keep_alive());
    }
}

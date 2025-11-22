//! # HTTP Response Types
//!
//! **Zero-copy HTTP response representation**

use super::request::Version;

/// HTTP status code
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum StatusCode {
    // 1xx Informational
    Continue = 100,
    SwitchingProtocols = 101,

    // 2xx Success
    Ok = 200,
    Created = 201,
    Accepted = 202,
    NoContent = 204,

    // 3xx Redirection
    MovedPermanently = 301,
    Found = 302,
    SeeOther = 303,
    NotModified = 304,

    // 4xx Client Error
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    RequestTimeout = 408,

    // 5xx Server Error
    InternalServerError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
    GatewayTimeout = 504,
}

impl StatusCode {
    /// Parse status code from u16
    pub fn from_u16(code: u16) -> Option<Self> {
        match code {
            100 => Some(StatusCode::Continue),
            101 => Some(StatusCode::SwitchingProtocols),
            200 => Some(StatusCode::Ok),
            201 => Some(StatusCode::Created),
            202 => Some(StatusCode::Accepted),
            204 => Some(StatusCode::NoContent),
            301 => Some(StatusCode::MovedPermanently),
            302 => Some(StatusCode::Found),
            303 => Some(StatusCode::SeeOther),
            304 => Some(StatusCode::NotModified),
            400 => Some(StatusCode::BadRequest),
            401 => Some(StatusCode::Unauthorized),
            403 => Some(StatusCode::Forbidden),
            404 => Some(StatusCode::NotFound),
            405 => Some(StatusCode::MethodNotAllowed),
            408 => Some(StatusCode::RequestTimeout),
            500 => Some(StatusCode::InternalServerError),
            501 => Some(StatusCode::NotImplemented),
            502 => Some(StatusCode::BadGateway),
            503 => Some(StatusCode::ServiceUnavailable),
            504 => Some(StatusCode::GatewayTimeout),
            _ => None,
        }
    }

    /// Get status code as u16
    #[inline(always)]
    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    /// Get reason phrase
    pub const fn reason_phrase(self) -> &'static str {
        match self {
            StatusCode::Continue => "Continue",
            StatusCode::SwitchingProtocols => "Switching Protocols",
            StatusCode::Ok => "OK",
            StatusCode::Created => "Created",
            StatusCode::Accepted => "Accepted",
            StatusCode::NoContent => "No Content",
            StatusCode::MovedPermanently => "Moved Permanently",
            StatusCode::Found => "Found",
            StatusCode::SeeOther => "See Other",
            StatusCode::NotModified => "Not Modified",
            StatusCode::BadRequest => "Bad Request",
            StatusCode::Unauthorized => "Unauthorized",
            StatusCode::Forbidden => "Forbidden",
            StatusCode::NotFound => "Not Found",
            StatusCode::MethodNotAllowed => "Method Not Allowed",
            StatusCode::RequestTimeout => "Request Timeout",
            StatusCode::InternalServerError => "Internal Server Error",
            StatusCode::NotImplemented => "Not Implemented",
            StatusCode::BadGateway => "Bad Gateway",
            StatusCode::ServiceUnavailable => "Service Unavailable",
            StatusCode::GatewayTimeout => "Gateway Timeout",
        }
    }

    /// Check if status is informational (1xx)
    #[inline(always)]
    pub const fn is_informational(self) -> bool {
        (self as u16) >= 100 && (self as u16) < 200
    }

    /// Check if status is success (2xx)
    #[inline(always)]
    pub const fn is_success(self) -> bool {
        (self as u16) >= 200 && (self as u16) < 300
    }

    /// Check if status is redirection (3xx)
    #[inline(always)]
    pub const fn is_redirection(self) -> bool {
        (self as u16) >= 300 && (self as u16) < 400
    }

    /// Check if status is client error (4xx)
    #[inline(always)]
    pub const fn is_client_error(self) -> bool {
        (self as u16) >= 400 && (self as u16) < 500
    }

    /// Check if status is server error (5xx)
    #[inline(always)]
    pub const fn is_server_error(self) -> bool {
        (self as u16) >= 500 && (self as u16) < 600
    }
}

/// HTTP response (zero-copy)
///
/// All strings are borrowed slices from original buffer.
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct HttpResponse<'a> {
    /// HTTP version
    pub version: Version,
    /// Status code
    pub status: StatusCode,
    /// Reason phrase
    pub reason: &'a str,
    /// Headers (name, value pairs)
    pub headers: Vec<(&'a str, &'a str)>,
    /// Body (if present)
    pub body: Option<&'a [u8]>,
}

impl<'a> HttpResponse<'a> {
    /// Create new HTTP response
    pub fn new(version: Version, status: StatusCode, reason: &'a str) -> Self {
        Self {
            version,
            status,
            reason,
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
    fn test_status_code_parsing() {
        assert_eq!(StatusCode::from_u16(200), Some(StatusCode::Ok));
        assert_eq!(StatusCode::from_u16(404), Some(StatusCode::NotFound));
        assert_eq!(StatusCode::from_u16(999), None);
    }

    #[test]
    fn test_status_code_reason() {
        assert_eq!(StatusCode::Ok.reason_phrase(), "OK");
        assert_eq!(StatusCode::NotFound.reason_phrase(), "Not Found");
    }

    #[test]
    fn test_status_code_classification() {
        assert!(StatusCode::Ok.is_success());
        assert!(!StatusCode::Ok.is_client_error());

        assert!(StatusCode::NotFound.is_client_error());
        assert!(!StatusCode::NotFound.is_success());

        assert!(StatusCode::InternalServerError.is_server_error());
    }

    #[test]
    fn test_response_creation() {
        let mut resp = HttpResponse::new(Version::Http11, StatusCode::Ok, "OK");
        resp.add_header("Content-Type", "application/json");
        resp.add_header("Content-Length", "100");

        assert_eq!(resp.version, Version::Http11);
        assert_eq!(resp.status, StatusCode::Ok);
        assert_eq!(resp.get_header("Content-Type"), Some("application/json"));
        assert_eq!(resp.content_length(), Some(100));
    }

    #[test]
    fn test_response_keep_alive() {
        let mut resp = HttpResponse::new(Version::Http11, StatusCode::Ok, "OK");
        assert!(resp.is_keep_alive()); // HTTP/1.1 default

        resp.add_header("Connection", "close");
        assert!(!resp.is_keep_alive());
    }
}

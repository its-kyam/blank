//! Stores all constants used by Blank.
//! Also stores user-facing strings and errors.
//!
//! BlankError type is implemented.
//!
//! Current constants are:
//!     - LOCALHOST
//!     - DEFAULT_PORT
//!     - HTTP_BUFFER
//!     - MAX_HTTP_HEADER_SIZE
//!     - MAX_ACTIVE_CONNECTIONS
//!     - MAX_ACCEPTS_PER_TICK
//!     - HTTP_REQUEST_TIMEOUT_MS
//!     - HTTP_WRITE_TIMEOUT_MS
//!     - HTTP_HEADER_END
//!     - RUNTIME_POLL_INTERVAL_MS
//!     - SUPPORTED_HTTP_METHODS
//!     - REAL_HTTP_METHODS
//!     - SUPPORTED_HTTP_VERSIONS
//!     - CONTENT_TYPE_JSON
//!     - CONTENT_TYPE_HTML
//!     - CONTENT_TYPE_TEXT
//!     - CONTENT_TYPE_OCTET_STREAM
//!
//! Current HTTP response strings are:
//!     - HTTP_OK
//!     - HTTP_CREATED
//!     - HTTP_NO_CONTENT
//!     - HTTP_MOVED_PERMANENTLY
//!     - HTTP_FOUND
//!     - HTTP_BAD_REQUEST
//!     - HTTP_UNAUTHORIZED
//!     - HTTP_FORBIDDEN
//!     - HTTP_NOT_FOUND
//!     - HTTP_METHOD_NOT_ALLOWED
//!     - HTTP_REQUEST_TIMEOUT
//!     - HTTP_PAYLOAD_TOO_LARGE
//!     - HTTP_TOO_MANY_REQUESTS
//!     - HTTP_HEADER_FIELDS_TOO_LARGE
//!     - HTTP_INTERNAL_SERVER_ERROR
//!     - HTTP_NOT_IMPLEMENTED
//!     - HTTP_BAD_GATEWAY
//!     - HTTP_SERVICE_UNAVAILABLE
//!     - HTTP_GATEWAY_TIMEOUT
//!     - HTTP_VERSION_NOT_SUPPORTED
//!
//! Current Errors are:
//!     - ERROR_BIND_SERVER
//!     - ERROR_SET_NONBLOCKING
//!     - ERROR_ACCEPT_CONNECTION
//!
//! All constants in this file are used internally by Blank,
//! except for the public error type and error codes.

use crate::server::Port;

// User-facing strings

/// Message returned by function `hello_blank`.
pub(crate) const HELLO_MESSAGE: &str = "Hello Bro";

// Server configuration

/// Localhost address used by Blank as its default host.
///
/// Blank currently binds to localhost by default.
pub(crate) const LOCALHOST: &str = "localhost";

/// Default TCP port of a Blank object returned by `Blank::new()`.
pub(crate) const DEFAULT_PORT: Port = 7600;

// HTTP request handling

/// Number of bytes allocated for each socket-read buffer.
pub(crate) const HTTP_BUFFER: [u8; 1024] = [0u8; 1024];

/// Maximum number of bytes accepted for an HTTP request header block.
pub(crate) const MAX_HTTP_HEADER_SIZE: usize = 16 * 1024;

/// Maximum number of active client connection tasks allowed by the runtime.
pub(crate) const MAX_ACTIVE_CONNECTIONS: usize = 1024;

/// Maximum number of new connections accepted during one runtime polling cycle.
pub(crate) const MAX_ACCEPTS_PER_TICK: usize = 128;

/// Maximum time, in milliseconds, allowed for a client to send its request headers.
pub(crate) const HTTP_REQUEST_TIMEOUT_MS: u64 = 15_000;

/// Maximum time, in milliseconds, allowed for the server to write a response.
pub(crate) const HTTP_WRITE_TIMEOUT_MS: u64 = 15_000;

/// HTTP header ending sequence.
///
/// A request's header section ends with two consecutive CRLF sequences.
pub(crate) const HTTP_HEADER_END: &[u8] = b"\r\n\r\n";

// Runtime configuration

/// Time, in milliseconds, between cooperative runtime polling cycles.
///
/// This controls how frequently pending connection tasks are checked again.
pub(crate) const RUNTIME_POLL_INTERVAL_MS: u64 = 1;

// HTTP methods

/// HTTP methods currently supported by Blank.
///
/// GET is the only currently supported method.
pub(crate) const SUPPORTED_HTTP_METHODS: &[&str] = &["GET"];

/// Registered HTTP methods.
///
/// Based on the IANA HTTP Method Registry.
/// This list contains registered methods, not arbitrary extension
/// methods that could be defined in the future.
pub(crate) const REAL_HTTP_METHODS: &[&str] = &[
    "ACL",
    "BASELINE-CONTROL",
    "BIND",
    "CHECKIN",
    "CHECKOUT",
    "CONNECT",
    "COPY",
    "DELETE",
    "GET",
    "HEAD",
    "LABEL",
    "LINK",
    "LOCK",
    "MERGE",
    "MKACTIVITY",
    "MKCALENDAR",
    "MKCOL",
    "MKREDIRECTREF",
    "MKWORKSPACE",
    "MOVE",
    "OPTIONS",
    "ORDERPATCH",
    "PATCH",
    "POST",
    "PRI",
    "PROPFIND",
    "PROPPATCH",
    "PUT",
    "QUERY",
    "REBIND",
    "REPORT",
    "SEARCH",
    "TRACE",
    "UNBIND",
    "UNCHECKOUT",
    "UNLINK",
    "UNLOCK",
    "UPDATE",
    "UPDATEREDIRECTREF",
    "VERSION-CONTROL",
];

/// HTTP versions currently supported by Blank.
///
/// HTTP/1.0 and HTTP/1.1 are currently supported.
pub(crate) const SUPPORTED_HTTP_VERSIONS: &[&str] = &["HTTP/1.0", "HTTP/1.1"];

// HTTP content types

/// Content type used for JSON responses.
pub(crate) const CONTENT_TYPE_JSON: &str = "application/json; charset=utf-8";

/// Content type used for HTML responses.
pub(crate) const CONTENT_TYPE_HTML: &str = "text/html; charset=utf-8";

/// Content type used for plain-text responses.
pub(crate) const CONTENT_TYPE_TEXT: &str = "text/plain; charset=utf-8";

/// Content type used for binary responses.
///
/// Indicates that the response contains arbitrary binary data.
pub(crate) const CONTENT_TYPE_OCTET_STREAM: &str = "application/octet-stream";

// HTTP response codes

/// Successful HTTP response.
pub(crate) const HTTP_OK: &str = "200 OK";

/// HTTP response indicating that a resource was successfully created.
pub(crate) const HTTP_CREATED: &str = "201 Created";

/// HTTP response indicating successful completion without response content.
pub(crate) const HTTP_NO_CONTENT: &str = "204 No Content";

/// HTTP response indicating that a resource has permanently moved.
pub(crate) const HTTP_MOVED_PERMANENTLY: &str = "301 Moved Permanently";

/// HTTP response indicating a temporary redirect.
pub(crate) const HTTP_FOUND: &str = "302 Found";

/// HTTP response indicating that the request was malformed or invalid.
pub(crate) const HTTP_BAD_REQUEST: &str = "400 Bad Request";

/// HTTP response indicating that authentication is required.
pub(crate) const HTTP_UNAUTHORIZED: &str = "401 Unauthorized";

/// HTTP response indicating that the server refused the request.
pub(crate) const HTTP_FORBIDDEN: &str = "403 Forbidden";

/// HTTP response indicating that the requested route does not exist.
pub(crate) const HTTP_NOT_FOUND: &str = "404 Not Found";

/// HTTP response indicating that the requested method is not allowed.
pub(crate) const HTTP_METHOD_NOT_ALLOWED: &str = "405 Method Not Allowed";

/// HTTP response indicating that the client took too long to send its request.
pub(crate) const HTTP_REQUEST_TIMEOUT: &str = "408 Request Timeout";

/// HTTP response indicating that the request body exceeds the accepted size.
pub(crate) const HTTP_PAYLOAD_TOO_LARGE: &str = "413 Content Too Large";

/// HTTP response indicating that the client has sent too many requests.
pub(crate) const HTTP_TOO_MANY_REQUESTS: &str = "429 Too Many Requests";

/// HTTP response indicating that the request headers exceed the accepted size.
pub(crate) const HTTP_HEADER_FIELDS_TOO_LARGE: &str = "431 Request Header Fields Too Large";

/// HTTP response indicating an unexpected server-side failure.
pub(crate) const HTTP_INTERNAL_SERVER_ERROR: &str = "500 Internal Server Error";

/// HTTP response indicating that the requested functionality is not implemented.
pub(crate) const HTTP_NOT_IMPLEMENTED: &str = "501 Not Implemented";

/// HTTP response indicating that the server received an invalid upstream response.
pub(crate) const HTTP_BAD_GATEWAY: &str = "502 Bad Gateway";

/// HTTP response indicating that the server is temporarily unavailable.
pub(crate) const HTTP_SERVICE_UNAVAILABLE: &str = "503 Service Unavailable";

/// HTTP response indicating that the server did not receive a timely upstream response.
pub(crate) const HTTP_GATEWAY_TIMEOUT: &str = "504 Gateway Timeout";

/// HTTP response indicating that the requested HTTP version is unsupported.
pub(crate) const HTTP_VERSION_NOT_SUPPORTED: &str = "505 HTTP Version Not Supported";

// Errors

/// Error type used by Blank.
///
/// Currently represented by a `u8`.
pub type BlankError = u8;

/// Error returned when Blank fails to bind its TCP listener.
pub const ERROR_BIND_SERVER: BlankError = 1;

/// Error returned when Blank fails to configure its listener as non-blocking.
pub const ERROR_SET_NONBLOCKING: BlankError = 2;

/// Error returned when Blank encounters an unexpected TCP listener accept failure.
pub const ERROR_ACCEPT_CONNECTION: BlankError = 3;

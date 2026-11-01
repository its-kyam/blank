//! Stores all constants used by Blank
//! Also stores user-facing strings
//! Contains errors (Just &str Lol) as well
//!
//! BlankError type is implemented.
//!
//! Current constants are:
//!     - LOCALHOST
//!     - DEFAULT_PORT
//!     - HTTP_BUFFER
//!     - HTTP_HEADER_END
//!     - SUPPORTED_HTTP_METHODS
//!     - REAL_HTTP_METHODS
//!     - SUPPORTED_HTTP_VERSIONS
//!
//! Current strings:
//!     - HELLO_MESSAGE
//!     - HTTP_OK
//!     - HTTP_BAD_REQUEST
//!     - HTTP_METHOD_NOT_ALLOWED
//!     - HTTP_NOT_IMPLEMENTED
//!     - HTTP_VERSION_NOT_SUPPORTED
//!
//! Current Errors are:
//!     - ERROR_BIND_SERVER

use crate::server::Port;

/// Message returned by function 'hello_blank'
pub(crate) const HELLO_MESSAGE: &str = "Hello Bro";

/// Localhost (lol)
pub(crate) const LOCALHOST: &str = "localhost";

/// Default tcp_port of Blank object returned by
/// Blank::new uses this
pub(crate) const DEFAULT_PORT: Port = 7600;

/// Number of buffer
pub(crate) const HTTP_BUFFER: [u8; 1024] = [0u8; 1024];

/// HTTP header ending sequence
pub(crate) const HTTP_HEADER_END: &[u8] = b"\r\n\r\n";

/// HTTP methods currently supported by Blank
///
/// GET is the only currently supported method.
pub(crate) const SUPPORTED_HTTP_METHODS: &[&str] = &["GET"];

/// All registered HTTP methods
///
/// Based on the IANA HTTP Method Registry.
/// This list contains registered methods, not arbitrary
/// extension methods that could be defined in the future.
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

/// HTTP versions currently supported by Blank
///
/// HTTP/1.0 and HTTP/1.1 are currently supported.
pub(crate) const SUPPORTED_HTTP_VERSIONS: &[&str] = &["HTTP/1.0", "HTTP/1.1"];

/// Successful HTTP response
pub(crate) const HTTP_OK: &str = "200 OK";

/// Bad HTTP request
pub(crate) const HTTP_BAD_REQUEST: &str = "400 Bad Request";

/// HTTP method not allowed
pub(crate) const HTTP_METHOD_NOT_ALLOWED: &str = "405 Method Not Allowed";

/// HTTP method not implemented
pub(crate) const HTTP_NOT_IMPLEMENTED: &str = "501 Not Implemented";

/// HTTP version not supported
pub(crate) const HTTP_VERSION_NOT_SUPPORTED: &str = "505 HTTP Version Not Supported";

/// Error type for this crate
pub type BlankError = u8;

// Errors

/// Error on fail to bind to a tcp port
pub const ERROR_BIND_SERVER: BlankError = 1;

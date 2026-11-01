// constants.rs
//! Stores all constants used by Blank.
//!
//! Also stores user-facing strings and errors.
//!
//! BlankError type is implemented.
//!
//! Current constants are:
//!     - LOCALHOST
//!     - DEFAULT_PORT
//!     - HTTP_BUFFER
//!     - MAX_HTTP_HEADER_SIZE
//!     - MAX_HTTP_BODY_SIZE
//!     - MAX_ACTIVE_CONNECTIONS
//!     - HTTP_REQUEST_TIMEOUT_MS
//!     - HTTP_WRITE_TIMEOUT_MS
//!     - SERVER_WAKE_TIMEOUT_MS
//!     - SERVER_THREAD_NAME
//!     - CLIENT_THREAD_NAME
//!     - HTTP_HEADER_END
//!     - HTTP_LINE_END
//!     - HTTP_METHOD_GET
//!     - HTTP_METHOD_POST
//!     - SUPPORTED_HTTP_METHODS
//!     - REAL_HTTP_METHODS
//!     - SUPPORTED_HTTP_VERSIONS
//!     - CONTENT_TYPE_JSON
//!     - CONTENT_TYPE_HTML
//!     - CONTENT_TYPE_TEXT
//!     - CONTENT_TYPE_OCTET_STREAM
//!     - CONTENT_TYPE_FORM_URLENCODED
//!     - CONTENT_TYPE_MULTIPART_FORM_DATA
//!     - MAX_FORM_FIELDS
//!     - MAX_FORM_FIELD_SIZE
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
//!     - ERROR_SET_NONBLOCKING (kept for compatibility)
//!     - ERROR_ACCEPT_CONNECTION
//!     - ERROR_SPAWN_SERVER_THREAD
//!     - ERROR_SERVER_THREAD_PANIC
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

/// Name assigned to the background server thread.
pub(crate) const SERVER_THREAD_NAME: &str = "blank-server";

/// Name assigned to each client worker thread.
pub(crate) const CLIENT_THREAD_NAME: &str = "blank-client";

/// Maximum time allowed for the local connection used to wake a stopped server.
pub(crate) const SERVER_WAKE_TIMEOUT_MS: u64 = 100;

// HTTP request handling

/// Number of bytes allocated for each socket-read buffer.
pub(crate) const HTTP_BUFFER: [u8; 1024] = [0u8; 1024];

/// Maximum number of bytes accepted for an HTTP request header block.
pub(crate) const MAX_HTTP_HEADER_SIZE: usize = 16 * 1024;

/// Maximum number of bytes accepted for an HTTP request body.
pub(crate) const MAX_HTTP_BODY_SIZE: usize = 1024 * 1024;

/// Maximum number of parsed fields accepted in one form.
pub(crate) const MAX_FORM_FIELDS: usize = 256;

/// Maximum number of bytes accepted for one parsed form field value.
pub(crate) const MAX_FORM_FIELD_SIZE: usize = 64 * 1024;

/// Maximum number of active client connection workers.
pub(crate) const MAX_ACTIVE_CONNECTIONS: usize = 1024;

/// Maximum time, in milliseconds, allowed for a client to send its request.
pub(crate) const HTTP_REQUEST_TIMEOUT_MS: u64 = 15_000;

/// Maximum time, in milliseconds, allowed for the server to write a response.
pub(crate) const HTTP_WRITE_TIMEOUT_MS: u64 = 15_000;

/// HTTP header ending sequence.
///
/// A request's header section ends with two consecutive CRLF sequences.
pub(crate) const HTTP_HEADER_END: &[u8] = b"\r\n\r\n";

/// HTTP line ending sequence.
pub(crate) const HTTP_LINE_END: &[u8] = b"\r\n";

/// HTTP header name used to identify the request body length.
pub(crate) const HTTP_CONTENT_LENGTH_HEADER: &str = "content-length";

/// HTTP header name used by multipart form fields.
pub(crate) const HTTP_CONTENT_DISPOSITION_HEADER: &str = "content-disposition";

/// Multipart parameter used to identify the request boundary.
pub(crate) const MULTIPART_BOUNDARY_PARAMETER: &str = "boundary";

/// Multipart parameter used to identify a form field name.
pub(crate) const MULTIPART_NAME_PARAMETER: &str = "name";

/// Multipart parameter used to identify a file upload.
pub(crate) const MULTIPART_FILENAME_PARAMETER: &str = "filename";

/// Prefix placed before a multipart boundary marker.
pub(crate) const MULTIPART_BOUNDARY_PREFIX: &str = "--";

/// Final marker suffix used by multipart form bodies.
pub(crate) const MULTIPART_FINAL_MARKER: &[u8] = b"--";

/// HTTP header name used to identify the request content type.
pub(crate) const HTTP_CONTENT_TYPE_HEADER: &str = "content-type";

/// HTTP header name used to identify transfer encoding.
pub(crate) const HTTP_TRANSFER_ENCODING_HEADER: &str = "transfer-encoding";

/// HTTP value used when the server closes a response connection.
pub(crate) const HTTP_CONNECTION_CLOSE: &str = "close";

/// HTTP version used in Blank response status lines.
pub(crate) const HTTP_RESPONSE_VERSION: &str = "HTTP/1.1";

/// Response header name for the body length.
pub(crate) const HTTP_RESPONSE_CONTENT_LENGTH: &str = "Content-Length";

/// Response header name for the body content type.
pub(crate) const HTTP_RESPONSE_CONTENT_TYPE: &str = "Content-Type";

/// Response header name for the connection behavior.
pub(crate) const HTTP_RESPONSE_CONNECTION: &str = "Connection";

/// Response header name used for redirects.
pub(crate) const HTTP_RESPONSE_LOCATION: &str = "Location";

/// Response header name used for cookies.
pub(crate) const HTTP_RESPONSE_SET_COOKIE: &str = "Set-Cookie";

/// Default attributes applied to cookies created by Blank.
pub(crate) const COOKIE_DEFAULT_ATTRIBUTES: &str = "Path=/; HttpOnly; SameSite=Lax";

/// Attributes applied when Blank clears a cookie.
pub(crate) const COOKIE_CLEAR_ATTRIBUTES: &str = "Path=/; Max-Age=0; HttpOnly; SameSite=Lax";

// HTTP response bodies

/// Default body for a 400 response.
pub(crate) const BODY_BAD_REQUEST: &str = "Bad Request";

/// Default body for a 401 response.
pub(crate) const BODY_UNAUTHORIZED: &str = "Unauthorized";

/// Default body for a 403 response.
pub(crate) const BODY_FORBIDDEN: &str = "Forbidden";

/// Default body for a 404 response.
pub(crate) const BODY_NOT_FOUND: &str = "Not Found";

/// Default body for a 405 response.
pub(crate) const BODY_METHOD_NOT_ALLOWED: &str = "Method Not Allowed";

/// Default body for a 408 response.
pub(crate) const BODY_REQUEST_TIMEOUT: &str = "Request Timeout";

/// Default body for a 413 response.
pub(crate) const BODY_CONTENT_TOO_LARGE: &str = "Content Too Large";

/// Default body for a 429 response.
pub(crate) const BODY_TOO_MANY_REQUESTS: &str = "Too Many Requests";

/// Default body for a 431 response.
pub(crate) const BODY_REQUEST_HEADERS_TOO_LARGE: &str = "Request headers too large";

/// Default body for a 500 response.
pub(crate) const BODY_INTERNAL_SERVER_ERROR: &str = "Internal Server Error";

/// Default body for a 501 response.
pub(crate) const BODY_NOT_IMPLEMENTED: &str = "Not Implemented";

/// Default body for a 502 response.
pub(crate) const BODY_BAD_GATEWAY: &str = "Bad Gateway";

/// Default body for a 503 response.
pub(crate) const BODY_SERVICE_UNAVAILABLE: &str = "Service Unavailable";

/// Default body for a 504 response.
pub(crate) const BODY_GATEWAY_TIMEOUT: &str = "Gateway Timeout";

/// Default body for a 505 response.
pub(crate) const BODY_HTTP_VERSION_NOT_SUPPORTED: &str = "HTTP Version Not Supported";

/// Body returned when a transfer encoding is not supported.
pub(crate) const BODY_TRANSFER_ENCODING_NOT_SUPPORTED: &str = "Transfer encoding not supported";

/// Generic body returned for other request errors.
pub(crate) const BODY_REQUEST_ERROR: &str = "Request Error";

// HTTP methods

/// HTTP methods currently supported by Blank.
///
/// GET and POST are currently supported.
pub(crate) const HTTP_METHOD_GET: &str = "GET";
pub(crate) const HTTP_METHOD_POST: &str = "POST";
pub(crate) const SUPPORTED_HTTP_METHODS: &[&str] = &[HTTP_METHOD_GET, HTTP_METHOD_POST];

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

/// Content type used for URL-encoded HTML form submissions.
pub(crate) const CONTENT_TYPE_FORM_URLENCODED: &str = "application/x-www-form-urlencoded";

/// Content type used for multipart HTML form submissions.
pub(crate) const CONTENT_TYPE_MULTIPART_FORM_DATA: &str = "multipart/form-data";

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
///
/// Kept for compatibility with the earlier polling runtime.
#[deprecated(note = "Blank now uses blocking socket readiness instead of non-blocking polling")]
pub const ERROR_SET_NONBLOCKING: BlankError = 2;

/// Error returned when Blank encounters an unexpected TCP listener accept failure.
pub const ERROR_ACCEPT_CONNECTION: BlankError = 3;

/// Error returned when Blank cannot create the background server thread.
pub const ERROR_SPAWN_SERVER_THREAD: BlankError = 4;

/// Error returned when the background server thread panics.
pub const ERROR_SERVER_THREAD_PANIC: BlankError = 5;

//! Defines the configuration values, HTTP protocol tokens, response messages, and error
//! identifiers used by Blank.
//!
//! This module is the central location for literal values that are shared by the server
//! implementation. Keeping these values together avoids scattering protocol spellings,
//! default limits, response phrases, and internal error codes throughout the crate.
//!
//! The HTTP-related constants are public so applications and integrations can refer to the
//! exact values used by Blank. Their documentation describes the meaning and intended use
//! of each value; it does not imply that changing a value at runtime is supported.
//!
//! Server configuration values that are specific to Blank's internal implementation remain
//! crate-visible. The public [`BlankError`] alias and its associated error constants provide
//! compact identifiers for the server errors that Blank exposes.

use crate::server::Port;

// User-facing strings

/// The default greeting returned by the `hello_blank` function.
///
/// This constant contains the complete message, including its capitalization and spacing.
/// It is kept separate from the function so the greeting has one canonical definition
/// rather than being repeated as a string literal at each use site.
///
/// The value is intended for the simple greeting functionality provided by Blank. It is not
/// an HTTP status phrase, protocol token, or server configuration setting.
pub(crate) const HELLO_MESSAGE: &str = "Hello Bro";

// Server configuration

/// The hostname used when a newly created Blank server has not been assigned another host.
///
/// The value is the loopback hostname `localhost`, which resolves to the local machine. It
/// makes the default binding local to the machine instead of selecting a public network
/// interface. Applications that need a different binding address can configure the host on
/// their `Blank` value rather than changing this crate default.
///
/// This is a hostname string, not a numeric IP address. Name resolution and the addresses
/// associated with `localhost` are handled by the operating system.
pub(crate) const LOCALHOST: &str = "localhost";

/// The TCP port assigned to a `Blank` value created with `Blank::new()`.
///
/// The value is `7600`. It is expressed using Blank's [`Port`] type so the default has the
/// same type as the port accepted by the server configuration API. This constant describes
/// the initial default only; a server can be configured to use another port before it starts
/// listening.
///
/// A port is a local TCP endpoint component, not a complete address. The host and port are
/// used together when Blank binds its listener.
pub(crate) const DEFAULT_PORT: Port = 7600;

/// The operating-system thread name assigned to Blank's background server thread.
///
/// The value is `blank-server`. Thread names can be useful when inspecting a process in a
/// debugger, profiler, system monitor, or diagnostic tool. This string is a diagnostic label;
/// it does not determine the thread's scheduling priority, execution policy, or lifetime.
///
/// This constant is crate-visible because the name belongs to Blank's internal threading
/// implementation rather than to the HTTP protocol or a public server configuration option.
pub(crate) const SERVER_THREAD_NAME: &str = "blank-server";

/// The operating-system thread name assigned to a client connection worker thread.
///
/// The value is `blank-client`. Blank uses this label to make its client workers easier to
/// distinguish from the background server thread when inspecting process threads with
/// diagnostic tools. It is a descriptive thread name only; it does not configure how many
/// workers are created or how requests are scheduled.
///
/// This constant is crate-visible because it is an implementation detail of Blank's worker
/// thread management.
pub(crate) const CLIENT_THREAD_NAME: &str = "blank-client";

/// The maximum duration, in milliseconds, allowed for the local wake-up connection used when
/// stopping the server.
///
/// The value is `100` milliseconds. Blank uses a local connection to wake the server's
/// listener so its shutdown path can make progress without waiting indefinitely for another
/// incoming client connection. This timeout limits that wake-up operation; it is not the
/// timeout for a normal client request or for writing an HTTP response.
///
/// The unit is milliseconds, represented as a `u64` so it can be used with duration and
/// timeout APIs that accept an integer millisecond count.
pub const SERVER_WAKE_TIMEOUT_MS: u64 = 100;

// HTTP request handling

/// The fixed-size byte buffer used for each socket read performed by the HTTP handling code.
///
/// The array contains `1024` bytes and is initialized to zero. Its length is the size of an
/// individual read buffer; it is not the maximum size of an entire HTTP request, header block,
/// or body. Those limits are defined independently by [`MAX_HTTP_HEADER_SIZE`] and
/// [`MAX_HTTP_BODY_SIZE`].
///
/// Keeping the buffer size in one constant makes the read chunk size explicit and consistent
/// across the HTTP handling implementation. The buffer is a byte array because socket reads
/// operate on bytes, before request data is interpreted as HTTP text or a body payload.
pub const HTTP_BUFFER: [u8; 1024] = [0u8; 1024];

/// The maximum number of bytes accepted for an HTTP request's header block.
///
/// The configured limit is `16 * 1024` bytes, or 16 KiB. It bounds the amount of header data
/// the request parser is willing to accept, helping prevent a client from making the server
/// accumulate an unbounded header section. The value is measured in bytes, not characters;
/// multibyte text encodings therefore consume more than one byte per character.
///
/// This limit applies to request headers, not to the request body. Body size is governed
/// separately by [`MAX_HTTP_BODY_SIZE`]. The value is a parser limit and should not be
/// interpreted as a guarantee that every request at or below the limit is otherwise valid.
pub const MAX_HTTP_HEADER_SIZE: usize = 16 * 1024;

/// The maximum number of bytes accepted for an HTTP request body.
///
/// The configured limit is `1024 * 1024` bytes, or 1 MiB. It caps the body data accepted by
/// Blank's HTTP request handling so that one request cannot cause the server to accept an
/// arbitrarily large payload. The limit is measured in bytes and is independent of the
/// request-header limit in [`MAX_HTTP_HEADER_SIZE`].
///
/// A request whose body exceeds this configured maximum is associated with the HTTP 413
/// response represented by [`HTTP_PAYLOAD_TOO_LARGE`]. The constant describes the maximum
/// accepted body size; it does not specify an application's own business-level upload limit.
pub const MAX_HTTP_BODY_SIZE: usize = 1024 * 1024;

/// The maximum number of parsed fields accepted in a single submitted form.
///
/// The value is `256` fields. It limits the number of individual form entries that Blank will
/// accept while parsing form data. A field is one parsed name/value entry; a form containing
/// repeated names can still contain multiple fields because each entry is counted separately.
///
/// This limit concerns the number of fields, not the size of each field or the total request
/// body. The maximum size of an individual field value is defined by
/// [`MAX_FORM_FIELD_SIZE`], while the total HTTP body is bounded by
/// [`MAX_HTTP_BODY_SIZE`].
pub const MAX_FORM_FIELDS: usize = 256;

/// The maximum number of bytes accepted for the value of one parsed form field.
///
/// The configured limit is `64 * 1024` bytes, or 64 KiB, per field value. It limits the size
/// of an individual parsed value rather than the number of fields in the form. The number of
/// accepted fields is controlled independently by [`MAX_FORM_FIELDS`].
///
/// The value is measured in bytes, not Unicode characters. This limit also does not replace
/// the overall request-body limit in [`MAX_HTTP_BODY_SIZE`]; a form must satisfy the relevant
/// individual and aggregate parsing limits.
pub const MAX_FORM_FIELD_SIZE: usize = 64 * 1024;

/// The maximum number of client connection workers that Blank allows to be active at once.
///
/// The configured limit is `1024`. It places an upper bound on the number of concurrently
/// active client connection workers managed by the server. It is a concurrency limit, not a
/// requests-per-second rate limit, a queue length, or a limit on the total number of clients
/// that may connect over the lifetime of the server.
///
/// This setting is independent of the per-request size limits and the request and response
/// timeouts. A connection still has to satisfy those limits while it is being handled.
pub const MAX_ACTIVE_CONNECTIONS: usize = 1024;

/// The maximum time, in milliseconds, that Blank allows a client to send an HTTP request.
///
/// The configured timeout is `15_000` milliseconds, equivalent to 15 seconds. It limits the
/// time allowed for receiving the request from a client. The unit is milliseconds and the
/// integer type is `u64`.
///
/// This is the request-receive timeout. It is distinct from
/// [`HTTP_WRITE_TIMEOUT_MS`], which limits writing the server's response, and from
/// [`SERVER_WAKE_TIMEOUT_MS`], which is used for the server's local shutdown wake-up
/// connection.
pub const HTTP_REQUEST_TIMEOUT_MS: u64 = 15_000;

/// The maximum time, in milliseconds, that Blank allows when writing an HTTP response.
///
/// The configured timeout is `15_000` milliseconds, equivalent to 15 seconds. It limits the
/// response-writing operation after the server has prepared a response. The unit is
/// milliseconds and the integer type is `u64`.
///
/// This is not the timeout for receiving a client's request; that limit is
/// [`HTTP_REQUEST_TIMEOUT_MS`]. It is also unrelated to the local wake-up connection used
/// during server shutdown, which is controlled by [`SERVER_WAKE_TIMEOUT_MS`].
pub const HTTP_WRITE_TIMEOUT_MS: u64 = 15_000;

/// The byte sequence that marks the end of an HTTP request's header section.
///
/// Its value is `\r\n\r\n`: two consecutive carriage-return/line-feed pairs. In HTTP/1.x
/// message syntax, the empty line after the header fields separates the header section from
/// the message body. This constant is a byte slice because request parsing compares raw bytes
/// read from a socket.
///
/// It is different from [`HTTP_LINE_END`], which represents the single CRLF sequence ending
/// an individual HTTP line. The header terminator is formed by two such line endings.
pub const HTTP_HEADER_END: &[u8] = b"\r\n\r\n";

/// The byte sequence used to terminate an HTTP protocol line.
///
/// Its value is `\r\n`, the carriage-return/line-feed pair used by HTTP/1.x to delimit the
/// request line and individual header lines. It is represented as bytes because the parser
/// handles data received from the network before converting relevant portions into text.
///
/// This is one line ending, whereas [`HTTP_HEADER_END`] is the two-line-ending sequence that
/// marks the end of the complete header section.
pub const HTTP_LINE_END: &[u8] = b"\r\n";

/// The normalized HTTP header name used to retrieve the declared request-body length.
///
/// The value is lowercase (`content-length`) so it matches the normalized header-name form
/// used by Blank's request handling. The corresponding HTTP field communicates the size of
/// the message body in decimal bytes. The value stored here is the header name only; it does
/// not include a colon, whitespace, or a length value.
///
/// This constant identifies a request header and should not be confused with
/// [`HTTP_RESPONSE_CONTENT_LENGTH`], which is the canonical-cased name emitted for a response
/// header.
pub const HTTP_CONTENT_LENGTH_HEADER: &str = "content-length";

/// The normalized HTTP header name used to identify a multipart part's content disposition.
///
/// The value is lowercase (`content-disposition`), matching the normalized header-name form
/// used by Blank's request handling. In multipart form data, a part's content-disposition
/// metadata can contain parameters such as a field name or a filename. This constant contains
/// only the header name, not a particular disposition value or parameter.
///
/// It is a request-side parsing token and is distinct from the response header constants.
pub const HTTP_CONTENT_DISPOSITION_HEADER: &str = "content-disposition";

/// The multipart parameter name that introduces the boundary value in a content-type value.
///
/// The literal value is `boundary`. A multipart/form-data content type uses a boundary
/// parameter to identify the delimiter that separates the individual parts in the body. This
/// constant contains the parameter name only; the boundary value itself is supplied by the
/// request and is not defined here.
///
/// The parameter name does not include an equals sign, quotation marks, or a leading
/// semicolon, so parsing code can compare it independently of the parameter's value.
pub const MULTIPART_BOUNDARY_PARAMETER: &str = "boundary";

/// The multipart content-disposition parameter name that identifies a form field's name.
///
/// The literal value is `name`. Multipart form parts commonly carry this parameter in their
/// content-disposition metadata to identify the form field represented by that part. This
/// constant stores the parameter name only; it does not contain a field name supplied by a
/// client.
///
/// It is separate from [`MULTIPART_FILENAME_PARAMETER`], which identifies a filename
/// parameter when a multipart part represents a file.
pub const MULTIPART_NAME_PARAMETER: &str = "name";

/// The multipart content-disposition parameter name that identifies a submitted filename.
///
/// The literal value is `filename`. It is the parameter name used to recognize filename
/// metadata in a multipart part's content-disposition value. The actual filename is request
/// data and is not stored in this constant.
///
/// The presence of this parameter distinguishes filename metadata from the ordinary field
/// name parameter represented by [`MULTIPART_NAME_PARAMETER`]. This constant does not define
/// a filesystem path or imply that a received filename should be trusted as one.
pub const MULTIPART_FILENAME_PARAMETER: &str = "filename";

/// The character prefix placed before a multipart boundary value when forming a delimiter.
///
/// The literal value is two hyphens (`--`). Multipart bodies use this prefix together with
/// the boundary token to mark the beginning of a part delimiter. The boundary token itself
/// comes from the request's content-type parameter and is not included in this constant.
///
/// This string is the prefix only; it does not include the boundary, trailing CRLF, or the
/// final-boundary suffix.
pub const MULTIPART_BOUNDARY_PREFIX: &str = "--";

/// The byte suffix that marks the final multipart boundary delimiter.
///
/// Its value is two hyphen bytes (`--`). In multipart message syntax, the final delimiter is
/// distinguished from an ordinary part delimiter by appending this suffix to the boundary
/// marker. It is represented as bytes because multipart body parsing operates on the raw
/// request payload.
///
/// This is the suffix only; the boundary prefix and actual boundary token are represented
/// separately by [`MULTIPART_BOUNDARY_PREFIX`] and request data.
pub const MULTIPART_FINAL_MARKER: &[u8] = b"--";

/// The normalized HTTP header name used to retrieve a request's media type.
///
/// The value is lowercase (`content-type`), matching the normalized header-name form used by
/// Blank's request handling. A request's content-type field describes the media type of its
/// body and can include parameters, such as a multipart boundary or a character-set
/// declaration. This constant contains the header name only.
///
/// It is distinct from [`HTTP_RESPONSE_CONTENT_TYPE`], which is the canonical-cased response
/// header name used when constructing an HTTP response.
pub const HTTP_CONTENT_TYPE_HEADER: &str = "content-type";

/// The normalized HTTP header name used to identify a request's transfer-encoding metadata.
///
/// The value is lowercase (`transfer-encoding`), matching the normalized header-name form
/// used by Blank's request handling. Transfer encoding describes message-body transfer
/// framing or coding; it is not the same as the media type identified by
/// [`HTTP_CONTENT_TYPE_HEADER`].
///
/// This constant is the header name only. It does not declare which transfer codings Blank
/// supports; that behavior is determined by the request-handling implementation.
pub const HTTP_TRANSFER_ENCODING_HEADER: &str = "transfer-encoding";

/// The HTTP connection-header value used when Blank closes a response connection.
///
/// The literal value is `close`. It is the value, rather than the header name, placed in the
/// connection-related response metadata to communicate that the connection will not be kept
/// open for another request. The response header name itself is defined by
/// [`HTTP_RESPONSE_CONNECTION`].
///
/// This constant does not represent a socket operation or a timeout; it is the HTTP field
/// value used to describe the connection behavior.
pub const HTTP_CONNECTION_CLOSE: &str = "close";

/// The HTTP protocol-version token written in Blank's response status lines.
///
/// The value is `HTTP/1.1`. It identifies the format/version token used when serializing a
/// response status line. It is intentionally separate from [`SUPPORTED_HTTP_VERSIONS`],
/// which lists request versions accepted by the current request handling code.
///
/// This constant does not change the request version supplied by a client; it defines the
/// version token used by Blank when producing its own response status line.
pub const HTTP_RESPONSE_VERSION: &str = "HTTP/1.1";

/// The canonical HTTP response-header name used to report the response body length.
///
/// The value is `Content-Length`, including its canonical capitalization. The associated
/// field communicates the size of the response message body in bytes. This constant contains
/// the field name only; the numeric length is calculated from the response body when the
/// response is serialized.
///
/// It is the response-side counterpart to [`HTTP_CONTENT_LENGTH_HEADER`], whose lowercase
/// value is used when reading a request header.
pub const HTTP_RESPONSE_CONTENT_LENGTH: &str = "Content-Length";

/// The canonical HTTP response-header name used to report the response media type.
///
/// The value is `Content-Type`, including its canonical capitalization. The associated
/// field communicates the media type of the response body and may include parameters such
/// as a character set. The media-type values used by Blank are defined separately by the
/// [`CONTENT_TYPE_JSON`], [`CONTENT_TYPE_HTML`], [`CONTENT_TYPE_TEXT`], and related constants.
///
/// This is a response header name, not a media-type value itself.
pub const HTTP_RESPONSE_CONTENT_TYPE: &str = "Content-Type";

/// The canonical HTTP response-header name used to describe connection behavior.
///
/// The value is `Connection`. Blank uses this field name when writing connection-related
/// response metadata; the value [`HTTP_CONNECTION_CLOSE`] represents the `close` directive.
///
/// This constant contains only the header name. It does not itself open, close, or otherwise
/// operate on a network connection.
pub const HTTP_RESPONSE_CONNECTION: &str = "Connection";

/// The canonical HTTP response-header name used to provide a redirect destination.
///
/// The value is `Location`. HTTP redirect responses use this field to identify the target
/// resource to which the client is being directed. The destination value is supplied by the
/// response being created; it is not part of this constant.
///
/// This constant contains the header name only and is used with redirect status responses
/// such as [`HTTP_MOVED_PERMANENTLY`] and [`HTTP_FOUND`].
pub const HTTP_RESPONSE_LOCATION: &str = "Location";

/// The canonical HTTP response-header name used to send a cookie to the client.
///
/// The value is `Set-Cookie`. A response can use this field to instruct a client to store or
/// update a cookie. Cookie content and attributes are supplied separately; this constant is
/// only the response header name.
///
/// Blank's default cookie attribute strings are defined by [`COOKIE_DEFAULT_ATTRIBUTES`]
/// and [`COOKIE_CLEAR_ATTRIBUTES`].
pub const HTTP_RESPONSE_SET_COOKIE: &str = "Set-Cookie";

/// The default attributes appended to cookies created by Blank.
///
/// The literal value is `Path=/; HttpOnly; SameSite=Lax`. It sets the cookie path to the root
/// path, marks the cookie as unavailable to ordinary script access through the `HttpOnly`
/// attribute, and selects the `Lax` SameSite policy. The attributes are represented as one
/// semicolon-separated string so they can be included in a `Set-Cookie` field value.
///
/// This constant defines the default attribute text only. It does not contain a cookie name
/// or cookie value, and it does not include a `Secure` attribute. Applications that require
/// additional cookie policy must account for that separately.
pub const COOKIE_DEFAULT_ATTRIBUTES: &str = "Path=/; HttpOnly; SameSite=Lax";

/// The cookie attributes used by Blank when clearing a cookie.
///
/// The literal value is `Path=/; Max-Age=0; HttpOnly; SameSite=Lax`. The root path matches the
/// default cookie path, while `Max-Age=0` instructs the client to expire the cookie
/// immediately. The `HttpOnly` and `SameSite=Lax` attributes are retained in the clearing
/// value as well.
///
/// This constant contains attributes only. The cookie name and the response's `Set-Cookie`
/// header are supplied by the cookie-clearing operation. As with
/// [`COOKIE_DEFAULT_ATTRIBUTES`], this literal does not include a `Secure` attribute.
pub const COOKIE_CLEAR_ATTRIBUTES: &str = "Path=/; Max-Age=0; HttpOnly; SameSite=Lax";

// HTTP response bodies

/// The default response-body text associated with an HTTP 400 Bad Request response.
///
/// The literal value is `Bad Request`. It provides a short, human-readable explanation for a
/// request that Blank cannot accept because the request is malformed or otherwise invalid.
/// It is a body message, not the HTTP status-line phrase; the status line is represented by
/// [`HTTP_BAD_REQUEST`].
///
/// Applications can provide their own response body when using the response API. This
/// constant records Blank's built-in default wording.
pub const BODY_BAD_REQUEST: &str = "Bad Request";

/// The default response-body text associated with an HTTP 401 Unauthorized response.
///
/// The literal value is `Unauthorized`. It is the concise body message used for an
/// unauthorized-request response. The corresponding status-line value is
/// [`HTTP_UNAUTHORIZED`]. The constant does not perform authentication or determine whether
/// a request is authorized; it supplies the default response text only.
pub const BODY_UNAUTHORIZED: &str = "Unauthorized";

/// The default response-body text associated with an HTTP 403 Forbidden response.
///
/// The literal value is `Forbidden`. It is used as the default explanatory body when access
/// to a requested resource or operation is refused. The matching status-line value is
/// [`HTTP_FORBIDDEN`]. The constant does not implement an authorization policy; it defines
/// the message associated with the built-in response helper.
pub const BODY_FORBIDDEN: &str = "Forbidden";

/// The default response-body text associated with an HTTP 404 Not Found response.
///
/// The literal value is `Not Found`. It communicates that the requested resource or route
/// could not be found. The matching status-line value is [`HTTP_NOT_FOUND`]. This is default
/// response wording and does not itself perform route matching or decide whether a route
/// exists.
pub const BODY_NOT_FOUND: &str = "Not Found";

/// The default response-body text associated with an HTTP 405 Method Not Allowed response.
///
/// The literal value is `Method Not Allowed`. It is used when a request method is not
/// permitted for the requested resource or route. The matching status-line value is
/// [`HTTP_METHOD_NOT_ALLOWED`]. The text does not enumerate the methods accepted by a route;
/// it is the default body message only.
pub const BODY_METHOD_NOT_ALLOWED: &str = "Method Not Allowed";

/// The default response-body text associated with an HTTP 408 Request Timeout response.
///
/// The literal value is `Request Timeout`. It describes a request that was not received
/// within the allowed time. The matching status-line value is [`HTTP_REQUEST_TIMEOUT`].
/// The actual receive-time limit is configured separately by [`HTTP_REQUEST_TIMEOUT_MS`].
pub const BODY_REQUEST_TIMEOUT: &str = "Request Timeout";

/// The default response-body text associated with an HTTP 413 Content Too Large response.
///
/// The literal value is `Content Too Large`. It describes a request body that exceeds the
/// configured accepted size. The matching status-line value is [`HTTP_PAYLOAD_TOO_LARGE`],
/// and the maximum accepted request-body size is defined by [`MAX_HTTP_BODY_SIZE`].
pub const BODY_CONTENT_TOO_LARGE: &str = "Content Too Large";

/// The default response-body text associated with an HTTP 429 Too Many Requests response.
///
/// The literal value is `Too Many Requests`. It describes a request rejected because a
/// request-count or request-rate policy has been exceeded. The matching status-line value is
/// [`HTTP_TOO_MANY_REQUESTS`]. This constant supplies response wording; it does not itself
/// implement rate limiting or define a request-rate threshold.
pub const BODY_TOO_MANY_REQUESTS: &str = "Too Many Requests";

/// The default response-body text associated with an HTTP 431 Request Header Fields Too
/// Large response.
///
/// The literal value is `Request headers too large`. It describes request headers that exceed
/// the accepted size. The matching status-line value is [`HTTP_HEADER_FIELDS_TOO_LARGE`],
/// while the configured header-size limit is [`MAX_HTTP_HEADER_SIZE`]. The body text uses the
/// capitalization stored in the literal and is kept separate from the status-line phrase.
pub const BODY_REQUEST_HEADERS_TOO_LARGE: &str = "Request headers too large";

/// The default response-body text associated with an HTTP 500 Internal Server Error response.
///
/// The literal value is `Internal Server Error`. It is the generic body message for an
/// unexpected server-side failure. The matching status-line value is
/// [`HTTP_INTERNAL_SERVER_ERROR`]. The constant deliberately describes the response text,
/// not the underlying cause or diagnostic details of the failure.
pub const BODY_INTERNAL_SERVER_ERROR: &str = "Internal Server Error";

/// The default response-body text associated with an HTTP 501 Not Implemented response.
///
/// The literal value is `Not Implemented`. It describes a request for functionality that the
/// server does not implement. The matching status-line value is [`HTTP_NOT_IMPLEMENTED`].
/// The text is a generic response message and does not identify which method, feature, or
/// protocol extension is unavailable.
pub const BODY_NOT_IMPLEMENTED: &str = "Not Implemented";

/// The default response-body text associated with an HTTP 502 Bad Gateway response.
///
/// The literal value is `Bad Gateway`. It describes a gateway or proxy receiving an invalid
/// response from an upstream server. The matching status-line value is [`HTTP_BAD_GATEWAY`].
/// The constant supplies a generic body message; it does not implement upstream communication
/// or provide diagnostic information about an upstream failure.
pub const BODY_BAD_GATEWAY: &str = "Bad Gateway";

/// The default response-body text associated with an HTTP 503 Service Unavailable response.
///
/// The literal value is `Service Unavailable`. It describes a server that is temporarily
/// unable to handle a request. The matching status-line value is
/// [`HTTP_SERVICE_UNAVAILABLE`]. The constant defines the response wording only and does not
/// control server availability or recovery behavior.
pub const BODY_SERVICE_UNAVAILABLE: &str = "Service Unavailable";

/// The default response-body text associated with an HTTP 504 Gateway Timeout response.
///
/// The literal value is `Gateway Timeout`. It describes a gateway or proxy that did not
/// receive a timely response from an upstream server. The matching status-line value is
/// [`HTTP_GATEWAY_TIMEOUT`]. It is a message string, not a timeout configuration; Blank's
/// response-write timeout is separately defined by [`HTTP_WRITE_TIMEOUT_MS`].
pub const BODY_GATEWAY_TIMEOUT: &str = "Gateway Timeout";

/// The default response-body text associated with an HTTP 505 HTTP Version Not Supported
/// response.
///
/// The literal value is `HTTP Version Not Supported`. It describes a request that uses an
/// HTTP version the server does not support. The matching status-line value is
/// [`HTTP_VERSION_NOT_SUPPORTED`]. The request versions currently accepted by Blank are
/// listed in [`SUPPORTED_HTTP_VERSIONS`].
pub const BODY_HTTP_VERSION_NOT_SUPPORTED: &str = "HTTP Version Not Supported";

/// The default response-body text used when a request specifies a transfer encoding that
/// Blank does not support.
///
/// The literal value is `Transfer encoding not supported`. It is distinct from the generic
/// 501 response-body text in [`BODY_NOT_IMPLEMENTED`] and from the status-line constant
/// [`HTTP_NOT_IMPLEMENTED`]: this constant identifies the more specific explanatory text
/// associated with unsupported transfer-encoding handling.
///
/// This string does not list supported transfer codings or define the parser's transfer
/// encoding behavior; it supplies the response body wording.
pub const BODY_TRANSFER_ENCODING_NOT_SUPPORTED: &str = "Transfer encoding not supported";

/// The generic response-body text used for request errors that do not have a more specific
/// built-in body message.
///
/// The literal value is `Request Error`. It provides a fallback message for request failures
/// when a dedicated body constant is not selected. More specific cases have their own
/// constants, such as [`BODY_BAD_REQUEST`], [`BODY_REQUEST_TIMEOUT`], and
/// [`BODY_NOT_IMPLEMENTED`].
///
/// This constant is only the fallback body text; it does not identify an HTTP status code or
/// describe the underlying parsing or connection failure.
pub const BODY_REQUEST_ERROR: &str = "Request Error";

// HTTP methods

/// The HTTP method token for the GET method.
///
/// The value is `GET`. GET is included in [`SUPPORTED_HTTP_METHODS`], which lists the
/// methods currently supported by Blank's request handling. The method token is kept in a
/// named constant so the parser and other HTTP-related code can use one canonical spelling.
///
/// This constant represents the method name only. It does not include a request target,
/// protocol version, or any other part of an HTTP request line.
pub const HTTP_METHOD_GET: &str = "GET";

/// The HTTP method token for the POST method.
///
/// The value is `POST`. POST is included in [`SUPPORTED_HTTP_METHODS`], which lists the
/// methods currently supported by Blank's request handling. The method token is kept in a
/// named constant so the parser and other HTTP-related code can use one canonical spelling.
///
/// This constant represents the method name only. It does not include a request target,
/// protocol version, or request body.
pub const HTTP_METHOD_POST: &str = "POST";

/// The set of HTTP method tokens currently supported by Blank's request handling.
///
/// The slice contains [`HTTP_METHOD_GET`] and [`HTTP_METHOD_POST`]. A method's presence in
/// this list means that Blank currently recognizes it as a supported request method; it does
/// not describe every method registered by the HTTP ecosystem. The broader registered-method
/// list is [`REAL_HTTP_METHODS`].
///
/// This distinction is important when classifying an incoming method: a method can be a
/// registered HTTP method without being implemented by Blank. The list is a slice of string
/// references so the method tokens can be compared without allocating new strings.
pub const SUPPORTED_HTTP_METHODS: &[&str] = &[HTTP_METHOD_GET, HTTP_METHOD_POST];

/// The collection of registered HTTP method tokens recognized by Blank for method
/// classification.
///
/// The entries are based on the IANA HTTP Method Registry and include methods beyond the
/// smaller set currently supported by Blank. This list is intentionally broader than
/// [`SUPPORTED_HTTP_METHODS`]: being present here identifies a registered method; it does
/// not mean Blank implements that method's semantics or accepts it for normal request
/// handling.
///
/// The distinction allows request handling to separate a known-but-not-implemented method
/// from an unrecognized method token. The list contains the method names as string slices and
/// does not define behavior for extension methods that are not included in this registry
/// snapshot.
pub const REAL_HTTP_METHODS: &[&str] = &[
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

/// The HTTP protocol versions currently accepted by Blank's request handling.
///
/// The slice contains `HTTP/1.0` and `HTTP/1.1`. These are the request-version tokens that
/// the current implementation is configured to support. A request that specifies a version
/// outside this set is not made valid merely because the version token follows the general
/// shape of an HTTP version.
///
/// This list concerns accepted request versions. The version token used when Blank writes a
/// response status line is defined separately by [`HTTP_RESPONSE_VERSION`].
pub const SUPPORTED_HTTP_VERSIONS: &[&str] = &["HTTP/1.0", "HTTP/1.1"];

// HTTP content types

/// The media type used for JSON response bodies.
///
/// The value is `application/json; charset=utf-8`. It identifies the response body as JSON
/// and declares UTF-8 as its character encoding. The complete value includes both the media
/// type and its charset parameter so it can be used directly as a `Content-Type` field value.
///
/// This constant is a media-type value, not the response header name; the header name is
/// [`HTTP_RESPONSE_CONTENT_TYPE`].
pub const CONTENT_TYPE_JSON: &str = "application/json; charset=utf-8";

/// The media type used for HTML response bodies.
///
/// The value is `text/html; charset=utf-8`. It identifies the response body as HTML and
/// declares UTF-8 as its character encoding. The constant contains the complete media-type
/// field value, including its charset parameter.
///
/// This is a content-type value rather than an HTTP header name. The response header name is
/// [`HTTP_RESPONSE_CONTENT_TYPE`].
pub const CONTENT_TYPE_HTML: &str = "text/html; charset=utf-8";

/// The media type used for plain-text response bodies.
///
/// The value is `text/plain; charset=utf-8`. It identifies the response body as plain text and
/// declares UTF-8 as its character encoding. The charset parameter is part of the literal so
/// the value can be placed directly in a `Content-Type` response field.
///
/// The header name itself is defined separately by [`HTTP_RESPONSE_CONTENT_TYPE`].
pub const CONTENT_TYPE_TEXT: &str = "text/plain; charset=utf-8";

/// The media type used for binary response bodies when no more specific media type is
/// supplied.
///
/// The value is `application/octet-stream`, the general-purpose media type for arbitrary
/// binary data. Unlike the text-oriented content types in this module, it does not include a
/// character-set parameter because the body is not declared to be text.
///
/// This constant identifies the response body's media type; it is not a byte encoding or an
/// instruction to transform the response body.
pub const CONTENT_TYPE_OCTET_STREAM: &str = "application/octet-stream";

/// The media type identifying URL-encoded HTML form submissions.
///
/// The value is `application/x-www-form-urlencoded`. It identifies a form representation in
/// which field names and values are encoded into a URL-encoded sequence. The literal contains
/// only the media type and does not include a charset or other parameter.
///
/// This value is used to identify the request-body format; it is not a response header name
/// and does not contain any submitted form fields.
pub const CONTENT_TYPE_FORM_URLENCODED: &str = "application/x-www-form-urlencoded";

/// The media type identifying multipart form submissions.
///
/// The value is `multipart/form-data`. A multipart form body is composed of separate parts
/// delimited by a boundary token. The boundary parameter name is defined by
/// [`MULTIPART_BOUNDARY_PARAMETER`]; the actual boundary value is provided in the request's
/// content-type metadata.
///
/// This constant contains the media type only. It does not include the boundary parameter or
/// any of the multipart body content.
pub const CONTENT_TYPE_MULTIPART_FORM_DATA: &str = "multipart/form-data";

// HTTP response codes

/// The HTTP status-line value for a successful `200 OK` response.
///
/// The value is `200 OK`. It indicates that the request succeeded. This constant contains
/// both the three-digit status code and its reason phrase in the exact form used by Blank's
/// response construction; it is not a response-body message.
///
/// The reason phrase is descriptive text associated with the status code. The status code is
/// the protocol-level value clients use when interpreting the response.
pub const HTTP_OK: &str = "200 OK";

/// The HTTP status-line value for a successful `201 Created` response.
///
/// The value is `201 Created`. It indicates that the request succeeded and resulted in a new
/// resource being created. The constant includes the status code and reason phrase together,
/// in the form used when constructing a response status line.
///
/// This is status-line text, not a response-body string. A response body or a `Location`
/// header may be provided separately when appropriate.
pub const HTTP_CREATED: &str = "201 Created";

/// The HTTP status-line value for a successful `204 No Content` response.
///
/// The value is `204 No Content`. It indicates that the request was successfully handled and
/// that the response has no content body. The constant stores the status code and reason
/// phrase together for response construction.
///
/// This status is distinct from a `200 OK` response whose body happens to be empty: `204`
/// communicates the no-content response semantics through the status code itself.
pub const HTTP_NO_CONTENT: &str = "204 No Content";

/// The HTTP status-line value for a `301 Moved Permanently` redirect response.
///
/// The value is `301 Moved Permanently`. It indicates that the requested resource has moved
/// permanently to another location. The destination is normally communicated through the
/// [`HTTP_RESPONSE_LOCATION`] response header; the destination value is not part of this
/// constant.
///
/// This constant contains the status code and reason phrase, not the redirect URL or response
/// body.
pub const HTTP_MOVED_PERMANENTLY: &str = "301 Moved Permanently";

/// The HTTP status-line value for a `302 Found` redirect response.
///
/// The value is `302 Found`. It indicates a temporary redirection to another location. The
/// destination is normally communicated through the [`HTTP_RESPONSE_LOCATION`] response
/// header and is supplied separately from this status-line constant.
///
/// This constant contains the status code and reason phrase only; it does not contain a
/// redirect destination or define how a client follows the redirect.
pub const HTTP_FOUND: &str = "302 Found";

/// The HTTP status-line value for a `400 Bad Request` response.
///
/// The value is `400 Bad Request`. It indicates that the server cannot process the request
/// because the request is malformed or invalid. Typical causes include invalid request
/// syntax or invalid request metadata, although this constant does not enumerate parser
/// failure conditions.
///
/// The default body text associated with this response is [`BODY_BAD_REQUEST`]. This
/// constant itself is the status-line value, not the body.
pub const HTTP_BAD_REQUEST: &str = "400 Bad Request";

/// The HTTP status-line value for a `401 Unauthorized` response.
///
/// The value is `401 Unauthorized`. It indicates that the request has not been applied
/// because valid authentication credentials are required or were not supplied. The status
/// line is separate from any authentication challenge or application-specific authentication
/// behavior.
///
/// The default body text associated with this response is [`BODY_UNAUTHORIZED`].
pub const HTTP_UNAUTHORIZED: &str = "401 Unauthorized";

/// The HTTP status-line value for a `403 Forbidden` response.
///
/// The value is `403 Forbidden`. It indicates that the server understood the request but is
/// refusing to fulfill it. Unlike an authentication challenge, this status communicates that
/// access is refused; it does not itself define the application's authorization rules.
///
/// The default body text associated with this response is [`BODY_FORBIDDEN`].
pub const HTTP_FORBIDDEN: &str = "403 Forbidden";

/// The HTTP status-line value for a `404 Not Found` response.
///
/// The value is `404 Not Found`. It indicates that the requested resource could not be found
/// or that the server has no matching resource for the request target. In Blank, this status
/// is associated with a request for which no matching route or resource is available.
///
/// The default body text associated with this response is [`BODY_NOT_FOUND`].
pub const HTTP_NOT_FOUND: &str = "404 Not Found";

/// The HTTP status-line value for a `405 Method Not Allowed` response.
///
/// The value is `405 Method Not Allowed`. It indicates that the request method is known but
/// is not allowed for the target resource. This is distinct from a method that the server
/// does not recognize at all. The response may use an `Allow` field where the applicable
/// methods need to be communicated.
///
/// The default body text associated with this response is [`BODY_METHOD_NOT_ALLOWED`].
pub const HTTP_METHOD_NOT_ALLOWED: &str = "405 Method Not Allowed";

/// The HTTP status-line value for a `408 Request Timeout` response.
///
/// The value is `408 Request Timeout`. It indicates that the server did not receive a
/// complete request within the time it was prepared to wait. Blank's configured request
/// receive timeout is represented by [`HTTP_REQUEST_TIMEOUT_MS`].
///
/// The default body text associated with this response is [`BODY_REQUEST_TIMEOUT`]. This
/// constant is the status-line value, not the timeout duration.
pub const HTTP_REQUEST_TIMEOUT: &str = "408 Request Timeout";

/// The HTTP status-line value for a `413 Content Too Large` response.
///
/// The value is `413 Content Too Large`. It indicates that the request content is larger than
/// the server is willing or configured to process. Blank's maximum request-body size is
/// defined by [`MAX_HTTP_BODY_SIZE`].
///
/// The default body text associated with this response is [`BODY_CONTENT_TOO_LARGE`]. The
/// identifier retains the existing name `HTTP_PAYLOAD_TOO_LARGE` while its stored reason
/// phrase is `Content Too Large`.
pub const HTTP_PAYLOAD_TOO_LARGE: &str = "413 Content Too Large";

/// The HTTP status-line value for a `429 Too Many Requests` response.
///
/// The value is `429 Too Many Requests`. It indicates that the client has sent too many
/// requests in a given period according to a rate or request-count policy. This constant
/// defines the status-line text; it does not implement a rate limiter or set a request quota.
///
/// The default body text associated with this response is [`BODY_TOO_MANY_REQUESTS`].
pub const HTTP_TOO_MANY_REQUESTS: &str = "429 Too Many Requests";

/// The HTTP status-line value for a `431 Request Header Fields Too Large` response.
///
/// The value is `431 Request Header Fields Too Large`. It indicates that the request header
/// fields exceed the size the server is willing to process. Blank's configured maximum
/// request-header size is defined by [`MAX_HTTP_HEADER_SIZE`].
///
/// The default body text associated with this response is
/// [`BODY_REQUEST_HEADERS_TOO_LARGE`].
pub const HTTP_HEADER_FIELDS_TOO_LARGE: &str = "431 Request Header Fields Too Large";

/// The HTTP status-line value for a `500 Internal Server Error` response.
///
/// The value is `500 Internal Server Error`. It is the generic status used when the server
/// encounters an unexpected condition that prevents it from fulfilling a request. The code
/// communicates a server-side failure without exposing its underlying cause in the status
/// line.
///
/// The default body text associated with this response is [`BODY_INTERNAL_SERVER_ERROR`].
pub const HTTP_INTERNAL_SERVER_ERROR: &str = "500 Internal Server Error";

/// The HTTP status-line value for a `501 Not Implemented` response.
///
/// The value is `501 Not Implemented`. It indicates that the server does not support the
/// functionality required to fulfill the request. In Blank's method handling, this status
/// can distinguish a registered method that is not implemented from an unrecognized method.
///
/// The default generic body text associated with this response is [`BODY_NOT_IMPLEMENTED`];
/// unsupported transfer-encoding handling also has the more specific body constant
/// [`BODY_TRANSFER_ENCODING_NOT_SUPPORTED`].
pub const HTTP_NOT_IMPLEMENTED: &str = "501 Not Implemented";

/// The HTTP status-line value for a `502 Bad Gateway` response.
///
/// The value is `502 Bad Gateway`. It indicates that a server acting as a gateway or proxy
/// received an invalid response from an upstream server while attempting to fulfill a
/// request. The status-line constant does not describe a particular upstream service or
/// failure cause.
///
/// The default body text associated with this response is [`BODY_BAD_GATEWAY`].
pub const HTTP_BAD_GATEWAY: &str = "502 Bad Gateway";

/// The HTTP status-line value for a `503 Service Unavailable` response.
///
/// The value is `503 Service Unavailable`. It indicates that the server is temporarily unable
/// to handle the request, for example because of temporary overload or maintenance. The
/// constant provides the status-line text; it does not control server startup, capacity, or
/// recovery.
///
/// The default body text associated with this response is [`BODY_SERVICE_UNAVAILABLE`].
pub const HTTP_SERVICE_UNAVAILABLE: &str = "503 Service Unavailable";

/// The HTTP status-line value for a `504 Gateway Timeout` response.
///
/// The value is `504 Gateway Timeout`. It indicates that a server acting as a gateway or
/// proxy did not receive a timely response from an upstream server. It is a protocol status,
/// not a local socket timeout setting.
///
/// The default body text associated with this response is [`BODY_GATEWAY_TIMEOUT`].
pub const HTTP_GATEWAY_TIMEOUT: &str = "504 Gateway Timeout";

/// The HTTP status-line value for a `505 HTTP Version Not Supported` response.
///
/// The value is `505 HTTP Version Not Supported`. It indicates that the server does not
/// support the HTTP protocol version used in the request. The request versions currently
/// accepted by Blank are listed in [`SUPPORTED_HTTP_VERSIONS`].
///
/// The default body text associated with this response is
/// [`BODY_HTTP_VERSION_NOT_SUPPORTED`].
pub const HTTP_VERSION_NOT_SUPPORTED: &str = "505 HTTP Version Not Supported";

// Errors

/// The numeric error-code type used by Blank for its server error constants.
///
/// `BlankError` is a type alias for `u8`, so a Blank error code is stored as an unsigned
/// 8-bit integer. It is an alias rather than a Rust `enum`: the named constants below provide
/// the documented codes, while the underlying type remains an ordinary `u8`.
///
/// The currently defined codes identify failures encountered while creating, configuring,
/// or running the server:
///
/// - [`ERROR_BIND_SERVER`] identifies a failure to bind the TCP listener.
/// - [`ERROR_SET_NONBLOCKING`] is retained for compatibility with an earlier runtime and is
///   deprecated because the current implementation no longer configures the listener for
///   non-blocking polling.
/// - [`ERROR_ACCEPT_CONNECTION`] identifies an unexpected listener accept failure.
/// - [`ERROR_SPAWN_SERVER_THREAD`] identifies a failure to create the background server
///   thread.
/// - [`ERROR_SERVER_THREAD_PANIC`] identifies a panic in the background server thread.
///
/// The numeric values are declared by the associated constants. Callers can compare a
/// returned `BlankError` against those names instead of scattering numeric literals through
/// their code. Because this is a `u8` alias, the type itself does not enforce that a value is
/// one of the currently named codes.
pub type BlankError = u8;

/// Error code returned when Blank cannot bind its TCP listener to the configured host and
/// port.
///
/// The code is `1`. Binding is the operation that asks the operating system to associate the
/// server's listening socket with its configured local network address. This error identifies
/// a failure at that stage, before the server can accept incoming client connections.
///
/// Possible underlying causes are determined by the operating system and environment; this
/// constant is the stable Blank-level identifier, not the operating-system error detail. It
/// should be compared by name, for example `error == ERROR_BIND_SERVER`, rather than by
/// repeating the numeric value `1`.
pub const ERROR_BIND_SERVER: BlankError = 1;

/// Deprecated error code historically used when Blank failed to configure its listener as
/// non-blocking.
///
/// The code is `2`. It is retained for compatibility with the earlier polling runtime, but
/// the current implementation uses blocking socket readiness instead of non-blocking polling.
/// The deprecation is intentional: new code should not expect the current server startup path
/// to produce this error for listener configuration.
///
/// The existing numeric value is preserved so code that still refers to the compatibility
/// identifier can continue to compile while receiving a deprecation warning. The attribute
/// and its note are part of the existing declaration and communicate the reason this code is
/// obsolete.
#[deprecated(note = "Blank now uses blocking socket readiness instead of non-blocking polling")]
pub const ERROR_SET_NONBLOCKING: BlankError = 2;

/// Error code returned when Blank encounters an unexpected failure while accepting a client
/// connection from its TCP listener.
///
/// The code is `3`. It identifies an error in the listener's accept operation, which is the
/// step that obtains a new client connection from the listening socket. It is distinct from
/// [`ERROR_BIND_SERVER`], which identifies failure to create the listening endpoint in the
/// first place.
///
/// The constant supplies Blank's error identifier; any underlying operating-system error
/// detail is separate. Callers should compare against this named constant rather than rely on
/// the literal numeric value.
pub const ERROR_ACCEPT_CONNECTION: BlankError = 3;

/// Error code returned when Blank cannot create its background server thread.
///
/// The code is `4`. It identifies failure to spawn the thread responsible for running the
/// server's background accept-and-handle work. This is a thread-creation failure, not a
/// failure to bind the listener and not a panic that occurs after the thread has started.
///
/// The constant is the Blank-level error identifier. It does not include operating-system
/// diagnostic details about why thread creation failed; those details, when available, are
/// separate from this numeric code.
pub const ERROR_SPAWN_SERVER_THREAD: BlankError = 4;

/// Error code returned when Blank's background server thread panics.
///
/// The code is `5`. It identifies an abnormal termination caused by a panic in the server
/// thread after that thread has been created. This differs from [`ERROR_SPAWN_SERVER_THREAD`],
/// which identifies failure to create the thread at all.
///
/// The constant provides a compact error identifier for callers and diagnostics. It does
/// not contain the panic payload or a backtrace; any such diagnostic information must be
/// obtained from the relevant panic or thread-handling context.
pub const ERROR_SERVER_THREAD_PANIC: BlankError = 5;

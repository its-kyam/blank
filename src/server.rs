//! # Blank's server, request, and response implementation
//!
//! This module contains the concrete implementation behind Blank's HTTP server API.
//! It is responsible for four connected jobs: storing server configuration and route
//! registrations, accepting TCP connections, turning incoming HTTP bytes into a
//! [`BlankReq`], and turning a view's [`BlankRes`] into bytes that can be written back
//! to the client.
//!
//! ## Request-to-response lifecycle
//!
//! A request follows this path:
//!
//! 1. A [`Blank`] value owns the configured host, TCP port, and route registry.
//! 2. `serve_await` or `serve_async` binds a [`TcpListener`] to that address. The
//!    asynchronous-named entry point means that the listener runs on a background
//!    operating-system thread; it does not create a Future or require an async runtime.
//! 3. The listener accepts a TCP connection and assigns it to a bounded worker thread.
//! 4. The worker reads one HTTP request, enforcing the configured header, body, and
//!    time limits, then parses the request line, headers, URL query, and supported form
//!    fields.
//! 5. The parsed request is matched against the route registry. A matching
//!    [`BlankView`] receives an immutable request and a mutable response builder.
//! 6. The response builder is serialized with a content length, content type,
//!    connection-close header, any additional response headers, and the body bytes.
//! 7. The worker writes the response and closes the TCP connection.
//!
//! ## Concurrency model
//!
//! The listener uses blocking `accept()` rather than repeatedly checking an idle
//! socket. The operating system can therefore put the listener thread to sleep until
//! a connection arrives. Accepted connections are handled by separate worker threads,
//! subject to `MAX_ACTIVE_CONNECTIONS`; Blank does not create an unbounded thread for
//! every incoming client. Completed workers are reaped, and shutdown joins the workers
//! that are still active.
//!
//! ## Protocol and implementation boundaries
//!
//! The implementation reads a request body using `Content-Length`. A non-empty
//! `Transfer-Encoding` header is rejected as not implemented, so chunked request bodies
//! are not supported by this implementation. The response writer sends
//! `Connection: close`, and the reader handles one request per connection rather than
//! keeping a connection alive for multiple requests. The parser supports the HTTP
//! versions and methods listed in the constants module; a known method that is not in
//! the supported set is distinguished from an unknown method.
//!
//! ## Examples
//!
//! A minimal server can register a view and block in the listener loop:
//!
//! ```no_run
//! use blank_web::{Blank, BlankReq, BlankRes};
//!
//! fn home(_request: &BlankReq, response: &mut BlankRes) {
//!     response.text("Hello from Blank");
//! }
//!
//! let mut app = Blank::new();
//! app.route_func("/", home);
//! app.serve_await().expect("server failed");
//! ```
//!
//! A server can also run on a background operating-system thread and be stopped
//! through its handle:
//!
//! ```no_run
//! use blank_web::{Blank, BlankReq, BlankRes};
//!
//! fn health(_request: &BlankReq, response: &mut BlankRes) {
//!     response.ok().text("healthy");
//! }
//!
//! let mut app = Blank::new();
//! app.port(0).route_func("/health", health);
//! let server = app.serve_async().expect("could not start server");
//! println!("listening at {}", server.local_addr());
//! server.async_stop().expect("could not stop server");
//! ```
//!

use crate::constants::{
    BODY_BAD_GATEWAY, BODY_BAD_REQUEST, BODY_CONTENT_TOO_LARGE, BODY_FORBIDDEN,
    BODY_GATEWAY_TIMEOUT, BODY_HTTP_VERSION_NOT_SUPPORTED, BODY_INTERNAL_SERVER_ERROR,
    BODY_METHOD_NOT_ALLOWED, BODY_NOT_FOUND, BODY_NOT_IMPLEMENTED, BODY_REQUEST_ERROR,
    BODY_REQUEST_HEADERS_TOO_LARGE, BODY_REQUEST_TIMEOUT, BODY_SERVICE_UNAVAILABLE,
    BODY_TOO_MANY_REQUESTS, BODY_TRANSFER_ENCODING_NOT_SUPPORTED, BODY_UNAUTHORIZED, BlankError,
    CLIENT_THREAD_NAME, CONTENT_TYPE_FORM_URLENCODED, CONTENT_TYPE_HTML, CONTENT_TYPE_JSON,
    CONTENT_TYPE_MULTIPART_FORM_DATA, CONTENT_TYPE_OCTET_STREAM, CONTENT_TYPE_TEXT,
    COOKIE_CLEAR_ATTRIBUTES, COOKIE_DEFAULT_ATTRIBUTES, DEFAULT_PORT, ERROR_ACCEPT_CONNECTION,
    ERROR_BIND_SERVER, ERROR_SERVER_THREAD_PANIC, ERROR_SPAWN_SERVER_THREAD, HTTP_BAD_GATEWAY,
    HTTP_BAD_REQUEST, HTTP_BUFFER, HTTP_CONNECTION_CLOSE, HTTP_CONTENT_DISPOSITION_HEADER,
    HTTP_CONTENT_LENGTH_HEADER, HTTP_CONTENT_TYPE_HEADER, HTTP_CREATED, HTTP_FORBIDDEN, HTTP_FOUND,
    HTTP_GATEWAY_TIMEOUT, HTTP_HEADER_END, HTTP_HEADER_FIELDS_TOO_LARGE,
    HTTP_INTERNAL_SERVER_ERROR, HTTP_LINE_END, HTTP_METHOD_GET, HTTP_METHOD_NOT_ALLOWED,
    HTTP_METHOD_POST, HTTP_MOVED_PERMANENTLY, HTTP_NO_CONTENT, HTTP_NOT_FOUND,
    HTTP_NOT_IMPLEMENTED, HTTP_OK, HTTP_PAYLOAD_TOO_LARGE, HTTP_REQUEST_TIMEOUT,
    HTTP_REQUEST_TIMEOUT_MS, HTTP_RESPONSE_CONNECTION, HTTP_RESPONSE_CONTENT_LENGTH,
    HTTP_RESPONSE_CONTENT_TYPE, HTTP_RESPONSE_LOCATION, HTTP_RESPONSE_SET_COOKIE,
    HTTP_RESPONSE_VERSION, HTTP_SERVICE_UNAVAILABLE, HTTP_TOO_MANY_REQUESTS,
    HTTP_TRANSFER_ENCODING_HEADER, HTTP_UNAUTHORIZED, HTTP_VERSION_NOT_SUPPORTED,
    HTTP_WRITE_TIMEOUT_MS, LOCALHOST, MAX_ACTIVE_CONNECTIONS, MAX_FORM_FIELD_SIZE, MAX_FORM_FIELDS,
    MAX_HTTP_BODY_SIZE, MAX_HTTP_HEADER_SIZE, MULTIPART_BOUNDARY_PARAMETER,
    MULTIPART_BOUNDARY_PREFIX, MULTIPART_FILENAME_PARAMETER, MULTIPART_FINAL_MARKER,
    MULTIPART_NAME_PARAMETER, REAL_HTTP_METHODS, SERVER_THREAD_NAME, SERVER_WAKE_TIMEOUT_MS,
    SUPPORTED_HTTP_METHODS, SUPPORTED_HTTP_VERSIONS,
};

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// A TCP port number used by a Blank server.
///
/// `Port` is an alias for `u16`, so it has the same representation and accepts the
/// same values as Rust's unsigned 16-bit integer. It exists to make APIs and fields
/// that specifically represent network ports easier to identify when reading the
/// code. It does not add validation or reserve a port by itself; the operating system
/// checks whether the requested address can be bound when the server starts.
///
/// Port `0` asks the operating system to choose an available local port. With
/// [`Blank::serve_async`], the selected address can be retrieved from
/// [`BlankServer::local_addr`].
///
/// # Examples
///
/// ```text
/// use blank_web::{Blank, Port};
///
/// let mut app = Blank::new();
/// let port: Port = 7600;
/// app.port(port);
/// assert_eq!(app.get_port(), 7600);
/// ```
///
/// ```no_run
/// use blank_web::Blank;
///
/// let mut app = Blank::new();
/// app.port(0);
/// let server = app.serve_async().expect("bind failed");
/// println!("operating system selected {}", server.local_addr().port());
/// server.async_stop().expect("stop failed");
/// ```
pub type Port = u16;

/// The host or IP-address string to which a Blank listener binds.
///
/// `Host` is an alias for [`String`]; it is not a DNS lookup result, socket address,
/// or validated hostname type. The value is combined with a [`Port`] when the listener
/// is created. The default is the `LOCALHOST` constant. A caller can replace it with
/// another bindable address before starting the server. Whether an address is
/// available and valid for the current machine is ultimately determined by
/// [`TcpListener::bind`].
///
/// # Examples
///
/// ```text
/// use blank_web::{Blank, Host};
///
/// let host: Host = "127.0.0.1".to_string();
/// let mut app = Blank::new();
/// app.host(host);
/// assert_eq!(app.get_host(), "127.0.0.1");
/// ```
///
/// ```
/// use blank_web::Blank;
///
/// let mut app = Blank::new();
/// app.host("localhost").port(7600);
/// assert_eq!(app.get_host(), "localhost");
/// assert_eq!(app.get_port(), 7600);
/// ```
pub type Host = String;

/// The map representation used for parsed request headers and stored response headers.
///
/// Each map key is a header name normalized to lowercase. Each value is a `Vec<String>`
/// rather than a single string because HTTP messages can contain repeated fields with
/// the same name. The order in which values are appended is retained within that
/// vector. This type alias does not itself perform normalization or validation; those
/// rules are applied by the request parser and the `BlankRes` header methods.
///
/// # Examples
///
/// ```
/// use blank_web::BlankHeaders;
///
/// let mut headers = BlankHeaders::new();
/// headers.entry("accept".to_string()).or_default().push("text/html".into());
/// headers.entry("accept".to_string()).or_default().push("application/json".into());
/// assert_eq!(headers["accept"].len(), 2);
/// ```
///
/// ```
/// use blank_web::BlankHeaders;
///
/// let mut headers = BlankHeaders::new();
/// headers.insert("content-type".into(), vec!["application/json".into()]);
/// assert_eq!(headers.get("content-type").unwrap()[0], "application/json");
/// ```
pub type BlankHeaders = HashMap<String, Vec<String>>;

/// The map representation used for parsed URL query parameters.
///
/// A query key maps to all decoded values supplied for that key. Repeated parameters
/// are not collapsed: a request target such as `/search?tag=rust&tag=web` produces one
/// `tag` entry with two values. Values are URL-decoded before being stored, including
/// conversion of `+` to a space and decoding of valid percent-encoded bytes. The map
/// uses the decoded parameter name as its key.
///
/// # Examples
///
/// ```text
/// use blank_web::BlankQuery;
///
/// let mut query = BlankQuery::new();
/// query.entry("tag".into()).or_default().push("rust".into());
/// query.entry("tag".into()).or_default().push("web".into());
/// assert_eq!(query["tag"], vec!["rust", "web"]);
/// ```
///
/// ```text
/// use blank_web::BlankQuery;
///
/// let mut query = BlankQuery::new();
/// query.insert("page".into(), vec!["2".into()]);
/// assert_eq!(query.get("page").unwrap().first().map(String::as_str), Some("2"));
/// ```
pub type BlankQuery = HashMap<String, Vec<String>>;

/// The map representation used for parsed HTML form fields.
///
/// Form field names map to vectors so that repeated names can be represented without
/// discarding values. URL-encoded forms and text parts from multipart forms populate
/// this map. Multipart file parts are intentionally skipped by the current parser, so
/// their file contents do not appear in `BlankForms`. The parser also applies limits to
/// the number of distinct fields and the byte length of each field value.
///
/// # Examples
///
/// ```text
/// use blank_web::BlankForms;
///
/// let mut forms = BlankForms::new();
/// forms.entry("color".into()).or_default().push("blue".into());
/// forms.entry("color".into()).or_default().push("green".into());
/// assert_eq!(forms["color"].len(), 2);
/// ```
///
/// ```text
/// use blank_web::BlankForms;
///
/// let mut forms = BlankForms::new();
/// forms.insert("username".into(), vec!["kyam".into()]);
/// assert_eq!(forms["username"][0], "kyam");
/// ```
pub type BlankForms = HashMap<String, Vec<String>>;

/// The parsed HTTP request passed to a registered [`BlankView`].
///
/// A `BlankReq` is created by the HTTP parser for a single accepted connection. The
/// request is then borrowed immutably by the view function; views inspect it through
/// accessors instead of changing the parsed request. Its fields are private so that
/// callers cannot accidentally make the stored route disagree with the parsed target,
/// or change the relationship between raw bytes and decoded query/form data.
///
/// The request keeps both the original body bytes and the parsed representations that
/// are useful for common web handlers:
///
/// - `method`, `route`, and `version` describe the request line;
/// - `headers` stores normalized header names and all values for repeated headers;
/// - `body` stores the request payload exactly as bytes;
/// - `query` stores decoded URL query parameters;
/// - `forms` stores supported URL-encoded and multipart text fields.
///
/// Header lookup is case-insensitive because names are normalized to lowercase during
/// parsing. Query and form field names are looked up using the decoded key as stored;
/// this implementation does not case-fold those names. Repeated values are available
/// through the `*_values` accessors, while singular accessors return the first value.
///
/// A query string is not part of `route()`. For example, `/items?id=7` is routed using
/// `/items`, while `id=7` is available through `query_value("id")`. The raw body is
/// independent of the form parser: callers can always inspect bytes with `body()` and
/// can request UTF-8 interpretation with `body_text()`.
///
/// # Examples
///
/// The request is normally received as the first argument of a view:
///
/// ```
/// use blank_web::{BlankReq, BlankRes};
///
/// fn inspect(request: &BlankReq, response: &mut BlankRes) {
///     let method = request.method();
///     let path = request.route();
///     response.text(format!("{method} {path}"));
/// }
/// ```
///
/// Query parameters and headers can be read without parsing the raw request again:
///
/// ```
/// use blank_web::{BlankReq, BlankRes};
///
/// fn search(request: &BlankReq, response: &mut BlankRes) {
///     let term = request.query_value("q").unwrap_or("");
///     let preferred = request.header("accept").unwrap_or("not supplied");
///     response.text(format!("q={term}; accept={preferred}"));
/// }
/// ```
///
/// For payloads that may not be UTF-8, use `body()` rather than assuming `body_text()`
/// will succeed. A UTF-8 conversion failure is reported as `std::str::Utf8Error` and
/// does not mutate or discard the original bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlankReq {
    /// The method token from the request line, stored as received after whitespace
    /// separation. Method support is checked by the parser before a request is passed
    /// to a view. Use [`BlankReq::method`] for read-only access and [`BlankReq::is_get`]
    /// or [`BlankReq::is_post`] for the convenience checks provided here.
    method: String,

    /// The origin-form request path used for exact route lookup, without its query
    /// string. A request target `/users/4?expand=true` therefore stores `/users/4` here.
    /// The registered route is matched against this path; query parameters are exposed
    /// separately through [`BlankReq::query`] and its lookup methods.
    route: String,

    /// The HTTP version token from the request line. The parser validates this token
    /// against the supported-version list before constructing the request. It remains
    /// available for handlers that need to inspect which supported version the client
    /// used.
    version: String,

    /// All parsed request headers. Names are normalized to lowercase and each name
    /// maps to a vector so that repeated header fields remain accessible. Prefer
    /// [`BlankReq::header`] for the first value or [`BlankReq::header_values`] when
    /// repeated values matter.
    headers: BlankHeaders,

    /// The request body as received, limited by the configured maximum body size and
    /// the parsed `Content-Length`. It remains a byte vector because HTTP payloads are
    /// not necessarily text. Use [`BlankReq::body_text`] only when UTF-8 is appropriate.
    body: Vec<u8>,

    /// Decoded query parameters extracted from the request target. Repeated keys keep
    /// all values in insertion order. Query parsing errors prevent the request from
    /// reaching a view.
    query: BlankQuery,

    /// Parsed form fields for the content types recognized by the request parser.
    /// URL-encoded fields and multipart text fields are included; multipart file parts
    /// are skipped by the current implementation.
    forms: BlankForms,
}

/// Provides read-only map access to a request's headers.
///
/// Dereferencing `BlankReq` yields `&BlankHeaders`, not a mutable reference. This is a
/// convenience for code that wants to use ordinary map operations on the complete
/// header collection. The same lowercase-key convention and vector-of-values
/// representation used by [`BlankReq::headers`] apply.
///
/// # Examples
///
/// ```
/// use blank_web::BlankReq;
///
/// fn read_accept(request: &BlankReq) -> Option<&String> {
///     request.get("accept").and_then(|values| values.first())
/// }
/// ```
///
/// For most handlers, the named accessors are more direct:
///
/// ```
/// use blank_web::BlankReq;
///
/// fn read_host(request: &BlankReq) -> Option<&str> {
///     request.header("Host")
/// }
/// ```
impl Deref for BlankReq {
    type Target = BlankHeaders;

    /// Borrows the request's header map for standard map-style lookup.
    ///
    /// The returned reference is immutable and points to the same lowercase-keyed map
    /// exposed by [`BlankReq::headers`]. This implementation exists for convenience;
    /// it does not create a copy or provide mutable access to request headers.
    ///
    /// Examples: `request.get("accept")` performs an ordinary map lookup through
    /// deref coercion, while `request.header("Accept")` uses the dedicated
    /// case-insensitive accessor.
    fn deref(&self) -> &Self::Target {
        &self.headers
    }
}

impl BlankReq {
    /// Returns the HTTP method token for this request.
    ///
    /// The returned string is borrowed from the request and remains valid for as long
    /// as the request is borrowed. The parser checks method support before a view is
    /// invoked, but this accessor reports the method string rather than converting it
    /// to an enum. Use this when a handler needs to display, log, or branch on the
    /// original method token.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{BlankReq, BlankRes};
    /// fn endpoint(request: &BlankReq, response: &mut BlankRes) {
    ///     response.text(format!("method: {}", request.method()));
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn is_read(request: &BlankReq) -> bool {
    ///     request.method() == "GET"
    /// }
    /// ```
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Returns the path used to select the registered view, excluding the query string.
    ///
    /// Route lookup is an exact string lookup after registration-time normalization
    /// adds a leading slash when needed. This accessor does not return a full URL and
    /// does not include `?key=value` parameters. Use [`BlankReq::query`] or
    /// [`BlankReq::query_value`] to inspect those parameters.
    ///
    /// # Examples
    ///
    /// For `/products/8?currency=INR`, this method returns `/products/8`.
    ///
    /// ```
    /// use blank_web::{BlankReq, BlankRes};
    /// fn product(request: &BlankReq, response: &mut BlankRes) {
    ///     response.text(request.route());
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn is_root(request: &BlankReq) -> bool {
    ///     request.route() == "/"
    /// }
    /// ```
    pub fn route(&self) -> &str {
        &self.route
    }

    /// Returns the HTTP version token accepted by the request parser.
    ///
    /// The parser rejects versions outside the supported list before it creates a
    /// `BlankReq`. This method is useful for diagnostics or protocol-sensitive handler
    /// logic; most application views do not need to branch on it.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{BlankReq, BlankRes};
    /// fn show_version(request: &BlankReq, response: &mut BlankRes) {
    ///     response.text(request.version());
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn is_http_11(request: &BlankReq) -> bool {
    ///     request.version() == "HTTP/1.1"
    /// }
    /// ```
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns the complete parsed header map.
    ///
    /// Header names are stored in lowercase, and a name maps to a vector of values.
    /// This method exposes the map immutably; callers can inspect or iterate over it but
    /// cannot alter the request. It is useful when a handler needs to inspect headers
    /// that do not have a dedicated accessor.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn header_count(request: &BlankReq) -> usize {
    ///     request.headers().len()
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn all_header_names(request: &BlankReq) -> Vec<&str> {
    ///     request.headers().keys().map(String::as_str).collect()
    /// }
    /// ```
    pub fn headers(&self) -> &BlankHeaders {
        &self.headers
    }

    /// Returns the first value associated with a request header name.
    ///
    /// `name` is converted to ASCII lowercase before the map lookup, so `Host`, `HOST`,
    /// and `host` refer to the same stored key. When the request contains repeated
    /// values for a header, this method returns only the first one. It returns `None`
    /// when that header was not present. The returned value is borrowed and is not
    /// copied or modified.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{BlankReq, BlankRes};
    /// fn content_type(request: &BlankReq, response: &mut BlankRes) {
    ///     response.text(request.header("Content-Type").unwrap_or("missing"));
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn has_json_body(request: &BlankReq) -> bool {
    ///     request.header("content-type").unwrap_or("").starts_with("application/json")
    /// }
    /// ```
    ///
    /// Use [`BlankReq::header_values`] when every repeated value is significant.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    /// Returns all values associated with a request header name.
    ///
    /// The name lookup is case-insensitive because the supplied name is normalized to
    /// lowercase. `Some` contains a borrowed slice of every value stored for that
    /// header; `None` means the header is absent. A present header has at least one
    /// value, because values are inserted as they are parsed.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn accepted_types(request: &BlankReq) -> &[String] {
    ///     request.header_values("Accept").unwrap_or(&[])
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn has_multiple_cookie_fields(request: &BlankReq) -> bool {
    ///     request.header_values("cookie").map_or(false, |values| values.len() > 1)
    /// }
    /// ```
    pub fn header_values(&self, name: &str) -> Option<&[String]> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(Vec::as_slice)
    }

    /// Returns the complete request body as a borrowed byte slice.
    ///
    /// The bytes are retained independently of the parsed query and form maps. No
    /// character encoding conversion is performed, so this is the appropriate accessor
    /// for arbitrary binary payloads or for parsers that need the original bytes. The
    /// returned slice is empty when the request has nobody.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{BlankReq, BlankRes};
    /// fn payload_size(request: &BlankReq, response: &mut BlankRes) {
    ///     response.text(request.body().len().to_string());
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn has_payload(request: &BlankReq) -> bool {
    ///     !request.body().is_empty()
    /// }
    /// ```
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Interprets the request body as UTF-8 and returns it as text.
    ///
    /// This does not allocate a new `String`; on success it returns a string slice that
    /// borrows the original body. It can fail when any body byte sequence is not valid
    /// UTF-8. Callers should handle that error rather than assuming all HTTP request
    /// bodies are textual. For opaque or binary data, use [`BlankReq::body`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{BlankReq, BlankRes};
    /// fn echo_text(request: &BlankReq, response: &mut BlankRes) {
    ///     match request.body_text() {
    ///         Ok(text) => { response.text(text); }
    ///         Err(_) => { response.bad_request(); }
    ///     }
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn text_length(request: &BlankReq) -> Result<usize, std::str::Utf8Error> {
    ///     Ok(request.body_text()?.len())
    /// }
    /// ```
    pub fn body_text(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.body)
    }

    /// Returns the first `Content-Type` request header, if one was supplied.
    ///
    /// The lookup uses the same case-insensitive header handling as
    /// [`BlankReq::header`]. The returned value is the header value as parsed, including
    /// any parameters such as a multipart boundary or a charset declaration. This
    /// accessor reports the declared type; it does not infer a type from the body bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{BlankReq, BlankRes};
    /// fn describe_body(request: &BlankReq, response: &mut BlankRes) {
    ///     response.text(request.content_type().unwrap_or("no content type"));
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn is_form(request: &BlankReq) -> bool {
    ///     request.content_type().unwrap_or("").starts_with("application/x-www-form-urlencoded")
    /// }
    /// ```
    pub fn content_type(&self) -> Option<&str> {
        self.header(HTTP_CONTENT_TYPE_HEADER)
    }

    /// Returns the complete parsed query-parameter map.
    ///
    /// The map contains decoded names and values. Multiple occurrences of the same
    /// parameter name are retained in a vector instead of being overwritten. A request
    /// without a query string has an empty map. This accessor exposes the collection
    /// immutably; use [`BlankReq::query_value`] for the common first-value lookup.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn query_key_count(request: &BlankReq) -> usize {
    ///     request.query().len()
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn has_filter(request: &BlankReq) -> bool {
    ///     request.query().contains_key("filter")
    /// }
    /// ```
    pub fn query(&self) -> &BlankQuery {
        &self.query
    }

    /// Returns the first decoded value for a query parameter.
    ///
    /// Parameter names are matched exactly after URL decoding; unlike header names,
    /// query names are not case-insensitive. If a key occurs more than once, only its
    /// first stored value is returned. The result is `None` when the parameter is not
    /// present. Use [`BlankReq::query_values`] to retrieve the entire repeated-value
    /// slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{BlankReq, BlankRes};
    /// fn greeting(request: &BlankReq, response: &mut BlankRes) {
    ///     let name = request.query_value("name").unwrap_or("visitor");
    ///     response.text(format!("Hello, {name}"));
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn page_number(request: &BlankReq) -> usize {
    ///     request.query_value("page").and_then(|value| value.parse().ok()).unwrap_or(1)
    /// }
    /// ```
    pub fn query_value(&self, name: &str) -> Option<&str> {
        self.query
            .get(name)
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    /// Returns every decoded value for one query parameter name.
    ///
    /// This method is useful for repeated query parameters such as
    /// `?tag=rust&tag=networking`. It returns a borrowed slice containing all values, or
    /// `None` if the parameter name does not occur. The values are already URL-decoded
    /// by the request parser.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn tags(request: &BlankReq) -> &[String] {
    ///     request.query_values("tag").unwrap_or(&[])
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn has_tag(request: &BlankReq, wanted: &str) -> bool {
    ///     request.query_values("tag").is_some_and(|values| values.iter().any(|v| v == wanted))
    /// }
    /// ```
    pub fn query_values(&self, name: &str) -> Option<&[String]> {
        self.query.get(name).map(Vec::as_slice)
    }

    /// Returns the complete map of parsed form fields.
    ///
    /// The parser populates this map only for the supported form media types:
    /// `application/x-www-form-urlencoded` and `multipart/form-data`. Multipart text
    /// fields are included, while multipart file parts are skipped. Repeated field names
    /// retain their values in a vector. Other content types leave this map empty, even
    /// though the raw request body remains available through [`BlankReq::body`].
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn field_count(request: &BlankReq) -> usize {
    ///     request.forms().len()
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn contains_email(request: &BlankReq) -> bool {
    ///     request.forms().contains_key("email")
    /// }
    /// ```
    pub fn forms(&self) -> &BlankForms {
        &self.forms
    }

    /// Returns the first parsed value for a form field name.
    ///
    /// Field names are compared exactly; the lookup is not case-insensitive. If the
    /// same field appears multiple times, this method returns its first value. It
    /// returns `None` when the field is absent. Use [`BlankReq::form_values`] when a
    /// repeated field is expected, such as a group of checkboxes with the same name.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{BlankReq, BlankRes};
    /// fn submit(request: &BlankReq, response: &mut BlankRes) {
    ///     let username = request.form("username").unwrap_or("anonymous");
    ///     response.text(format!("received {username}"));
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn remember_me(request: &BlankReq) -> bool {
    ///     request.form("remember").is_some_and(|value| value == "yes")
    /// }
    /// ```
    pub fn form(&self, name: &str) -> Option<&str> {
        self.forms
            .get(name)
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    /// Returns all parsed values associated with one form field name.
    ///
    /// The returned slice preserves the order in which matching fields were parsed.
    /// A missing field produces `None`; a present field produces `Some` with one or
    /// more values. This is the appropriate accessor for repeated form controls that
    /// share a name.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn selected_options(request: &BlankReq) -> &[String] {
    ///     request.form_values("option").unwrap_or(&[])
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn chose_anything(request: &BlankReq) -> bool {
    ///     request.form_values("option").is_some_and(|values| !values.is_empty())
    /// }
    /// ```
    pub fn form_values(&self, name: &str) -> Option<&[String]> {
        self.forms.get(name).map(Vec::as_slice)
    }

    /// Reports whether the parsed method token is exactly `POST`.
    ///
    /// This is a convenience comparison against the POST method constant. It does not
    /// change method support and does not make a POST request routable by itself; the
    /// parser still applies the configured supported-method list before calling a view.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{BlankReq, BlankRes};
    /// fn post_only_logic(request: &BlankReq, response: &mut BlankRes) {
    ///     if request.is_post() { response.text("POST"); }
    ///     else { response.method_not_allowed(); }
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn is_not_a_post(request: &BlankReq) -> bool {
    ///     !request.is_post()
    /// }
    /// ```
    pub fn is_post(&self) -> bool {
        self.method == HTTP_METHOD_POST
    }

    /// Reports whether the parsed method token is exactly `GET`.
    ///
    /// This method is a convenience comparison; it does not inspect the request body or
    /// route and does not perform any additional validation. A request reaches a view
    /// only after the parser has accepted its method according to the supported-method
    /// constants.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{BlankReq, BlankRes};
    /// fn read_endpoint(request: &BlankReq, response: &mut BlankRes) {
    ///     if request.is_get() { response.text("read request"); }
    ///     else { response.method_not_allowed(); }
    /// }
    /// ```
    ///
    /// ```
    /// use blank_web::BlankReq;
    /// fn should_read(request: &BlankReq) -> bool {
    ///     request.is_get() && request.query_value("id").is_some()
    /// }
    /// ```
    pub fn is_get(&self) -> bool {
        self.method == HTTP_METHOD_GET
    }
}

/// The function signature accepted by [`Blank::route_func`].
///
/// A view receives a shared reference to the parsed request and a mutable reference to
/// a response builder. The request can be inspected but not modified; the response is
/// the place where the handler chooses a status, body, content type, headers, cookies,
/// or redirect. A view is a plain function pointer (`fn`), not a capturing closure or a
/// dynamically allocated trait object. State needed by a view must therefore be
/// available through other program-level mechanisms or represented by a plain function
/// that can access it.
///
/// The view does not write to the TCP stream. After the function returns, Blank
/// serializes the response and writes it on the connection worker.
///
/// # Examples
///
/// ```
/// use blank_web::{Blank, BlankReq, BlankRes, BlankView};
///
/// fn home(_request: &BlankReq, response: &mut BlankRes) {
///     response.html("<h1>Home</h1>");
/// }
///
/// let view: BlankView = home;
/// let mut app = Blank::new();
/// app.route_func("/", view);
/// ```
///
/// ```
/// use blank_web::{BlankReq, BlankRes};
/// fn api_status(_request: &BlankReq, response: &mut BlankRes) {
///     response.status("200 OK").json(r#"{"status":"ready"}"#);
/// }
/// ```
pub type BlankView = fn(rq: &BlankReq, out: &mut BlankRes);

// Each output type stores a hardcoded string for now.
// Generates one small, owned response-body wrapper for a fixed media type.
//
// The macro is deliberately private: it is an implementation tool, not a macro
// exposed to applications. For each invocation it emits a public struct containing a
// `String`, a constructor accepting anything convertible into `String`, a borrowed
// `content()` accessor, and an associated `CONTENT_TYPE` constant. Keeping the three
// textual output wrappers structurally identical avoids repeating their constructor
// and accessor implementations while allowing `BlankBody` to retain the selected
// content type through its enum variant.
//
// The invocations below create `BlankJSON`, `BlankHTML`, and `BlankText`. They differ
// only in the content type constant supplied to the macro. These wrappers own text;
// binary response data is represented separately by `BlankBody::Binary(Vec<u8>)`.
//
// Conceptual examples of the generated API:
// - `BlankJSON::new(r#"{"ok":true}"#)` stores JSON text and reports the JSON media type.
// - `BlankHTML::new("<h1>Hi</h1>")` stores markup and reports the HTML media type.
// - `BlankText::new("hello")` stores ordinary text and reports the plain-text media type.
macro_rules! blank_output {
    ($name:ident, $content_type:expr) => {
        /// An owned textual response body generated by Blank's internal output-type macro.
        ///
        /// Each generated wrapper stores its response content as a `String` and
        /// associates that content with a fixed media type through `CONTENT_TYPE`.
        /// The wrapper does not serialize arbitrary Rust values and does not validate
        /// that the string is syntactically valid for its media type. For example,
        /// the JSON wrapper expects its caller to supply JSON text; it does not turn a
        /// struct into JSON. The HTML wrapper likewise stores markup as supplied.
        ///
        /// The constructor accepts `impl Into<String>`, so both string slices and
        /// owned strings can be used. `content()` returns a borrowed view of the
        /// stored text. The wrapper is used internally by `BlankBody` to distinguish
        /// JSON, HTML, and plain-text responses while keeping the body owned.
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            // This owned String stores the textual payload exactly as supplied. The
            // generated wrapper selects its default Content-Type; the payload is not
            // parsed or validated by this storage type.
            content: String,
        }

        impl $name {
            /// Creates a wrapper containing the supplied textual response content.
            ///
            /// The argument is converted into an owned `String` and stored without
            /// parsing, escaping, validation, or serialization. The wrapper's type and
            /// associated `CONTENT_TYPE` constant determine the default media type; the
            /// constructor does not infer a type from the supplied text.
            ///
            /// Examples of the generated constructors are `BlankJSON::new("{}")`,
            /// `BlankHTML::new("<p>text</p>")`, and `BlankText::new("text")`.
            pub fn new(content: impl Into<String>) -> Self {
                Self {
                    content: content.into(),
                }
            }

            /// Borrows the exact text stored in this output wrapper.
            ///
            /// The returned string slice is tied to the borrow of the wrapper and does
            /// not allocate a new string. It contains the value supplied to `new`; no
            /// formatting, JSON serialization, HTML escaping, or content conversion is
            /// applied by this accessor.
            ///
            /// For example, a wrapper created with `BlankText::new("hello")` returns
            /// `"hello"`, while a JSON wrapper returns the JSON text supplied by its caller.
            pub fn content(&self) -> &str {
                &self.content
            }

            /// The default media type associated with this generated wrapper.
            ///
            /// The macro invocation supplies this constant. `BlankJSON`, `BlankHTML`,
            /// and `BlankText` therefore expose different media types even though their
            /// storage and methods have the same shape. The response serializer uses
            /// the associated default unless `BlankRes::content_type` supplies an
            /// explicit override.
            pub const CONTENT_TYPE: &'static str = $content_type;
        }
    };
}

blank_output!(BlankJSON, CONTENT_TYPE_JSON);
blank_output!(BlankHTML, CONTENT_TYPE_HTML);
blank_output!(BlankText, CONTENT_TYPE_TEXT);

/// Internal representation of the body selected for a [`BlankRes`].
///
/// The enum keeps the body representation and its default media type together. Textual
/// variants wrap their content in a dedicated output type, allowing JSON, HTML, and
/// plain text to retain distinct default content types. The binary variant stores raw
/// bytes and defaults to `application/octet-stream`.
///
/// This type is private because callers configure response bodies through
/// [`BlankRes::json`], [`BlankRes::html`], [`BlankRes::text`], and [`BlankRes::binary`];
/// they do not need to construct or match this enum directly. The methods below are the
/// common internal interface used by the response serializer.
///
/// # Representation examples
///
/// - A call to `response.json(r#"{"ok":true}"#)` selects `BlankBody::Json`.
/// - A call to `response.html("<p>hello</p>")` selects `BlankBody::Html`.
/// - A call to `response.text("hello")` selects `BlankBody::Text`.
/// - A call to `response.binary(vec![0, 1, 2])` selects `BlankBody::Binary`.
///
/// These are descriptions of the internal variants; application code should use the
/// `BlankRes` methods rather than the private enum directly.
#[derive(Debug, Clone, PartialEq, Eq)]
enum BlankBody {
    /// Text stored as JSON, using the JSON wrapper's default media type.
    Json(BlankJSON),
    /// Markup stored as HTML, using the HTML wrapper's default media type.
    Html(BlankHTML),
    /// Ordinary UTF-8 text, using the plain-text wrapper's default media type.
    Text(BlankText),
    /// Arbitrary bytes, using `application/octet-stream` unless overridden on `BlankRes`.
    Binary(Vec<u8>),
}

impl BlankBody {
    /// Returns the selected body as bytes for HTTP serialization.
    ///
    /// Textual variants expose the UTF-8 bytes of their stored `String`; the binary
    /// variant already contains bytes and is returned without an encoding conversion.
    /// The returned slice borrows the enum's stored content, so serialization can write
    /// it without making another body copy. This method intentionally hides which
    /// variant is active from the response writer.
    ///
    /// Internal examples:
    /// - `Text("hello")` yields the bytes for `hello`.
    /// - `Binary(vec![0, 255])` yields those exact two bytes, including the non-text byte.
    fn content(&self) -> &[u8] {
        match self {
            Self::Json(content) => content.content().as_bytes(),
            Self::Html(content) => content.content().as_bytes(),
            Self::Text(content) => content.content().as_bytes(),
            Self::Binary(content) => content,
        }
    }

    /// Returns the default media type implied by the active body variant.
    ///
    /// JSON, HTML, and text variants use the constants attached to their corresponding
    /// generated output wrappers. Binary data uses the octet-stream constant. A
    /// `BlankRes` may separately carry a custom content type; in that case
    /// [`BlankRes::response_content_type`] selects the override instead of this default.
    ///
    /// Internal examples:
    /// - A JSON body defaults to the JSON media type.
    /// - A binary body defaults to `application/octet-stream`.
    /// - A later response-level content-type override takes precedence at serialization.
    fn content_type(&self) -> &'static str {
        match self {
            Self::Json(_) => BlankJSON::CONTENT_TYPE,
            Self::Html(_) => BlankHTML::CONTENT_TYPE,
            Self::Text(_) => BlankText::CONTENT_TYPE,
            Self::Binary(_) => CONTENT_TYPE_OCTET_STREAM,
        }
    }
}

/// A mutable builder for the HTTP response returned by a [`BlankView`].
///
/// A view receives `&mut BlankRes` and configures the response without writing directly
/// to the network. The builder stores four pieces of response state: a status string,
/// a body representation, additional headers, and an optional content-type override.
/// Once the view returns, the server serializes that state into an HTTP response.
///
/// A newly created response has status `200 OK`, an empty plain-text body, and no
/// additional headers. Calling a body setter replaces the previous body. `json`,
/// `html`, and `text` select their corresponding default media types; `binary` selects
/// octet-stream. Calling `content_type` overrides the media type used during output.
/// The response methods return `&mut Self`, so configuration calls can be chained.
///
/// Header helpers normalize accepted names to lowercase. `header` appends a value,
/// while `set_header` replaces all values for that name. Invalid header names or values
/// are ignored rather than added. Cookie helpers use the same response-header storage
/// and apply the default cookie attributes documented below.
///
/// # Examples
///
/// A simple textual response:
///
/// ```
/// use blank_web::{BlankReq, BlankRes};
/// fn hello(_request: &BlankReq, response: &mut BlankRes) {
///     response.ok().text("Hello!");
/// }
/// ```
///
/// A JSON response with a custom header and an explicit status:
///
/// ```
/// use blank_web::{BlankReq, BlankRes};
/// fn api(_request: &BlankReq, response: &mut BlankRes) {
///     response.status("201 Created")
///         .json(r#"{"created":true}"#)
///         .header("X-API-Version", "1");
/// }
/// ```
///
/// A binary response can be supplied as a byte vector:
///
/// ```
/// use blank_web::{BlankReq, BlankRes};
/// fn download(_request: &BlankReq, response: &mut BlankRes) {
///     response.binary(vec![0x42, 0x4c, 0x41, 0x4e, 0x4b]);
/// }
/// ```
///
/// The `json` method accepts JSON text, not a Rust value to be serialized. Serialize a
/// value before passing its resulting string to this builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlankRes {
    /// Status-line value retained for serialization. The builder does not parse this
    /// string, so callers of `status` are responsible for supplying a valid value.
    status: &'static str,

    /// Owned response payload and its default media type. Text and binary variants
    /// remain distinct so the serializer can write arbitrary bytes without text conversion.
    body: BlankBody,

    /// Additional application-configured response headers. Keys are lowercase and
    /// repeated values are stored in vectors; standard framing headers are generated
    /// separately by the response serializer.
    headers: BlankHeaders,

    /// Optional media-type override. When absent, the active body variant supplies
    /// the default; a valid `content_type` call sets this override.
    content_type: Option<String>,
}

impl BlankRes {
    /// Creates an empty response builder with the default response state.
    ///
    /// The initial status is `200 OK`, the body is an empty plain-text body, the header
    /// map is empty, and there is no custom content-type override. A view normally
    /// receives a builder that the server has already created, but `new()` is available
    /// when application code needs to prepare a standalone response value.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let response = BlankRes::new();
    /// assert_eq!(response.body(), b"");
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.text("ready");
    /// assert_eq!(response.body(), b"ready");
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the response body with the supplied JSON text.
    ///
    /// This method stores the string as JSON output and clears any previous custom
    /// content-type override so the JSON media type becomes active. It does not serialize
    /// a Rust value and does not validate the supplied string as JSON. The caller must
    /// provide syntactically valid JSON when a valid JSON response is required. Use a
    /// serializer such as `serde_json` in application code, then pass its resulting text
    /// to this method.
    ///
    /// Because this replaces the body, any earlier text, HTML, or binary content is
    /// discarded. Existing status and response headers are preserved.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.json(r#"{"message":"hello"}"#);
    /// assert_eq!(response.body(), br#"{"message":"hello"}"#);
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// let serialized = format!(r#"{{"count":{}}}"#, 3);
    /// response.status("200 OK").json(serialized);
    /// assert!(response.response_content_type().starts_with("application/json"));
    /// ```
    pub fn json(&mut self, content: impl Into<String>) -> &mut Self {
        self.body = BlankBody::Json(BlankJSON::new(content));
        self.content_type = None;
        self
    }

    /// Replaces the response body with the supplied HTML text.
    ///
    /// The supplied content is stored as-is; Blank does not escape it, sanitize it, or
    /// validate that it is well-formed HTML. The method selects the HTML media type by
    /// default and clears any custom content-type override that was previously set.
    /// The current status and additional headers remain unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.html("<!doctype html><h1>Welcome</h1>");
    /// assert!(response.response_content_type().starts_with("text/html"));
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.status("200 OK").html("<main>Dashboard</main>");
    /// assert!(response.body().starts_with(b"<main>"));
    /// ```
    pub fn html(&mut self, content: impl Into<String>) -> &mut Self {
        self.body = BlankBody::Html(BlankHTML::new(content));
        self.content_type = None;
        self
    }

    /// Replaces the response body with plain UTF-8 text.
    ///
    /// The content is stored exactly as the supplied string. This method selects the
    /// default plain-text media type and clears a previously selected custom content
    /// type. It does not modify the response status or other headers. Use `binary` when
    /// the payload is arbitrary bytes rather than UTF-8 text.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.text("The service is ready");
    /// assert_eq!(response.body(), b"The service is ready");
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.status("404 Not Found").text("No such page");
    /// assert_eq!(response.body(), b"No such page");
    /// ```
    pub fn text(&mut self, content: impl Into<String>) -> &mut Self {
        self.body = BlankBody::Text(BlankText::new(content));
        self.content_type = None;
        self
    }

    /// Replaces the response body with arbitrary bytes.
    ///
    /// Unlike the textual body methods, this method does not interpret or convert the
    /// payload as UTF-8. The bytes are retained exactly and the response content type is
    /// set to `application/octet-stream`. The status and additional response headers are
    /// preserved. A later call to `content_type` can override the default media type.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.binary(vec![0, 1, 2, 255]);
    /// assert_eq!(response.body(), &[0, 1, 2, 255]);
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// let bytes: Vec<u8> = b"FILE".to_vec();
    /// response.binary(bytes).header("X-Download", "yes");
    /// assert_eq!(response.body(), b"FILE");
    /// ```
    pub fn binary(&mut self, content: impl Into<Vec<u8>>) -> &mut Self {
        self.body = BlankBody::Binary(content.into());
        self.content_type = Some(CONTENT_TYPE_OCTET_STREAM.to_string());
        self
    }

    /// Replaces the HTTP status line value used when the response is serialized.
    ///
    /// The argument is a static string so the response can retain it without allocating
    /// or owning a dynamic status value. The method does not parse or validate the
    /// string; callers should supply a valid HTTP status representation, including the
    /// numeric code and reason phrase expected by this implementation. Setting a status
    /// does not change the body or headers.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.status("202 Accepted").text("queued");
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// const CUSTOM_STATUS: &str = "418 I'm a teapot";
    /// let mut response = BlankRes::new();
    /// response.status(CUSTOM_STATUS);
    /// ```
    pub fn status(&mut self, status: &'static str) -> &mut Self {
        self.status = status;
        self
    }

    /// Sets the response status to `200 OK` without changing the body or headers.
    ///
    /// This is a named convenience wrapper around [`BlankRes::status`]. It is useful
    /// when a handler wants to make the successful status explicit while continuing to
    /// configure the response through the same chain of mutable builder methods.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.ok().text("done");
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.ok().json(r#"{"ok":true}"#).header("X-Result", "success");
    /// ```
    pub fn ok(&mut self) -> &mut Self {
        self.status(HTTP_OK)
    }

    /// Sets the response status to `201 Created` without changing the body or headers.
    ///
    /// Use this for a successful operation that created a resource. The method only
    /// sets the status; it does not create a resource, construct a `Location` header, or
    /// choose the response body. Those remain the responsibility of the view.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.created().json(r#"{"id":42}"#);
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.created().header("Location", "/items/42").text("created");
    /// ```
    pub fn created(&mut self) -> &mut Self {
        self.status(HTTP_CREATED)
    }

    /// Sets `204 No Content` and replaces the current body with an empty text body.
    ///
    /// The helper changes both status and body. It does not remove previously added
    /// response headers. The resulting body is empty; the serializer still constructs
    /// its normal response headers, including a content length and the selected
    /// content type. Use this when the operation succeeded and the client does not need
    /// a response representation.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.no_content();
    /// assert!(response.body().is_empty());
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Operation", "deleted").no_content();
    /// assert!(response.body().is_empty());
    /// ```
    pub fn no_content(&mut self) -> &mut Self {
        self.status(HTTP_NO_CONTENT).text("")
    }

    /// Sets `400 Bad Request` and the framework's standard bad-request text body.
    ///
    /// Use this when the handler cannot process the request because its application-level
    /// input is invalid. This helper replaces the current body with the predefined
    /// error text but preserves additional headers that were already added.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.bad_request();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Validation", "failed").bad_request();
    /// ```
    pub fn bad_request(&mut self) -> &mut Self {
        self.status(HTTP_BAD_REQUEST).text(BODY_BAD_REQUEST)
    }

    /// Sets `401 Unauthorized` and the framework's standard unauthorized text body.
    ///
    /// This helper selects the status and predefined body only. It does not implement
    /// authentication, validate credentials, or automatically add a `WWW-Authenticate`
    /// challenge. If an application requires additional authentication headers, add
    /// them explicitly with a response header method.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.unauthorized();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("WWW-Authenticate", "Bearer").unauthorized();
    /// ```
    pub fn unauthorized(&mut self) -> &mut Self {
        self.status(HTTP_UNAUTHORIZED).text(BODY_UNAUTHORIZED)
    }

    /// Sets `403 Forbidden` and the framework's standard forbidden text body.
    ///
    /// Use this when the request is understood but the application will not permit the
    /// requested operation. The helper does not perform authorization checks; the view
    /// must make that decision before calling it. Existing additional headers are
    /// retained while the body is replaced.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.forbidden();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Access", "denied").forbidden();
    /// ```
    pub fn forbidden(&mut self) -> &mut Self {
        self.status(HTTP_FORBIDDEN).text(BODY_FORBIDDEN)
    }

    /// Sets `404 Not Found` and the framework's standard not-found text body.
    ///
    /// Use this when the requested application resource or route is not available. A
    /// missing registered route is also handled by the server internally with this
    /// status, but a view may use the helper when a route exists and a more specific
    /// resource does not. The helper replaces the body and retains other headers.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.not_found();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Resource", "missing").not_found();
    /// ```
    pub fn not_found(&mut self) -> &mut Self {
        self.status(HTTP_NOT_FOUND).text(BODY_NOT_FOUND)
    }

    /// Sets `405 Method Not Allowed` and the framework's standard error body.
    ///
    /// This helper is for a request method that is not permitted for the selected
    /// resource. It does not automatically add an `Allow` header; add one explicitly if
    /// the application wants to advertise accepted methods. The method replaces the
    /// body while preserving the other response headers.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.method_not_allowed();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("Allow", "GET").method_not_allowed();
    /// ```
    pub fn method_not_allowed(&mut self) -> &mut Self {
        self.status(HTTP_METHOD_NOT_ALLOWED)
            .text(BODY_METHOD_NOT_ALLOWED)
    }

    /// Sets `408 Request Timeout` and the framework's standard timeout text body.
    ///
    /// This is a response-builder convenience for application-level timeout decisions.
    /// The low-level request reader also uses a corresponding status when it cannot
    /// receive the complete request within its configured absolute deadline. Calling
    /// this method does not change any socket timeout.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.request_timeout();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Timeout-Source", "application").request_timeout();
    /// ```
    pub fn request_timeout(&mut self) -> &mut Self {
        self.status(HTTP_REQUEST_TIMEOUT).text(BODY_REQUEST_TIMEOUT)
    }

    /// Sets `500 Internal Server Error` and the framework's standard error body.
    ///
    /// Use this when the handler cannot complete an otherwise valid request because of
    /// an unexpected application-side failure. The helper does not log the failure or
    /// expose an error object; applications should record diagnostic details separately
    /// and avoid returning sensitive internal information to clients.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.internal_server_error();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Error-Class", "internal").internal_server_error();
    /// ```
    pub fn internal_server_error(&mut self) -> &mut Self {
        self.status(HTTP_INTERNAL_SERVER_ERROR)
            .text(BODY_INTERNAL_SERVER_ERROR)
    }

    /// Configures a `301 Moved Permanently` redirect to `location`.
    ///
    /// The location is first converted into a `String` and checked with the same
    /// response-header value validation used by header helpers. Values containing a
    /// carriage return or line feed are rejected; in that case the response is left
    /// unchanged. For a valid location, the method sets the 301 status, replaces the
    /// `Location` header with the supplied value, and selects an empty text body.
    ///
    /// A permanent redirect communicates that the resource has moved to a stable new
    /// location. It does not contact or verify the destination.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.permanent_redirect("/new-location");
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.permanent_redirect("https://example.com/docs");
    /// ```
    pub fn permanent_redirect(&mut self, location: impl Into<String>) -> &mut Self {
        let location = location.into();

        if !is_valid_header_value(&location) {
            return self;
        }

        self.status(HTTP_MOVED_PERMANENTLY)
            .set_header(HTTP_RESPONSE_LOCATION, location)
            .text("")
    }

    /// Sets `413 Content Too Large` and the framework's standard body.
    ///
    /// This helper is useful when application-level validation determines that a
    /// payload is too large. The request reader separately enforces its configured
    /// maximum HTTP body size before a view is called. Calling this method changes the
    /// response status and body but does not change those parser limits.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.payload_too_large();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Limit", "application-defined").payload_too_large();
    /// ```
    pub fn payload_too_large(&mut self) -> &mut Self {
        self.status(HTTP_PAYLOAD_TOO_LARGE)
            .text(BODY_CONTENT_TOO_LARGE)
    }

    /// Sets `429 Too Many Requests` and the framework's standard body.
    ///
    /// Use this when application logic decides that a client has exceeded a request
    /// quota. The method does not implement rate limiting, track client identities, or
    /// add a `Retry-After` header automatically. Applications may add such metadata
    /// separately with [`BlankRes::header`].
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.too_many_requests();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("Retry-After", "30").too_many_requests();
    /// ```
    pub fn too_many_requests(&mut self) -> &mut Self {
        self.status(HTTP_TOO_MANY_REQUESTS)
            .text(BODY_TOO_MANY_REQUESTS)
    }

    /// Sets `502 Bad Gateway` and the framework's standard body.
    ///
    /// This helper is intended for an application acting as a gateway when an upstream
    /// service returns an unusable response. It does not make an upstream request or
    /// interpret upstream errors; it only configures the response sent by Blank.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.bad_gateway();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Upstream", "unavailable").bad_gateway();
    /// ```
    pub fn bad_gateway(&mut self) -> &mut Self {
        self.status(HTTP_BAD_GATEWAY).text(BODY_BAD_GATEWAY)
    }

    /// Sets `504 Gateway Timeout` and the framework's standard body.
    ///
    /// Use this when an upstream dependency or gateway operation fails to complete in
    /// the time allowed by application logic. This is distinct from the server's own
    /// request-read and response-write deadlines; this helper only changes the response
    /// builder and does not alter any network timeout.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.gateway_timeout();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Upstream-Timeout", "true").gateway_timeout();
    /// ```
    pub fn gateway_timeout(&mut self) -> &mut Self {
        self.status(HTTP_GATEWAY_TIMEOUT).text(BODY_GATEWAY_TIMEOUT)
    }

    /// Sets `503 Service Unavailable` and the framework's standard body.
    ///
    /// Use this when the application cannot currently handle the request. The listener
    /// also emits a small 503 response when the active worker limit has been reached.
    /// Calling this method in a view does not change the worker limit or stop the server.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.service_unavailable();
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("Retry-After", "60").service_unavailable();
    /// ```
    pub fn service_unavailable(&mut self) -> &mut Self {
        self.status(HTTP_SERVICE_UNAVAILABLE)
            .text(BODY_SERVICE_UNAVAILABLE)
    }

    /// Overrides the media type written for the current response body.
    ///
    /// The supplied value is stored as a custom content-type string. Carriage returns
    /// and line feeds are rejected to prevent the value from introducing additional
    /// response-header lines. Other syntax is not parsed or validated, so callers should
    /// supply a valid media type and any intended parameters. The override remains in
    /// effect until a textual body setter (`json`, `html`, or `text`) clears it, or until
    /// another valid call replaces it. `binary` sets its own octet-stream type.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.text("hello").content_type("text/plain; charset=utf-8");
    /// assert_eq!(response.response_content_type(), "text/plain; charset=utf-8");
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.binary(vec![1, 2, 3]).content_type("application/custom-format");
    /// assert_eq!(response.response_content_type(), "application/custom-format");
    /// ```
    pub fn content_type(&mut self, content_type: impl Into<String>) -> &mut Self {
        let content_type = content_type.into();

        // Reject line breaks so a content type cannot inject headers.
        if !content_type.contains('\r') && !content_type.contains('\n') {
            self.content_type = Some(content_type);
        }

        self
    }

    /// Appends one additional response-header value.
    ///
    /// Header names are validated and normalized to lowercase before storage. Values
    /// are rejected when they contain carriage returns or line feeds. If either part is
    /// invalid, the method leaves the header map unchanged and returns the builder so a
    /// chain can continue. If the name already exists, the new value is appended rather
    /// than replacing earlier values. This is appropriate for headers that may occur
    /// repeatedly, including `Set-Cookie`.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Trace", "first").header("X-Trace", "second");
    /// assert_eq!(response.header_values("x-trace").unwrap().len(), 2);
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("Cache-Control", "no-store");
    /// assert_eq!(response.header_values("CACHE-CONTROL").unwrap()[0], "no-store");
    /// ```
    pub fn header(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        let name = name.into();
        let value = value.into();

        if !is_valid_header_name(&name) || !is_valid_header_value(&value) {
            return self;
        }

        self.headers
            .entry(name.to_ascii_lowercase())
            .or_default()
            .push(value);

        self
    }

    /// Adds a response-header value using the same behavior as [`BlankRes::header`].
    ///
    /// This plural spelling is an alias retained alongside the singular method. It
    /// performs the same name/value validation, lowercase normalization, and append
    /// behavior; it does not accept a map or replace an existing value. Use
    /// [`BlankRes::set_header`] when replacement is intended.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.headers("X-Mode", "safe");
    /// assert_eq!(response.header_values("x-mode").unwrap()[0], "safe");
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.headers("Vary", "Accept").headers("Vary", "Origin");
    /// assert_eq!(response.header_values("vary").unwrap().len(), 2);
    /// ```
    pub fn headers(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.header(name, value)
    }

    /// Replaces all stored values for a response header with one value.
    ///
    /// The name and value undergo the same validation as [`BlankRes::header`]. On
    /// success, the name is normalized to lowercase and its map entry is replaced with
    /// a one-element vector. If validation fails, the existing header values are left
    /// untouched. This method is useful for singleton headers or when a prior appended
    /// value must be replaced.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Mode", "old").set_header("X-Mode", "new");
    /// assert_eq!(response.header_values("x-mode").unwrap()[0], "new");
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.set_header("Content-Language", "en");
    /// assert_eq!(response.header_values("content-language").unwrap()[0], "en");
    /// ```
    pub fn set_header(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        let name = name.into();
        let value = value.into();

        if !is_valid_header_name(&name) || !is_valid_header_value(&value) {
            return self;
        }

        self.headers.insert(name.to_ascii_lowercase(), vec![value]);
        self
    }

    /// Removes every stored value for the named response header.
    ///
    /// The supplied name is converted to lowercase before removal, matching the
    /// normalization used when headers are added. Removing a header that is not present
    /// is harmless. The method does not affect the internally generated standard
    /// headers such as content length or connection; it operates on the additional
    /// header map configured by the application.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Debug", "1").remove_header("x-debug");
    /// assert!(response.header_values("X-Debug").is_none());
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Trace", "a").header("X-Trace", "b");
    /// response.remove_header("X-TRACE");
    /// assert!(response.header_values("x-trace").is_none());
    /// ```
    pub fn remove_header(&mut self, name: &str) -> &mut Self {
        self.headers.remove(&name.to_ascii_lowercase());
        self
    }

    /// Returns every additional response-header value stored under `name`.
    ///
    /// Header names are normalized to lowercase when inserted, and the lookup performs
    /// the same normalization. A present name returns a borrowed slice of its values;
    /// an absent name returns `None`. This method inspects the user-configured header
    /// map, not the standard headers that the serializer constructs automatically.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.header("X-Tag", "one").header("X-Tag", "two");
    /// assert_eq!(response.header_values("x-tag").unwrap().len(), 2);
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let response = BlankRes::new();
    /// assert!(response.header_values("x-missing").is_none());
    /// ```
    pub fn header_values(&self, name: &str) -> Option<&[String]> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(Vec::as_slice)
    }

    /// Adds a simple `Set-Cookie` response header.
    ///
    /// The cookie name and value are validated before the header is created. Names use
    /// the HTTP token-character validation used for header names. Values reject control
    /// characters, spaces, commas, and semicolons so they can safely fit this simple
    /// `name=value` format. Invalid input is ignored and leaves the response unchanged.
    ///
    /// A valid cookie is appended as a `Set-Cookie` value and receives the default
    /// attributes defined by the constants module: `Path=/`, `HttpOnly`, and
    /// `SameSite=Lax`. This helper does not provide a general cookie builder for
    /// expiration, domain, secure transport, or custom attributes. For those cases,
    /// supply a carefully constructed value through [`BlankRes::header`].
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.cookie("session", "abc123");
    /// assert!(response.header_values("set-cookie").is_some());
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.cookie("theme", "dark").cookie("language", "en");
    /// assert_eq!(response.header_values("set-cookie").unwrap().len(), 2);
    /// ```
    pub fn cookie(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        let name = name.into();
        let value = value.into();

        if !is_valid_cookie_name(&name) || !is_valid_cookie_value(&value) {
            return self;
        }

        self.header(
            HTTP_RESPONSE_SET_COOKIE,
            format!("{}={}; {}", name, value, COOKIE_DEFAULT_ATTRIBUTES),
        )
    }

    /// Adds a `Set-Cookie` value intended to expire a cookie with the supplied name.
    ///
    /// The cookie name is validated before use. A valid name is added with an empty
    /// value and the predefined clearing attributes, which instruct the browser to
    /// remove the matching cookie. An invalid name is ignored. The helper does not
    /// inspect or remove a cookie from the current request; it only configures the
    /// response header that the browser will receive.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.clear_cookie("session");
    /// assert!(response.header_values("set-cookie").is_some());
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.cookie("theme", "dark").clear_cookie("theme");
    /// assert_eq!(response.header_values("set-cookie").unwrap().len(), 2);
    /// ```
    pub fn clear_cookie(&mut self, name: impl Into<String>) -> &mut Self {
        let name = name.into();

        if !is_valid_cookie_name(&name) {
            return self;
        }

        self.header(
            HTTP_RESPONSE_SET_COOKIE,
            format!("{}=; {}", name, COOKIE_CLEAR_ATTRIBUTES),
        )
    }

    /// Configures a temporary `302 Found` redirect to `location`.
    ///
    /// The location value is checked for carriage returns and line feeds before any
    /// response state is changed. If it is rejected, the response remains exactly as it
    /// was. For a valid value, this method sets the 302 status, replaces the `Location`
    /// header, and uses an empty text body. The destination is not fetched or verified.
    ///
    /// Use this for a temporary redirect; use [`BlankRes::permanent_redirect`] when the
    /// move is intended to be permanent.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.redirect("/login");
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.redirect("https://example.com/continue");
    /// ```
    pub fn redirect(&mut self, location: impl Into<String>) -> &mut Self {
        let location = location.into();

        if !is_valid_header_value(&location) {
            return self;
        }

        self.status(HTTP_FOUND)
            .set_header(HTTP_RESPONSE_LOCATION, location)
            .text("")
    }

    /// Returns the current response body as bytes.
    ///
    /// Textual response variants are exposed as their UTF-8 bytes; the binary variant
    /// is exposed unchanged. The slice borrows the response and is useful for tests,
    /// inspection, and code that needs to verify the configured payload before the
    /// server serializes it. It does not include HTTP headers or the status line.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.text("ok");
    /// assert_eq!(response.body(), b"ok");
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.binary(vec![0, 255]);
    /// assert_eq!(response.body(), &[0, 255]);
    /// ```
    pub fn body(&self) -> &[u8] {
        self.body.content()
    }

    /// Returns the media type that will be used when this response body is serialized.
    ///
    /// If a custom content type was accepted through [`BlankRes::content_type`], that
    /// value is returned. Otherwise, the method returns the default associated with the
    /// active body variant: JSON, HTML, plain text, or octet-stream for binary bytes.
    /// The returned string is borrowed from the response or from a static constant.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.json(r#"{"ok":true}"#);
    /// assert!(response.response_content_type().starts_with("application/json"));
    /// ```
    ///
    /// ```
    /// use blank_web::BlankRes;
    /// let mut response = BlankRes::new();
    /// response.text("data").content_type("text/plain; charset=utf-8");
    /// assert_eq!(response.response_content_type(), "text/plain; charset=utf-8");
    /// ```
    pub fn response_content_type(&self) -> &str {
        self.content_type
            .as_deref()
            .unwrap_or_else(|| self.body.content_type())
    }
}

/// Provides the same initial state as [`BlankRes::new`].
///
/// The default response is `200 OK` with an empty text body, no additional headers,
/// and no custom content-type override. This makes `BlankRes` usable with ordinary
/// Rust default construction as well as the explicit constructor.
///
/// # Examples
///
/// ```
/// use blank_web::BlankRes;
/// let response = BlankRes::default();
/// assert!(response.body().is_empty());
/// ```
///
/// ```
/// use blank_web::BlankRes;
/// let mut response = BlankRes::default();
/// response.ok().text("ready");
/// assert_eq!(response.body(), b"ready");
/// ```
impl Default for BlankRes {
    /// Returns the initial empty response state.
    ///
    /// The resulting builder is equivalent to `BlankRes::new()`: it has the default
    /// success status, an empty text body, no additional headers, and no media-type
    /// override. The response can then be configured with the ordinary builder methods.
    ///
    /// Examples: `BlankRes::default().text("ready")` creates a text response, while
    /// `BlankRes::default().no_content()` configures an empty 204 response.
    fn default() -> Self {
        Self {
            status: HTTP_OK,
            body: BlankBody::Text(BlankText::new("")),
            headers: HashMap::new(),
            content_type: None,
        }
    }
}

/// Checks whether a string contains only the HTTP token characters accepted for a field name.
///
/// The empty string is rejected. Every byte must be ASCII alphanumeric or one of the
/// punctuation characters listed by the HTTP token grammar in this implementation.
/// Non-ASCII bytes, spaces, separators such as `:`, and control characters fail the
/// check. This helper validates the name before it is inserted into the response map;
/// the request parser uses the same rule for incoming fields.
///
/// Internal examples:
/// - `content-type` and `X_Request_Id` pass because their bytes are allowed.
/// - `Bad Header`, `name:value`, and an empty name fail validation.
fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

/// Rejects response-header values containing carriage returns or line feeds.
///
/// These line-ending bytes are disallowed because a header value is serialized directly
/// into an HTTP header line. Accepting them could let a value terminate the current
/// field and inject another header or alter the message boundary. This helper does not
/// validate the complete grammar of every individual header value; it enforces the
/// newline restriction needed by this serializer.
///
/// Internal examples:
/// - `application/json; charset=utf-8` is accepted.
/// - A value containing `\r` or `\n` is rejected.
fn is_valid_header_value(value: &str) -> bool {
    !value.contains('\r') && !value.contains('\n')
}

/// Applies the header-token name rule to a cookie name.
///
/// The simple cookie helper writes the name into a `Set-Cookie` header, so it reuses
/// the conservative token validation performed by [`is_valid_header_name`]. This
/// rejects empty names, whitespace, separators, and non-token bytes. It does not
/// implement a separate cookie-name grammar beyond that shared validation.
///
/// Internal examples:
/// - `session_id` is accepted.
/// - `session id` and an empty name are rejected.
fn is_valid_cookie_name(name: &str) -> bool {
    is_valid_header_name(name)
}

/// Checks whether a value is safe for the simple `name=value` cookie helper.
///
/// The helper rejects bytes at or below ASCII space, the delete character, semicolons,
/// and commas. These characters are excluded because this API assembles a compact
/// cookie pair and fixed attributes into one header value. This check is deliberately
/// narrow and is not a general-purpose parser or encoder for every cookie format.
///
/// Internal examples:
/// - `abc123` and `dark_mode` are accepted.
/// - Values containing a space, comma, semicolon, or control byte are rejected.
fn is_valid_cookie_value(value: &str) -> bool {
    !value
        .bytes()
        .any(|byte| byte <= 0x20 || byte == 0x7f || byte == b';' || byte == b',')
}

// Serializes a response into the exact byte sequence written to a client socket.
//
// This private macro centralizes response framing so normal view responses, parser
// errors, and overload responses use the same wire format. Its four arguments are the
// status value, a byte slice for the body, the content-type value, and a map of
// additional response headers. It calculates Content-Length from the body byte count
// (not from a character count), emits the configured HTTP version and standard
// Connection: close header, appends every stored additional-header value, writes the
// blank line that ends the header section, and finally appends the body bytes.
//
// The body argument is treated as bytes from the start. This is important for binary
// responses: converting the body to text would corrupt arbitrary byte sequences.
// Header values are expected to have been validated before they reach this macro; the
// macro is a serializer, not a second validation layer.
//
// Conceptual examples:
// - A text response passes `b"hello"`, a text content type, and the response header map.
// - A binary response passes the original byte slice, octet-stream, and the same map.
// - A parser error passes a short static body and an empty additional-header map.
macro_rules! http_return {
    (
        $res_code:expr,
        $content:expr,
        $content_type:expr,
        $headers:expr
    ) => {{
        // Keep the body as bytes so binary responses remain intact.
        let content: &[u8] = $content;

        // Make the response line and standard headers.
        let mut response = format!(
            "{} {}\r\n{}: {}\r\n{}: {}\r\n{}: {}\r\n",
            HTTP_RESPONSE_VERSION,
            $res_code,
            HTTP_RESPONSE_CONTENT_LENGTH,
            content.len(),
            HTTP_RESPONSE_CONTENT_TYPE,
            $content_type,
            HTTP_RESPONSE_CONNECTION,
            HTTP_CONNECTION_CLOSE,
        )
        .into_bytes();

        // Add every stored header value.
        for (name, values) in ($headers).iter() {
            for value in values {
                response.extend_from_slice(format!("{}: {}\r\n", name, value).as_bytes());
            }
        }

        // Finish the headers and append the response body.
        response.extend_from_slice(HTTP_LINE_END);
        response.extend_from_slice(content);

        response
    }};
}

/// A configurable HTTP server and route registry.
///
/// `Blank` stores the local bind host, TCP port, and a map from normalized route paths
/// to [`BlankView`] function pointers. It is the main entry point for creating a server:
/// configure the bind address, register views, and then choose a blocking or
/// background-thread serving method.
///
/// Routes are matched by exact path. Registration ensures that a route begins with `/`,
/// but it does not perform parameter matching, wildcard matching, trailing-slash
/// rewriting, or query-string matching. Query strings are parsed separately and are
/// available through [`BlankReq`]. Registering the same normalized path again replaces
/// its previous view.
///
/// The listener receives a snapshot of the route map when the server starts. The
/// serving methods clone the current map into shared server state; later modifications
/// to the original `Blank` value do not update a running listener. Configure routes
/// before starting the server.
///
/// # Examples
///
/// A minimal route:
///
/// ```no_run
/// use blank_web::{Blank, BlankReq, BlankRes};
/// fn index(_request: &BlankReq, response: &mut BlankRes) {
///     response.text("Blank is running");
/// }
/// let mut app = Blank::new();
/// app.route_func("/", index);
/// app.serve().expect("server failed");
/// ```
///
/// Configure the address and multiple routes:
///
/// ```no_run
/// use blank_web::{Blank, BlankReq, BlankRes};
/// fn root(_: &BlankReq, out: &mut BlankRes) { out.text("root"); }
/// fn status(_: &BlankReq, out: &mut BlankRes) { out.json(r#"{"ready":true}"#); }
/// let mut app = Blank::new();
/// app.host("127.0.0.1").port(7600);
/// app.route_func("/", root).route_func("/status", status);
/// app.serve_await().expect("server failed");
/// ```
///
/// For an operating-system-selected port or background execution, use
/// [`Blank::serve_async`] and retain its [`BlankServer`] handle.
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Blank {
    /// TCP Port on which the server listens on.
    tcp_port: Port,

    /// Host on which the server listens.
    ///
    /// localhost is the only available host right now by default.
    host: Host,

    /// All registered routes and their views.
    routes: HashMap<String, BlankView>,
}

impl Blank {
    /// Creates a server configuration with Blank's default host and port.
    ///
    /// The new value starts with the default port from the constants module, the
    /// default localhost host, and an empty route registry. Construction does not bind
    /// a socket or start a thread. Use [`Blank::host`], [`Blank::port`], and
    /// [`Blank::route_func`] to configure it before starting the listener.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::Blank;
    /// let app = Blank::new();
    /// assert_eq!(app.get_port(), 7600);
    /// ```
    ///
    /// ```
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn index(_: &BlankReq, out: &mut BlankRes) { out.text("hi"); }
    /// let mut app = Blank::new();
    /// app.route_func("/", index);
    /// assert!(app.has_route("/"));
    /// ```
    pub fn new() -> Self {
        Self {
            tcp_port: DEFAULT_PORT,
            host: LOCALHOST.to_string(),
            routes: HashMap::new(),
        }
    }

    /// Sets the host or IP-address string used when the listener binds.
    ///
    /// The argument is converted into an owned `String` and replaces the currently
    /// configured host. This method only changes configuration; it does not resolve the
    /// host, check interface availability, or bind the socket. Binding occurs when
    /// `serve_await`, `serve`, or `serve_async` is called. Invalid or unavailable bind
    /// addresses are reported by those serving methods as a `BlankError`.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::Blank;
    /// let mut app = Blank::new();
    /// app.host("127.0.0.1");
    /// assert_eq!(app.get_host(), "127.0.0.1");
    /// ```
    ///
    /// ```
    /// use blank_web::Blank;
    /// let mut app = Blank::new();
    /// app.host(String::from("localhost")).port(7600);
    /// ```
    pub fn host(&mut self, host: impl Into<String>) -> &mut Self {
        self.host = host.into();
        self
    }

    /// Returns the currently configured bind host.
    ///
    /// The result is a borrowed string slice from the `Blank` configuration. It reports
    /// the configured value, not necessarily the numeric address ultimately selected by
    /// the operating system. For the actual bound socket address, use
    /// [`BlankServer::local_addr`] on a server started with `serve_async`.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::Blank;
    /// let app = Blank::new();
    /// assert_eq!(app.get_host(), "localhost");
    /// ```
    ///
    /// ```
    /// use blank_web::Blank;
    /// let mut app = Blank::new();
    /// app.host("127.0.0.1");
    /// assert_eq!(app.get_host(), "127.0.0.1");
    /// ```
    pub fn get_host(&self) -> &str {
        &self.host
    }

    /// Sets the TCP port used when the listener binds.
    ///
    /// The value replaces the current configured port and is not bound immediately.
    /// Port `0` is permitted and asks the operating system to select an available port;
    /// inspect [`BlankServer::local_addr`] to discover that selected port when using
    /// `serve_async`. A bind failure is returned by the serving method rather than by
    /// this configuration setter.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::Blank;
    /// let mut app = Blank::new();
    /// app.port(8080);
    /// assert_eq!(app.get_port(), 8080);
    /// ```
    ///
    /// ```
    /// use blank_web::Blank;
    /// let mut app = Blank::new();
    /// app.port(0); // Ask the operating system to choose the port at bind time.
    /// ```
    pub fn port(&mut self, port: Port) -> &mut Self {
        self.tcp_port = port;
        self
    }

    /// Returns the currently configured TCP port.
    ///
    /// This is the requested port stored in the configuration. When the configured
    /// value is `0`, this accessor continues to return `0`; it does not report the
    /// ephemeral port chosen after binding. Use [`BlankServer::local_addr`] for the
    /// actual listening address returned by the operating system.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::Blank;
    /// let mut app = Blank::new();
    /// app.port(5748);
    /// assert_eq!(app.get_port(), 5748);
    /// ```
    ///
    /// ```
    /// use blank_web::Blank;
    /// let mut app = Blank::new();
    /// app.port(0);
    /// assert_eq!(app.get_port(), 0);
    /// ```
    pub fn get_port(&self) -> Port {
        self.tcp_port
    }

    /// Registers a view function for an exact request path.
    ///
    /// If `route` does not begin with `/`, a leading slash is added before the entry is
    /// stored. No other normalization is performed: paths are not made lowercase,
    /// trailing slashes are not removed, and path parameters or wildcard patterns are
    /// not interpreted. At request time, the parsed path must match the stored key
    /// exactly. Query strings are excluded from the route key and can be read from the
    /// request's query accessors.
    ///
    /// Registering a route whose normalized key already exists replaces the previous
    /// function. The method returns `&mut Self` to allow chained registrations. A running
    /// server uses the route-map snapshot taken when it started, so register or replace
    /// routes before invoking a serving method.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn home(_: &BlankReq, out: &mut BlankRes) { out.text("home"); }
    /// let mut app = Blank::new();
    /// app.route_func("/", home);
    /// assert!(app.has_route("/"));
    /// ```
    ///
    /// ```
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn first(_: &BlankReq, out: &mut BlankRes) { out.text("first"); }
    /// fn replacement(_: &BlankReq, out: &mut BlankRes) { out.text("replacement"); }
    /// let mut app = Blank::new();
    /// app.route_func("health", first); // Stored as "/health".
    /// app.route_func("/health", replacement); // Replaces the existing view.
    /// assert_eq!(app.route_count(), 1);
    /// ```
    pub fn route_func(&mut self, route: &str, view: BlankView) -> &mut Self {
        let route = normalize_route(route);
        self.routes.insert(route, view);
        self
    }

    /// Checks whether the normalized route key is present in the registry.
    ///
    /// The supplied path is passed through the same leading-slash normalization used
    /// by [`Blank::route_func`], so checking `health` finds a route registered as
    /// `/health`. Matching is otherwise exact. This method examines the configuration
    /// held by this `Blank` value; it does not inspect a separately running server's
    /// already-cloned route snapshot.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn health(_: &BlankReq, out: &mut BlankRes) { out.text("ok"); }
    /// let mut app = Blank::new();
    /// app.route_func("/health", health);
    /// assert!(app.has_route("health"));
    /// ```
    ///
    /// ```
    /// use blank_web::Blank;
    /// let app = Blank::new();
    /// assert!(!app.has_route("/missing"));
    /// ```
    pub fn has_route(&self, route: &str) -> bool {
        self.routes.contains_key(&normalize_route(route))
    }

    /// Removes the route associated with the normalized path.
    ///
    /// The same leading-slash normalization used during registration is applied before
    /// removal. The return value is `true` when an entry existed and was removed, or
    /// `false` when no matching entry was present. This affects the configuration in
    /// this `Blank` value; it does not rewrite the route snapshot of a server that has
    /// already started.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn home(_: &BlankReq, out: &mut BlankRes) { out.text("home"); }
    /// let mut app = Blank::new();
    /// app.route_func("/", home);
    /// assert!(app.remove_route("/"));
    /// assert!(!app.has_route("/"));
    /// ```
    ///
    /// ```
    /// use blank_web::Blank;
    /// let mut app = Blank::new();
    /// assert!(!app.remove_route("unknown"));
    /// ```
    pub fn remove_route(&mut self, route: &str) -> bool {
        self.routes.remove(&normalize_route(route)).is_some()
    }

    /// Removes every route from this server configuration.
    ///
    /// After this method returns, the route registry is empty and `route_count()` is
    /// zero. It returns the mutable configuration so new routes can be registered in
    /// the same chain. As with other registry changes, clearing routes does not affect a
    /// listener that has already taken its startup snapshot.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn home(_: &BlankReq, out: &mut BlankRes) { out.text("home"); }
    /// let mut app = Blank::new();
    /// app.route_func("/", home).clear_routes();
    /// assert_eq!(app.route_count(), 0);
    /// ```
    ///
    /// ```
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn status(_: &BlankReq, out: &mut BlankRes) { out.text("ok"); }
    /// let mut app = Blank::new();
    /// app.clear_routes().route_func("/status", status);
    /// assert_eq!(app.route_count(), 1);
    /// ```
    pub fn clear_routes(&mut self) -> &mut Self {
        self.routes.clear();
        self
    }

    /// Returns the number of currently registered route keys.
    ///
    /// The count reflects the `Blank` configuration at the time of the call. Registering
    /// a duplicate normalized key replaces its view and does not increase the count;
    /// removing a present route decreases it by one. This method does not count active
    /// connections, workers, or routes in a server snapshot that was started earlier.
    ///
    /// # Examples
    ///
    /// ```
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn page(_: &BlankReq, out: &mut BlankRes) { out.text("page"); }
    /// let mut app = Blank::new();
    /// assert_eq!(app.route_count(), 0);
    /// app.route_func("/page", page);
    /// assert_eq!(app.route_count(), 1);
    /// ```
    ///
    /// ```
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn page(_: &BlankReq, out: &mut BlankRes) { out.text("page"); }
    /// let mut app = Blank::new();
    /// app.route_func("page", page).route_func("/page", page);
    /// assert_eq!(app.route_count(), 1);
    /// ```
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Binds the configured address and runs the listener on the current thread.
    ///
    /// This is Blank's blocking serving entry point. It first attempts to bind a TCP
    /// listener to the configured host and port, then enters the accept loop. The call
    /// does not return while the server is operating normally; it returns if the
    /// listener encounters an accept error. Unlike `serve_async`, this entry point does
    /// not return a handle through which the caller can request shutdown. Errors from
    /// binding or accepting are represented by `BlankError`.
    ///
    /// The listener uses blocking operating-system socket operations. It does not busy
    /// poll an idle socket. Each accepted connection is assigned to a bounded worker
    /// thread, and the route map is cloned when serving begins. This method is useful
    /// when the server should own the main thread. Use [`Blank::serve_async`] when the
    /// caller needs the server to run on a background thread and wants a stop handle.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn index(_: &BlankReq, out: &mut BlankRes) { out.text("hello"); }
    /// let mut app = Blank::new();
    /// app.route_func("/", index);
    /// app.serve_await().expect("server failed");
    /// ```
    ///
    /// ```no_run
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn health(_: &BlankReq, out: &mut BlankRes) { out.text("ok"); }
    /// let mut app = Blank::new();
    /// app.host("127.0.0.1").port(9000).route_func("/health", health);
    /// app.serve().expect("server failed");
    /// ```
    pub fn serve_await(&self) -> Result<(), BlankError> {
        let server = bind_server(&self.host, self.tcp_port)?;
        run_server(
            server,
            Arc::new(self.routes.clone()),
            Arc::new(AtomicBool::new(false)),
        )
    }

    /// Runs the server with the same blocking behavior as [`Blank::serve_await`].
    ///
    /// `serve` is an alias rather than a separate execution mode. It binds the
    /// configured host and port, clones the current route map for the listener, and
    /// blocks the calling thread in the server loop. The result reports the same
    /// `BlankError` failures as `serve_await`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn index(_: &BlankReq, out: &mut BlankRes) { out.text("hello"); }
    /// let mut app = Blank::new();
    /// app.route_func("/", index);
    /// app.serve().expect("server failed");
    /// ```
    ///
    /// ```no_run
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn api(_: &BlankReq, out: &mut BlankRes) { out.json(r#"{"ready":true}"#); }
    /// let mut app = Blank::new();
    /// app.port(7601).route_func("/ready", api);
    /// app.serve().expect("server failed");
    /// ```
    pub fn serve(&self) -> Result<(), BlankError> {
        self.serve_await()
    }

    /// Binds the configured address and starts the listener on a background OS thread.
    ///
    /// Despite the method name, this implementation does not return a Future and does
    /// not use an async runtime. It creates a named operating-system thread to run the
    /// same blocking listener loop used by `serve_await`. The method itself returns
    /// after the listener has been bound and the thread has been spawned, yielding a
    /// [`BlankServer`] handle that exposes the actual local address and controls
    /// shutdown.
    ///
    /// The route registry is cloned before the new thread starts. Register or remove
    /// routes before this call; later edits to the original `Blank` configuration do
    /// not update the server that is already running. Binding and thread-spawn failures
    /// are returned as `BlankError`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn health(_: &BlankReq, out: &mut BlankRes) { out.text("healthy"); }
    /// let mut app = Blank::new();
    /// app.port(0).route_func("/health", health);
    /// let server = app.serve_async().expect("could not start");
    /// println!("bound at {}", server.local_addr());
    /// server.async_stop().expect("could not stop");
    /// ```
    ///
    /// ```no_run
    /// use blank_web::{Blank, BlankReq, BlankRes};
    /// fn index(_: &BlankReq, out: &mut BlankRes) { out.text("background server"); }
    /// let mut app = Blank::new();
    /// app.route_func("/", index);
    /// let server = app.serve_async().expect("could not start");
    /// // Other application work can run here while the listener uses its own thread.
    /// server.async_stop().expect("could not stop");
    /// ```
    pub fn serve_async(&self) -> Result<BlankServer, BlankError> {
        let server = bind_server(&self.host, self.tcp_port)?;
        let address = server.local_addr().map_err(|_| ERROR_BIND_SERVER)?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let routes = Arc::new(self.routes.clone());

        let handle = thread::Builder::new()
            .name(SERVER_THREAD_NAME.to_string())
            .spawn(move || run_server(server, routes, thread_stop))
            .map_err(|_| ERROR_SPAWN_SERVER_THREAD)?;

        Ok(BlankServer {
            stop,
            address,
            thread: Some(handle),
        })
    }
}

/// Owns the lifecycle handle for a server started by [`Blank::serve_async`].
///
/// The handle stores a shared stop flag, the bound local socket address, and the join
/// handle for the listener thread. It is intentionally owned: calling `async_stop`
/// consumes the handle, signals shutdown, wakes the blocking listener, and waits for
/// the listener thread to finish. Dropping the handle performs the same shutdown path
/// automatically, so a server is not left running solely because its handle went out
/// of scope.
///
/// Shutdown stops accepting new connections. Request workers that have already been
/// started are joined before the listener thread exits. Their socket reads and writes
/// have deadlines, but application view functions are ordinary synchronous functions;
/// a view that blocks indefinitely can therefore delay shutdown. The handle does not
/// expose a non-blocking stop operation or a restart operation.
///
/// # Examples
///
/// ```no_run
/// use blank_web::{Blank, BlankReq, BlankRes};
/// fn index(_: &BlankReq, out: &mut BlankRes) { out.text("hi"); }
/// let mut app = Blank::new();
/// app.port(0).route_func("/", index);
/// let server = app.serve_async().expect("start failed");
/// println!("address: {}", server.local_addr());
/// server.async_stop().expect("stop failed");
/// ```
///
/// Dropping the handle also stops the background listener:
///
/// ```no_run
/// use blank_web::Blank;
/// let app = Blank::new();
/// let server = app.serve_async().expect("start failed");
/// drop(server); // Drop requests shutdown and joins the server thread.
/// ```
pub struct BlankServer {
    /// Shared shutdown signal observed by the listener thread. Shutdown sets this flag
    /// before opening the local wake-up connection that releases blocking `accept()`.
    stop: Arc<AtomicBool>,
    /// Actual socket address returned by the operating system after binding, including
    /// an ephemeral port when the configured port was zero.
    address: SocketAddr,
    /// Listener thread join handle. Taking this option during shutdown prevents the
    /// same thread from being joined more than once.
    thread: Option<JoinHandle<Result<(), BlankError>>>,
}

impl BlankServer {
    /// Returns the actual local socket address selected when the listener was bound.
    ///
    /// This value comes from `TcpListener::local_addr`, so it is more authoritative than
    /// the configured host and port when the port was set to `0`. The returned address
    /// is a copy and can be inspected after the listener starts. It does not indicate
    /// whether the server is still running; use the handle's ownership and shutdown
    /// methods to control its lifecycle.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use blank_web::Blank;
    /// let app = Blank::new();
    /// let server = app.serve_async().expect("bind failed");
    /// let address = server.local_addr();
    /// println!("{}:{}", address.ip(), address.port());
    /// server.async_stop().expect("stop failed");
    /// ```
    ///
    /// ```no_run
    /// use blank_web::Blank;
    /// let mut app = Blank::new();
    /// app.port(0);
    /// let server = app.serve_async().expect("bind failed");
    /// assert_ne!(server.local_addr().port(), 0);
    /// server.async_stop().expect("stop failed");
    /// ```
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    /// Stops the listener thread and waits for server shutdown to complete.
    ///
    /// The method consumes the handle, sets the shared stop flag, creates a local
    /// connection to wake the listener if it is blocked in `accept()`, and joins the
    /// listener thread. The listener then joins workers that were already servicing
    /// accepted connections. A successful join returns the listener loop's result, so
    /// an accept-loop error can be returned as `BlankError`; a thread panic is mapped to
    /// the configured server-thread-panic error.
    ///
    /// Because the handle is consumed, it cannot be used again after this call. The
    /// same stop-and-join operation is used by `Drop`, so explicit shutdown is optional
    /// when scope-based shutdown is sufficient.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use blank_web::Blank;
    /// let app = Blank::new();
    /// let server = app.serve_async().expect("start failed");
    /// server.async_stop().expect("shutdown failed");
    /// ```
    ///
    /// ```no_run
    /// use blank_web::Blank;
    /// let app = Blank::new();
    /// let server = app.serve_async().expect("start failed");
    /// let address = server.local_addr();
    /// println!("server at {address}");
    /// let shutdown_result = server.async_stop();
    /// assert!(shutdown_result.is_ok());
    /// ```
    pub fn async_stop(mut self) -> Result<(), BlankError> {
        self.stop_and_join()
    }

    /// Performs the shared shutdown procedure used by explicit stop and `Drop`.
    ///
    /// Taking the optional join handle makes shutdown idempotent for the lifetime of the
    /// handle: after it has been taken, a later call returns `Ok(())` without signaling
    /// or joining a second time. On the first call, the method stores `true` in the
    /// shared stop flag with release ordering, makes a short local TCP connection to
    /// wake the listener's blocking `accept`, and joins the listener thread.
    ///
    /// The wake-up connection is not an application request. The listener checks the
    /// stop flag immediately after accepting it and closes that connection instead of
    /// dispatching it to a worker. The join waits for the listener loop and its active
    /// workers to finish. A panic in the listener thread is translated into the
    /// configured server-thread-panic error.
    fn stop_and_join(&mut self) -> Result<(), BlankError> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };

        self.stop.store(true, Ordering::Release);

        // A local connection wakes the listener without polling or sleeping.
        let wake_address = loopback_address(self.address);
        let _ = TcpStream::connect_timeout(
            &wake_address,
            Duration::from_millis(SERVER_WAKE_TIMEOUT_MS),
        );

        thread.join().map_err(|_| ERROR_SERVER_THREAD_PANIC)?
    }
}

/// Ensures that dropping a server handle also stops and joins its listener thread.
///
/// This prevents the background listener from silently outliving the value that owns
/// its lifecycle. `Drop` calls the same internal shutdown routine as `async_stop`, but
/// cannot return an error; any shutdown error is therefore intentionally ignored at
/// drop time. Applications that need to observe a thread-panic error should call
/// `async_stop` explicitly before the handle is dropped.
///
/// # Examples
///
/// ```no_run
/// use blank_web::Blank;
/// let app = Blank::new();
/// let server = app.serve_async().expect("start failed");
/// drop(server); // The listener is stopped and joined here.
/// ```
///
/// ```no_run
/// use blank_web::Blank;
/// let app = Blank::new();
/// {
///     let _server = app.serve_async().expect("start failed");
/// } // The handle leaves scope, triggering shutdown.
/// ```
impl Drop for BlankServer {
    /// Runs the shutdown procedure when the owning handle leaves scope.
    ///
    /// Drop cannot return the result of joining the listener thread, so this method
    /// deliberately discards the internal shutdown result. Call `async_stop` explicitly
    /// when the caller needs to observe a shutdown error.
    fn drop(&mut self) {
        // Do not leave a server thread running when its handle is dropped.
        let _ = self.stop_and_join();
    }
}

/// Creates the TCP listener for a configured host and port.
///
/// The host string and numeric port are combined into the address string passed to
/// `TcpListener::bind`. Any operating-system bind error is mapped to Blank's bind
/// error constant, keeping the public error type independent of the full `io::Error`.
/// This function does not start accepting connections; it only returns the bound
/// listener. Both blocking and background serving paths use it before starting the
/// listener loop.
///
/// Internal examples:
/// - Binding `127.0.0.1` with a fixed port creates a listener on that local endpoint if available.
/// - Binding with port `0` lets the operating system select a free port, discoverable through `local_addr()`.
fn bind_server(host: &str, port: Port) -> Result<TcpListener, BlankError> {
    TcpListener::bind(format!("{}:{}", host, port)).map_err(|_| ERROR_BIND_SERVER)
}

/// Produces a loopback destination with the same port for listener wake-up.
///
/// During shutdown, the listener may be blocked inside `accept()`. Connecting to the
/// listener's own port creates a connection that releases that blocking call. If the
/// bound address is an unspecified IPv4 or IPv6 address, this helper substitutes the
/// corresponding loopback address because an unspecified address is a bind wildcard,
/// not the destination needed for the local wake-up connection. A concrete bound IP is
/// preserved unchanged.
///
/// Internal examples:
/// - An unspecified IPv4 listener becomes `127.0.0.1:<same-port>`.
/// - An unspecified IPv6 listener becomes `[::1]:<same-port>`.
/// - A concrete loopback or interface address is returned unchanged.
fn loopback_address(address: SocketAddr) -> SocketAddr {
    match address {
        SocketAddr::V4(address) if address.ip().is_unspecified() => {
            SocketAddr::from(([127, 0, 0, 1], address.port()))
        }
        SocketAddr::V6(address) if address.ip().is_unspecified() => {
            SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], address.port()))
        }
        address => address,
    }
}

/// Runs the blocking accept loop and dispatches connections to a bounded worker set.
///
/// The listener thread waits in `accept()` until a client connects or the shutdown
/// wake-up connection arrives. Interrupted accepts are retried; other accept errors
/// terminate the loop with Blank's accept error. After each accepted connection, the
/// stop flag is checked before the client can become application work. Completed
/// worker handles are reaped before the maximum-active-connection limit is evaluated.
///
/// When the active worker count has reached the configured maximum, the newly accepted
/// client receives a short `503 Service Unavailable` response and is closed. Otherwise,
/// a named worker thread is spawned to run `handle_connection`. If spawning a worker
/// fails, that client is not dispatched and the loop continues. On exit, the listener
/// joins every worker still recorded in the list before returning its final result.
///
/// The shared route map is immutable while workers use it. The `Arc` lets each worker
/// own a reference without copying the full map for every lookup.
fn run_server(
    server: TcpListener,
    routes: Arc<HashMap<String, BlankView>>,
    stop: Arc<AtomicBool>,
) -> Result<(), BlankError> {
    // Keep worker handles so completed workers can be reaped and shutdown is clean.
    let mut workers: Vec<JoinHandle<()>> = Vec::new();
    let mut result = Ok(());

    loop {
        // The listener blocks here until a client connects or shutdown wakes it.
        let (client, _address) = match server.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => {
                result = Err(ERROR_ACCEPT_CONNECTION);
                break;
            }
        };

        // The wake connection exists only to release accept() during shutdown.
        if stop.load(Ordering::Acquire) {
            let _ = client.shutdown(Shutdown::Both);
            break;
        }

        // Remove completed workers before checking the connection limit.
        reap_workers(&mut workers);

        // Refuse excess connections instead of creating an unbounded number of threads.
        if workers.len() >= MAX_ACTIVE_CONNECTIONS {
            write_service_unavailable(client);
            continue;
        }

        let routes = Arc::clone(&routes);
        let worker = thread::Builder::new()
            .name(CLIENT_THREAD_NAME.to_string())
            .spawn(move || handle_connection(client, routes));

        match worker {
            Ok(worker) => workers.push(worker),
            Err(_) => continue,
        }
    }

    // Finish active requests before returning from the server thread.
    for worker in workers {
        let _ = worker.join();
    }

    result
}
/// Removes and joins every worker thread that has already completed.
///
/// A finished `JoinHandle` is removed with `swap_remove`, avoiding the cost of shifting
/// the remainder of the vector. The index advances only when the current handle is
/// still running, because a removal places another element into the current slot. The
/// join result is ignored: a worker panic does not terminate the listener loop. Handles
/// for workers that have not yet finished remain in the vector and continue to count
/// toward the configured connection limit.
///
/// Internal examples:
/// - A completed worker is joined and removed before the next capacity check.
/// - A still-running worker remains tracked so the server cannot exceed its worker cap.
fn reap_workers(workers: &mut Vec<JoinHandle<()>>) {
    let mut index = 0;

    while index < workers.len() {
        if workers[index].is_finished() {
            let worker = workers.swap_remove(index);
            let _ = worker.join();
        } else {
            index += 1;
        }
    }
}

/// Sends the listener's overload response to a client that exceeded the worker limit.
///
/// The response is built through the same HTTP serializer used elsewhere, with status
/// `503 Service Unavailable`, the predefined short text body, and no application-added
/// headers. It is written using the bounded write helper, after which the socket is
/// explicitly shut down in both directions. Errors are ignored because this path is
/// already rejecting the connection and has no caller that can recover from a failed
/// client write.
///
/// Internal examples:
/// - When all worker slots are occupied, the next accepted stream is sent a 503.
/// - When the client disconnects before the response can be written, the helper still closes the stream.
fn write_service_unavailable(mut client: TcpStream) {
    let response = http_return!(
        HTTP_SERVICE_UNAVAILABLE,
        BODY_SERVICE_UNAVAILABLE.as_bytes(),
        CONTENT_TYPE_TEXT,
        &HashMap::<String, Vec<String>>::new()
    );

    let _ = write_all_timeout(&mut client, &response);
    let _ = client.shutdown(Shutdown::Both);
}

/// Adds a leading slash when a route registration or lookup omits one.
///
/// The function does not trim whitespace, collapse repeated slashes, decode percent
/// escapes, remove a trailing slash, or interpret route parameters. If the input
/// already begins with `/`, it is copied unchanged; otherwise exactly one slash is
/// prepended. Registration and route-management lookups share this helper so they use
/// the same key transformation.
///
/// Internal examples:
/// - `health` becomes `/health`.
/// - `/health` remains `/health`.
/// - `health/` becomes `/health/`; the trailing slash is preserved.
fn normalize_route(route: &str) -> String {
    if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{}", route)
    }
}

/// Categories of failures that can occur while reading a request from a TCP stream.
///
/// This private enum distinguishes malformed input, header and body size limits,
/// unsupported transfer encoding, an expired read deadline, and other socket I/O
/// failures. Keeping these cases separate allows the connection worker to select an
/// appropriate HTTP status and short response body. It is intentionally not exposed as
/// a public application error type; handlers only receive requests that passed this
/// stage.
///
/// Examples of the mapping are: malformed request data becomes 400, an oversized
/// header becomes 431, an oversized body becomes 413, unsupported transfer encoding
/// becomes 501, and a read deadline becomes 408.
enum RequestReadError {
    /// The bytes or request framing are malformed, incomplete, or otherwise invalid.
    BadRequest,
    /// The request header section exceeded the configured maximum header size.
    HeaderTooLarge,
    /// The declared request body length exceeded the configured maximum body size.
    BodyTooLarge,
    /// The request uses a non-empty Transfer-Encoding value that this reader does not implement.
    NotImplemented,
    /// The absolute deadline for receiving the complete request expired.
    TimedOut,
    /// A socket read or timeout-configuration operation failed for another I/O reason.
    Io,
}

/// Maps a low-level request-read failure to the HTTP status and short text body.
///
/// This conversion is used before a full `BlankReq` exists, so it returns static
/// response components rather than constructing a response builder. Header-size and
/// body-size failures map to their corresponding client error statuses; unsupported
/// transfer encoding maps to 501; elapsed read deadlines map to 408. The I/O category
/// is currently represented as a bad request in the response mapping.
impl RequestReadError {
    /// Returns the HTTP status string and predefined text body for this read failure.
    ///
    /// The pair is consumed by the connection worker when it needs to reject a request
    /// before a `BlankReq` or application view exists. Returning static strings keeps
    /// this low-level error path allocation-free until the HTTP response buffer is
    /// assembled. Malformed and generic I/O failures become 400; size failures become
    /// 431 or 413; unsupported transfer encoding becomes 501; and an expired request
    /// read deadline becomes 408.
    ///
    /// For example, `HeaderTooLarge` maps to the header-fields-too-large status, while
    /// `TimedOut` maps to the request-timeout status.
    fn response(&self) -> (&'static str, &'static str) {
        match self {
            Self::BadRequest => (HTTP_BAD_REQUEST, BODY_BAD_REQUEST),
            Self::HeaderTooLarge => (HTTP_HEADER_FIELDS_TOO_LARGE, BODY_REQUEST_HEADERS_TOO_LARGE),
            Self::BodyTooLarge => (HTTP_PAYLOAD_TOO_LARGE, BODY_CONTENT_TOO_LARGE),
            Self::NotImplemented => (HTTP_NOT_IMPLEMENTED, BODY_TRANSFER_ENCODING_NOT_SUPPORTED),
            Self::TimedOut => (HTTP_REQUEST_TIMEOUT, BODY_REQUEST_TIMEOUT),
            Self::Io => (HTTP_BAD_REQUEST, BODY_BAD_REQUEST),
        }
    }
}

/// Reads exactly one complete HTTP request from a blocking TCP stream.
///
/// The reader first accumulates bytes until it finds the header terminator. It then
/// extracts request-body metadata, rejects non-empty `Transfer-Encoding`, verifies the
/// body limit, and computes the total byte count from the header boundary plus
/// `Content-Length`. It continues reading only until that count is reached and
/// truncates any additional bytes so one connection produces at most one request.
///
/// One absolute deadline covers the entire read operation. Each socket read receives
/// only the remaining time; the timeout is not restarted for every byte. The maximum
/// header and body sizes are enforced before the complete buffer is returned. A clean
/// end-of-stream before any bytes produces `Ok(None)`, which represents a client that
/// closed without sending a request. End-of-stream after partial data is a malformed
/// request. Timed-out reads, oversized input, unsupported transfer encoding, malformed
/// metadata, and other I/O failures are returned as distinct `RequestReadError`
/// variants.
///
/// This reader relies on `Content-Length` for body framing. It does not decode chunked
/// transfer bodies. It also does not preserve pipelined requests because every response
/// advertises `Connection: close` and the connection is closed after the first response.
///
/// Internal examples:
/// - Headers ending in `\r\n\r\n` with no content length form a zero-length body request.
/// - A request declaring `Content-Length: 5` is returned only after five body bytes have arrived.
/// - A partial request followed by EOF is an error; an immediate EOF is `Ok(None)`.
fn read_http_request(client: &mut TcpStream) -> Result<Option<Vec<u8>>, RequestReadError> {
    let mut request = Vec::with_capacity(HTTP_BUFFER.len() * 2);
    let mut buffer = HTTP_BUFFER;
    let mut headers_complete = false;
    let mut expected_length: Option<usize> = None;
    let started = Instant::now();
    let timeout = Duration::from_millis(HTTP_REQUEST_TIMEOUT_MS);

    loop {
        // Once the header is known, read no more than the remaining request bytes.
        let read_limit = match expected_length {
            Some(length) => length.saturating_sub(request.len()).min(buffer.len()),
            None => buffer.len(),
        };

        if read_limit == 0 {
            return Ok(Some(request));
        }

        // Use one deadline for the whole request, not a fresh timeout per byte.
        let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
            return Err(RequestReadError::TimedOut);
        };

        if remaining.is_zero() {
            return Err(RequestReadError::TimedOut);
        }

        client
            .set_read_timeout(Some(remaining))
            .map_err(|_| RequestReadError::Io)?;

        let bytes_read = match client.read(&mut buffer[..read_limit]) {
            Ok(0) => {
                if request.is_empty() {
                    return Ok(None);
                }

                return Err(RequestReadError::BadRequest);
            }
            Ok(bytes_read) => bytes_read,
            Err(error)
                if error.kind() == io::ErrorKind::TimedOut
                    || error.kind() == io::ErrorKind::WouldBlock =>
            {
                return Err(RequestReadError::TimedOut);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(RequestReadError::Io),
        };

        request.extend_from_slice(&buffer[..bytes_read]);

        // Locate and validate the header block once it has arrived.
        if !headers_complete {
            if let Some(end) = find_bytes(&request, HTTP_HEADER_END) {
                if end > MAX_HTTP_HEADER_SIZE {
                    return Err(RequestReadError::HeaderTooLarge);
                }

                let header = &request[..end];
                let (content_length, has_transfer_encoding) = request_body_metadata(header)?;

                if has_transfer_encoding {
                    return Err(RequestReadError::NotImplemented);
                }

                if content_length > MAX_HTTP_BODY_SIZE {
                    return Err(RequestReadError::BodyTooLarge);
                }

                let total_length = end + HTTP_HEADER_END.len() + content_length;
                headers_complete = true;
                expected_length = Some(total_length);
            } else if request.len() > MAX_HTTP_HEADER_SIZE {
                return Err(RequestReadError::HeaderTooLarge);
            }
        }

        // Return exactly one request. Extra pipelined bytes are ignored because
        // Blank sends Connection: close for every response.
        if let Some(total_length) = expected_length
            && request.len() >= total_length
        {
            request.truncate(total_length);
            return Ok(Some(request));
        }
    }
}

/// Writes all response bytes before one absolute write deadline expires.
///
/// TCP writes may complete only part of the supplied buffer, so this helper tracks the
/// number of bytes already written and continues with the remaining slice. It retries
/// interrupted writes, treats a zero-byte write as `WriteZero`, and returns other I/O
/// errors to its caller. Before each write it updates the socket's write timeout to the
/// remaining portion of one deadline measured from the start of the operation. A slow
/// client therefore cannot extend the total write period indefinitely by making small
/// amounts of progress.
///
/// Internal examples:
/// - A sequence of partial writes is combined until the entire response has been sent.
/// - An interrupted write is retried without advancing the byte counter.
/// - If the absolute deadline expires, the helper returns a timed-out I/O error.
fn write_all_timeout(client: &mut TcpStream, content: &[u8]) -> io::Result<()> {
    let started = Instant::now();
    let timeout = Duration::from_millis(HTTP_WRITE_TIMEOUT_MS);
    let mut written = 0;

    while written < content.len() {
        let remaining = timeout
            .checked_sub(started.elapsed())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))?;

        client.set_write_timeout(Some(remaining))?;

        match client.write(&content[written..]) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
            Ok(bytes_written) => written += bytes_written,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }

    Ok(())
}

/// Extracts the request body's declared length and transfer-encoding presence from headers.
///
/// The byte slice must contain the header section without the terminating blank line.
/// It is interpreted as UTF-8 and each field line after the request line must contain a
/// colon separator. Header names are compared case-insensitively after trimming. At
/// most one `Content-Length` field is accepted; its value must parse as `usize`. A
/// non-empty `Transfer-Encoding` field is recorded so callers can reject transfer
/// encodings that are not implemented. If no content length exists, the body length is
/// treated as zero.
///
/// The function does not read bytes from the socket and does not itself enforce the
/// maximum body limit; callers apply that limit using the returned length.
///
/// Internal examples:
/// - A single `Content-Length: 12` returns `(12, false)`.
/// - No Content-Length returns a body length of zero.
/// - Duplicate Content-Length fields, invalid numbers, malformed lines, or non-UTF-8 headers fail.
/// - A non-empty Transfer-Encoding field sets the boolean result to true.
fn request_body_metadata(header: &[u8]) -> Result<(usize, bool), RequestReadError> {
    let header_text = std::str::from_utf8(header).map_err(|_| RequestReadError::BadRequest)?;
    let mut content_length = None;
    let mut has_transfer_encoding = false;

    for line in header_text.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            return Err(RequestReadError::BadRequest);
        };

        let name = name.trim();
        let value = value.trim();

        if name.eq_ignore_ascii_case(HTTP_CONTENT_LENGTH_HEADER) {
            if content_length.is_some() {
                return Err(RequestReadError::BadRequest);
            }

            content_length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| RequestReadError::BadRequest)?,
            );
        }

        if name.eq_ignore_ascii_case(HTTP_TRANSFER_ENCODING_HEADER) && !value.is_empty() {
            has_transfer_encoding = true;
        }
    }

    Ok((content_length.unwrap_or(0), has_transfer_encoding))
}

/// Finds the first start index of `needle` within `haystack`.
///
/// The search operates on bytes, so it can locate HTTP delimiters and multipart
/// boundaries without first converting payloads to text. The returned index is the
/// offset of the first matching window. An empty needle is deliberately treated as
/// not found, and a needle longer than the haystack also returns `None`.
///
/// Internal examples:
/// - Searching `b"abc--boundary"` for `b"--"` returns `Some(3)`.
/// - Searching for a missing delimiter, an empty delimiter, or a longer byte sequence returns `None`.
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Internal wrapper for a request that has passed HTTP parsing and validation.
///
/// This wrapper keeps the parser's result type distinct from the raw request buffer and
/// leaves room for internal parsing metadata to be added without changing the parser's
/// call sites. At present, it contains only the fully populated [`BlankReq`] that will be
/// passed to the route view. It is not part of the public API.
struct ParsedRequest {
    /// Fully parsed request data, ready for immutable access by a view function.
    request: BlankReq,
}

/// Validates and parses a complete HTTP request buffer into Blank's request model.
///
/// The function expects the complete request bytes returned by `read_http_request`,
/// including the request line, header terminator, and the declared body. It first
/// locates the header boundary and decodes only the header section as UTF-8. The
/// request line must contain exactly three whitespace-separated parts: method, target,
/// and HTTP version. The version must be in the supported-version list. Methods in the
/// known HTTP-method list but outside the supported set are reported as not implemented;
/// unknown methods are reported as method not allowed. The request target must be an
/// origin-form path beginning with `/`.
///
/// Header lines must contain a name/value separator. Names are validated and
/// lowercased, values are trimmed, and repeated names accumulate multiple values. The
/// body framing metadata is checked again here so this parser remains guarded when
/// called directly from tests or other internal code. The body must contain at least
/// the declared Content-Length bytes and must not exceed the maximum body size.
///
/// After the raw body is sliced, the target is split into a path and an optional query
/// string. Query data is parsed with the URL-encoded parser. Form data is parsed only
/// when the first Content-Type value identifies URL-encoded form data or multipart
/// form data. Other content types leave the form map empty. The result contains a
/// `BlankReq` whose raw body, headers, route, query, and forms are consistent with the
/// same request buffer.
///
/// The error is a static HTTP status value, allowing `handle_connection` to choose the
/// short response body without allocating an error message.
///
/// Internal examples:
/// - A valid `GET /items?page=2 HTTP/1.1` produces route `/items` and query key `page`.
/// - A repeated header such as two `Accept` lines creates one lowercase key with two values.
/// - A target not beginning with `/`, unsupported version, malformed header, or incomplete body is rejected.
fn parse_http_request(request: &[u8]) -> Result<ParsedRequest, &'static str> {
    // Find the end of the HTTP headers.
    let Some(header_end) = find_bytes(request, HTTP_HEADER_END) else {
        return Err(HTTP_BAD_REQUEST);
    };

    // Convert the header bytes into text.
    let header_text = std::str::from_utf8(&request[..header_end]).map_err(|_| HTTP_BAD_REQUEST)?;

    // Parse the request line and all following header lines.
    let mut lines = header_text.lines();

    // The first line contains method, route, and version.
    let Some(line) = lines.next() else {
        return Err(HTTP_BAD_REQUEST);
    };

    let mut parts = line.split_whitespace();

    let (Some(method), Some(target), Some(version)) = (parts.next(), parts.next(), parts.next())
    else {
        return Err(HTTP_BAD_REQUEST);
    };

    // A request line must contain exactly three parts.
    if parts.next().is_some() {
        return Err(HTTP_BAD_REQUEST);
    }

    // Check whether the HTTP version is supported.
    if !SUPPORTED_HTTP_VERSIONS.contains(&version) {
        return Err(HTTP_VERSION_NOT_SUPPORTED);
    }

    // Keep the existing Blank method handling.
    // Known but not implemented methods return 501.
    if !SUPPORTED_HTTP_METHODS.contains(&method) {
        if REAL_HTTP_METHODS.contains(&method) {
            return Err(HTTP_NOT_IMPLEMENTED);
        }

        return Err(HTTP_METHOD_NOT_ALLOWED);
    }

    // Routes must be origin-form paths beginning with a slash.
    if !target.starts_with('/') {
        return Err(HTTP_BAD_REQUEST);
    }

    // Store every request header, including repeated header names.
    let mut headers: BlankHeaders = HashMap::new();

    for line in lines {
        // Ignore empty lines inside the parsed header section.
        if line.trim().is_empty() {
            continue;
        }

        // Every header must contain a name and a value.
        let Some((name, value)) = line.split_once(':') else {
            return Err(HTTP_BAD_REQUEST);
        };

        let name = name.trim();
        let value = value.trim();

        // Reject malformed header names and line breaks in values.
        if !is_valid_header_name(name) || !is_valid_header_value(value) {
            return Err(HTTP_BAD_REQUEST);
        }

        headers
            .entry(name.to_ascii_lowercase())
            .or_default()
            .push(value.to_string());
    }

    // The body length is checked again while parsing so this function
    // remains safe when it is called independently in tests.
    let (content_length, has_transfer_encoding) =
        request_body_metadata(&request[..header_end]).map_err(|error| error.response().0)?;

    if has_transfer_encoding {
        return Err(HTTP_NOT_IMPLEMENTED);
    }

    if content_length > MAX_HTTP_BODY_SIZE {
        return Err(HTTP_PAYLOAD_TOO_LARGE);
    }

    let body_start = header_end + HTTP_HEADER_END.len();
    let body_end = body_start
        .checked_add(content_length)
        .ok_or(HTTP_BAD_REQUEST)?;

    if request.len() < body_end {
        return Err(HTTP_BAD_REQUEST);
    }

    let body = request[body_start..body_end].to_vec();

    // The query string is not part of the route registry key.
    let (path, query_string) = target.split_once('?').unwrap_or((target, ""));
    let query = parse_urlencoded(query_string.as_bytes()).map_err(|_| HTTP_BAD_REQUEST)?;

    // Forms are parsed according to the request's content type.
    let mut forms = BlankForms::new();

    if let Some(content_type) = headers
        .get(HTTP_CONTENT_TYPE_HEADER)
        .and_then(|values| values.first())
    {
        let media_type = content_type.split(';').next().unwrap_or("").trim();

        if media_type.eq_ignore_ascii_case(CONTENT_TYPE_FORM_URLENCODED) {
            forms = parse_urlencoded(&body).map_err(|_| HTTP_BAD_REQUEST)?;
        } else if media_type.eq_ignore_ascii_case(CONTENT_TYPE_MULTIPART_FORM_DATA) {
            forms = parse_multipart_form(&body, content_type).map_err(|_| HTTP_BAD_REQUEST)?;
        }
    }

    Ok(ParsedRequest {
        request: BlankReq {
            method: method.to_string(),
            route: path.to_string(),
            version: version.to_string(),
            headers,
            body,
            query,
            forms,
        },
    })
}

/// Parses an `application/x-www-form-urlencoded` byte sequence into a multi-value map.
///
/// The input is split on ampersands into field pairs. Each pair is split at its first
/// equals sign; a pair without an equals sign represents a key with an empty value.
/// Empty pairs are ignored. Both key and value are URL-decoded, then inserted through
/// `insert_form_value`, which enforces non-empty names, a maximum value size, and a
/// maximum number of distinct fields. Repeated names append another value instead of
/// replacing the previous one.
///
/// Invalid percent escapes, incomplete escape sequences, decoded non-UTF-8 data, empty
/// field names, and limit violations return `Err(())`. Empty input is valid and yields
/// an empty map. The same parser is used for query strings and URL-encoded request
/// bodies; the alias name `BlankForms` is also used as the internal map type for query
/// parsing.
///
/// Internal examples:
/// - `name=Kyam&city=Hyderabad` creates two single-value entries.
/// - `tag=rust&tag=web` stores two values under `tag`.
/// - `flag` stores an empty value for `flag`.
/// - `q=hello+world` decodes to `hello world`.
fn parse_urlencoded(input: &[u8]) -> Result<BlankForms, ()> {
    let mut values = BlankForms::new();

    if input.is_empty() {
        return Ok(values);
    }

    for pair in input.split(|byte| *byte == b'&') {
        if pair.is_empty() {
            continue;
        }

        let (name, value) = match pair.iter().position(|byte| *byte == b'=') {
            Some(index) => (&pair[..index], &pair[index + 1..]),
            None => (pair, &[][..]),
        };

        let name = url_decode(name).ok_or(())?;
        let value = url_decode(value).ok_or(())?;
        insert_form_value(&mut values, name, value)?;
    }

    Ok(values)
}

/// Decodes one URL-encoded component and requires the decoded bytes to be UTF-8.
///
/// A plus byte is converted to a space. A percent sign must be followed by two valid
/// hexadecimal digits; those digits are combined into one decoded byte. All other input
/// bytes are copied unchanged. After the scan, the resulting byte vector is converted
/// into a `String`. The function returns `None` for an incomplete or invalid percent
/// escape, a non-hexadecimal escape digit, or bytes that do not form valid UTF-8.
///
/// This is the component-level decoder used for both parameter names and values by
/// `parse_urlencoded`; it does not split pairs or handle ampersand/equal separators.
///
/// Internal examples:
/// - `b"hello+world"` becomes `Some("hello world")`.
/// - `b"%2Fitems"` becomes `Some("/items")`.
/// - `b"%Q0"` and an incomplete trailing percent escape return `None`.
fn url_decode(input: &[u8]) -> Option<String> {
    let mut decoded = Vec::with_capacity(input.len());
    let mut index = 0;

    while index < input.len() {
        match input[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' => {
                let high = *input.get(index + 1)?;
                let low = *input.get(index + 2)?;
                decoded.push((hex_value(high)? << 4) | hex_value(low)?);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8(decoded).ok()
}

/// Converts one ASCII hexadecimal digit into its numeric nibble value.
///
/// Decimal digits map to zero through nine, lowercase and uppercase letters `a` through
/// `f` map to ten through fifteen, and every other byte returns `None`. URL decoding
/// calls this helper twice for each percent escape and combines the two nibbles into a
/// decoded byte. The function does not accept prefixes such as `0x` or multibyte text.
///
/// Internal examples:
/// - `b'0'` maps to `Some(0)` and `b'9'` maps to `Some(9)`.
/// - `b'a'` and `b'F'` map to `Some(10)` and `Some(15)` respectively.
/// - `b'G'` and punctuation return `None`.
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Inserts one decoded field value while enforcing the configured form limits.
///
/// A field name must not be empty. The UTF-8 byte length of its value must not exceed
/// `MAX_FORM_FIELD_SIZE`. When the name is new, the number of distinct keys must remain
/// below `MAX_FORM_FIELDS`; repeated occurrences of an existing key do not consume an
/// additional distinct-field slot. On success, the value is appended to the vector for
/// that name, preserving repeated-field order. Any rejected input returns `Err(())`
/// without inserting the rejected value.
///
/// Internal examples:
/// - Inserting `("tag", "rust")` followed by `("tag", "web")` gives two values under one key.
/// - Inserting a different name increases the number of distinct fields by one.
/// - An empty name, oversized value, or new key after the distinct-field limit is reached fails.
fn insert_form_value(forms: &mut BlankForms, name: String, value: String) -> Result<(), ()> {
    if name.is_empty() || value.len() > MAX_FORM_FIELD_SIZE {
        return Err(());
    }

    if !forms.contains_key(&name) && forms.len() >= MAX_FORM_FIELDS {
        return Err(());
    }

    forms.entry(name).or_default().push(value);
    Ok(())
}

/// Parses text fields from a `multipart/form-data` body.
///
/// The boundary is taken from the Content-Type parameter named `boundary`; surrounding
/// double quotes are removed from the parameter value. The parser searches the body for
/// each boundary marker, separates the part headers from the part payload, and reads the
/// `Content-Disposition` value to identify the field name. A part with a `filename`
/// parameter is treated as a file part and skipped. A text part must provide a `name`
/// parameter and contain valid UTF-8 text; the resulting field is inserted through the
/// same size and distinct-field limits used by URL-encoded data.
///
/// This is a text-field parser, not a general file-upload implementation. File bytes
/// are deliberately not returned, and part content is required to be UTF-8. Malformed
/// boundaries, missing disposition/name data for text parts, invalid UTF-8, or form
/// limit violations return `Err(())`.
///
/// Internal examples:
/// - A multipart part named `username` with a UTF-8 value is added to the returned map.
/// - A part whose disposition includes `filename="photo.png"` is skipped.
/// - A Content-Type without a non-empty boundary is rejected.
fn parse_multipart_form(body: &[u8], content_type: &str) -> Result<BlankForms, ()> {
    let boundary = content_type
        .split(';')
        .skip(1)
        .find_map(|parameter| {
            let (name, value) = parameter.trim().split_once('=')?;
            if name
                .trim()
                .eq_ignore_ascii_case(MULTIPART_BOUNDARY_PARAMETER)
            {
                Some(value.trim().trim_matches('"').to_string())
            } else {
                None
            }
        })
        .filter(|boundary| !boundary.is_empty())
        .ok_or(())?;

    let marker = format!("{}{}", MULTIPART_BOUNDARY_PREFIX, boundary).into_bytes();
    let mut forms = BlankForms::new();
    let mut cursor = 0;

    loop {
        let marker_start = find_bytes(&body[cursor..], &marker)
            .map(|offset| cursor + offset)
            .ok_or(())?;
        let mut part_start = marker_start + marker.len();

        // The final marker ends with two hyphens.
        if body.get(part_start..part_start + MULTIPART_FINAL_MARKER.len())
            == Some(MULTIPART_FINAL_MARKER)
        {
            break;
        }

        if body.get(part_start..part_start + HTTP_LINE_END.len()) != Some(HTTP_LINE_END) {
            return Err(());
        }

        part_start += HTTP_LINE_END.len();

        let next_marker = find_bytes(&body[part_start..], &marker)
            .map(|offset| part_start + offset)
            .ok_or(())?;

        if next_marker < part_start + 2 {
            return Err(());
        }

        // The CRLF immediately before the next boundary is not field content.
        let part_end = next_marker - 2;
        let part = &body[part_start..part_end];
        let header_end = find_bytes(part, HTTP_HEADER_END).ok_or(())?;
        let part_headers = std::str::from_utf8(&part[..header_end]).map_err(|_| ())?;
        let disposition = part_headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case(HTTP_CONTENT_DISPOSITION_HEADER)
                    .then_some(value.trim())
            })
            .ok_or(())?;

        // File parts need a dedicated binary upload API, so they are skipped.
        if !disposition_parameter_exists(disposition, MULTIPART_FILENAME_PARAMETER) {
            let name = disposition_parameter(disposition, MULTIPART_NAME_PARAMETER).ok_or(())?;
            let value = std::str::from_utf8(&part[header_end + HTTP_HEADER_END.len()..])
                .map_err(|_| ())?
                .to_string();
            insert_form_value(&mut forms, name, value)?;
        }

        cursor = next_marker;
    }

    Ok(forms)
}

/// Extracts one semicolon-separated parameter from a Content-Disposition value.
///
/// The helper ignores the first segment, which normally contains the disposition type,
/// then scans the remaining segments for a `name=value` pair whose parameter name
/// matches `key` case-insensitively. Whitespace around each segment and around the name
/// and value is trimmed. Surrounding double-quote characters are trimmed from the
/// value. This lightweight parser does not implement quoted-string escaping or a full
/// MIME parameter grammar.
///
/// Internal examples:
/// - `form-data; name="username"` yields `username` for key `name`.
/// - `form-data; filename=avatar.png` yields `avatar.png` for key `filename`.
/// - A missing parameter or segment without `=` returns `None`.
fn disposition_parameter(disposition: &str, key: &str) -> Option<String> {
    disposition.split(';').skip(1).find_map(|parameter| {
        let (name, value) = parameter.trim().split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case(key)
            .then(|| value.trim().trim_matches('"').to_string())
    })
}

/// Reports whether `disposition_parameter` can extract the requested parameter.
///
/// This is a convenience predicate used to distinguish multipart file parts from text
/// fields. It delegates parsing to `disposition_parameter`, so its matching is
/// case-insensitive for the parameter name and follows the same simple semicolon-split
/// rules. A present parameter with an empty value still counts as present because the
/// underlying parser returns `Some("")` for it.
///
/// Internal examples:
/// - A disposition containing `filename="report.txt"` returns `true` for `filename`.
/// - A disposition containing only `name="notes"` returns `false` for `filename`.
fn disposition_parameter_exists(disposition: &str, key: &str) -> bool {
    disposition_parameter(disposition, key).is_some()
}

/// Processes one accepted TCP connection from request read through response shutdown.
///
/// Each connection runs on a worker thread created by `run_server`. The function first
/// enables TCP_NODELAY and installs read/write timeouts, ignoring configuration errors
/// for those socket options. It then reads one complete request. A client that closes
/// without sending bytes ends the worker quietly; a read failure is mapped to a short
/// HTTP error response, written with the bounded writer, and followed by socket
/// shutdown.
///
/// A successfully read buffer is parsed into `BlankReq`. On parse failure, the function
/// chooses the standard short body for the corresponding status and serializes the
/// error response. On parse success, it creates a default `BlankRes`, looks up the
/// request path in the immutable route snapshot, and invokes the matching function
/// pointer. If no route exists, it configures a 404 response. When the view returns,
/// the response builder is serialized through `http_return!` and written to the client.
/// The connection is shut down after the response whether the Write succeeds or fails.
///
/// Only one request is handled on this connection. This matches the serializer's
/// `Connection: close` behavior and the request reader's handling of a single
/// Content-Length-framed request.
///
/// Internal examples:
/// - A parsed request matching `/health` invokes the `/health` view with request and response references.
/// - A syntactically valid request for an unregistered path receives a 404 response.
/// - A malformed or oversized request is answered before any view is invoked.
fn handle_connection(mut client: TcpStream, routes: Arc<HashMap<String, BlankView>>) {
    // Let the operating system optimize small request/response exchanges.
    let _ = client.set_nodelay(true);
    let _ = client.set_read_timeout(Some(Duration::from_millis(HTTP_REQUEST_TIMEOUT_MS)));
    let _ = client.set_write_timeout(Some(Duration::from_millis(HTTP_WRITE_TIMEOUT_MS)));

    // Read the HTTP request without polling or holding the listener thread.
    let request = match read_http_request(&mut client) {
        Ok(Some(request)) => request,
        Ok(None) => return,
        Err(error) => {
            let (status, body) = error.response();
            let response = http_return!(
                status,
                body.as_bytes(),
                CONTENT_TYPE_TEXT,
                &HashMap::<String, Vec<String>>::new()
            );

            let _ = write_all_timeout(&mut client, &response);
            let _ = client.shutdown(Shutdown::Both);
            return;
        }
    };

    // Parse the request into Blank's request representation.
    let response = match parse_http_request(&request) {
        Ok(request) => {
            // Create an output object for the selected view.
            let mut out = BlankRes::new();

            // Find the view registered for this route.
            if let Some(view) = routes.get(&request.request.route) {
                // Run the view with the complete request and mutable response.
                view(&request.request, &mut out);
            } else {
                // The requested route does not exist.
                out.status(HTTP_NOT_FOUND).text(BODY_NOT_FOUND);
            }

            // Convert the response builder into a valid HTTP response.
            http_return!(
                out.status,
                out.body.content(),
                out.response_content_type(),
                &out.headers
            )
        }
        Err(status) => {
            // Keep error bodies short and readable for now.
            let body = match status {
                HTTP_BAD_REQUEST => BODY_BAD_REQUEST,
                HTTP_VERSION_NOT_SUPPORTED => BODY_HTTP_VERSION_NOT_SUPPORTED,
                HTTP_NOT_IMPLEMENTED => BODY_NOT_IMPLEMENTED,
                HTTP_METHOD_NOT_ALLOWED => BODY_METHOD_NOT_ALLOWED,
                HTTP_PAYLOAD_TOO_LARGE => BODY_CONTENT_TOO_LARGE,
                _ => BODY_REQUEST_ERROR,
            };

            http_return!(
                status,
                body.as_bytes(),
                CONTENT_TYPE_TEXT,
                &HashMap::<String, Vec<String>>::new()
            )
        }
    };

    // Send the response. A failed client write simply ends this worker.
    let _ = write_all_timeout(&mut client, &response);
    let _ = client.shutdown(Shutdown::Both);
}

/// Constructs a default-host `Blank` configuration from a TCP port value.
///
/// This conversion is a compact alternative to calling `Blank::new()` and then
/// `port(...)`. It uses the same default localhost host and starts with an empty route
/// registry. As with the regular constructor, it only creates configuration; it does
/// not bind the port or start the server.
///
/// # Examples
///
/// ```text
/// use blank_web::{Blank, Port};
/// let app = Blank::from(8080 as Port);
/// assert_eq!(app.get_port(), 8080);
/// assert_eq!(app.get_host(), "localhost");
/// ```
///
/// ```
/// use blank_web::Blank;
/// let app: Blank = 9000u16.into();
/// assert_eq!(app.get_port(), 9000);
/// ```
impl From<Port> for Blank {
    /// Converts a port number into a new server configuration.
    ///
    /// The resulting value uses the default host and an empty route map, just like
    /// `Blank::new()` followed by `port(port)`. This is only a configuration
    /// conversion: it does not reserve the port or start a listener. The operating
    /// system checks the address when one of the serving methods is called.
    ///
    /// Examples:
    /// - `Blank::from(8080u16)` creates a configuration whose requested port is 8080.
    /// - `let app: Blank = 0u16.into()` requests an operating-system-selected port.
    fn from(port: Port) -> Self {
        Self {
            tcp_port: port,
            host: LOCALHOST.to_string(),
            routes: HashMap::new(),
        }
    }
}

/// Makes `Blank::default()` equivalent to [`Blank::new`].
///
/// The default server uses the framework's default port and localhost host and has no
/// registered routes. Like `new`, this is configuration-only construction; it does not
/// open a socket or spawn a thread.
///
/// # Examples
///
/// ```
/// use blank_web::Blank;
/// let app = Blank::default();
/// assert_eq!(app.get_port(), 7600);
/// assert_eq!(app.route_count(), 0);
/// ```
///
/// ```
/// use blank_web::{Blank, BlankReq, BlankRes};
/// fn index(_: &BlankReq, out: &mut BlankRes) { out.text("default"); }
/// let mut app = Blank::default();
/// app.route_func("/", index);
/// assert!(app.has_route("/"));
/// ```
impl Default for Blank {
    /// Returns the same configuration as [`Blank::new`].
    ///
    /// The value contains the default host and port with no registered routes. No
    /// network operation occurs during default construction.
    ///
    /// Examples: `Blank::default()` is ready for configuration, and
    /// `Blank::default().get_port()` reads its configured default port.
    fn default() -> Self {
        Self::new()
    }
}

// The shared test macro is expanded inside this child module so its tests can access
// private implementation details without exposing those details in Blank's public API.
/// Module-local test entry point for the server implementation.
///
/// The shared `server_tests!` macro is expanded inside this module so its test code can
/// access private fields, constructors, parsing helpers, and implementation details
/// that are intentionally unavailable to applications. These tests exercise internal
/// behavior without making those implementation details part of Blank's public API.
#[cfg(test)]
mod tests {
    use crate::server_tests;

    server_tests!();
}

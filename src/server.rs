// server.rs
//! This file manages Blank objects.
//!
//! Blank objects are basically servers.
//!
//! Implements all related traits and methods as well.
//! Blank can be used to create a server, register routes,
//! bind it to a TCP port, and listen for requests.
//!
//! The server uses blocking OS socket readiness and bounded
//! connection workers. It does not repeatedly poll idle sockets.

// Constants
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

// Imports
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// An Alias of an u16 that is to make Ports.
/// Port is basically a TCP port.
pub type Port = u16;

/// An alias of String.
pub type Host = String;

/// An alias for request headers.
pub type BlankHeaders = HashMap<String, Vec<String>>;

/// An alias for parsed query-string values.
pub type BlankQuery = HashMap<String, Vec<String>>;

/// An alias for parsed HTML form values.
pub type BlankForms = HashMap<String, Vec<String>>;

/// A parsed HTTP request available to every Blank view.
///
/// Header names are stored in lowercase. Repeated headers, query
/// parameters, and form fields are stored as multiple values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlankReq {
    /// HTTP method, such as GET or POST.
    method: String,

    /// Request path without its query string.
    route: String,

    /// HTTP version used by the client.
    version: String,

    /// All request headers.
    headers: BlankHeaders,

    /// Raw request body.
    body: Vec<u8>,

    /// Parsed URL query parameters.
    query: BlankQuery,

    /// Parsed URL-encoded or multipart form fields.
    forms: BlankForms,
}

impl Deref for BlankReq {
    type Target = BlankHeaders;

    fn deref(&self) -> &Self::Target {
        &self.headers
    }
}

impl BlankReq {
    /// Returns the HTTP method.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Returns the request path without its query string.
    pub fn route(&self) -> &str {
        &self.route
    }

    /// Returns the HTTP version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns every request header.
    pub fn headers(&self) -> &BlankHeaders {
        &self.headers
    }

    /// Returns the first value for a request header.
    ///
    /// Header names are case-insensitive.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    /// Returns every value for a request header.
    ///
    /// Header names are case-insensitive.
    pub fn header_values(&self, name: &str) -> Option<&[String]> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(Vec::as_slice)
    }

    /// Returns the raw request body.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Returns the request body as UTF-8 text.
    pub fn body_text(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.body)
    }

    /// Returns the request content type, if one was supplied.
    pub fn content_type(&self) -> Option<&str> {
        self.header(HTTP_CONTENT_TYPE_HEADER)
    }

    /// Returns every parsed query parameter.
    pub fn query(&self) -> &BlankQuery {
        &self.query
    }

    /// Returns the first value for a query parameter.
    pub fn query_value(&self, name: &str) -> Option<&str> {
        self.query
            .get(name)
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    /// Returns every value for a query parameter.
    pub fn query_values(&self, name: &str) -> Option<&[String]> {
        self.query.get(name).map(Vec::as_slice)
    }

    /// Returns every parsed form field.
    pub fn forms(&self) -> &BlankForms {
        &self.forms
    }

    /// Returns the first value for a form field.
    pub fn form(&self, name: &str) -> Option<&str> {
        self.forms
            .get(name)
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    /// Returns every value for a form field.
    pub fn form_values(&self, name: &str) -> Option<&[String]> {
        self.forms.get(name).map(Vec::as_slice)
    }

    /// Returns true when this request uses the POST method.
    pub fn is_post(&self) -> bool {
        self.method == HTTP_METHOD_POST
    }

    /// Returns true when this request uses the GET method.
    pub fn is_get(&self) -> bool {
        self.method == HTTP_METHOD_GET
    }
}

/// An alias of a Blank view function.
///
/// The request is immutable while the response can be modified.
pub type BlankView = fn(rq: &BlankReq, out: &mut BlankRes);

// A macro for creating Blank output types.
// Each output type stores a hardcoded string for now.
macro_rules! blank_output {
    ($name:ident, $content_type:expr) => {
        /// A response body with its own content type.
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            // The actual response content.
            content: String,
        }

        impl $name {
            /// Creates a new output value.
            pub fn new(content: impl Into<String>) -> Self {
                Self {
                    content: content.into(),
                }
            }

            /// Returns the response content as a string slice.
            pub fn content(&self) -> &str {
                &self.content
            }

            /// Returns the response content type.
            pub const CONTENT_TYPE: &'static str = $content_type;
        }
    };
}

// Output types
blank_output!(BlankJSON, CONTENT_TYPE_JSON);
blank_output!(BlankHTML, CONTENT_TYPE_HTML);
blank_output!(BlankText, CONTENT_TYPE_TEXT);

/// The body of a Blank response.
#[derive(Debug, Clone, PartialEq, Eq)]
enum BlankBody {
    Json(BlankJSON),
    Html(BlankHTML),
    Text(BlankText),
    Binary(Vec<u8>),
}

impl BlankBody {
    /// Returns the body as bytes.
    fn content(&self) -> &[u8] {
        match self {
            Self::Json(content) => content.content().as_bytes(),
            Self::Html(content) => content.content().as_bytes(),
            Self::Text(content) => content.content().as_bytes(),
            Self::Binary(content) => content,
        }
    }

    /// Returns the default content type for this body.
    fn content_type(&self) -> &'static str {
        match self {
            Self::Json(_) => BlankJSON::CONTENT_TYPE,
            Self::Html(_) => BlankHTML::CONTENT_TYPE,
            Self::Text(_) => BlankText::CONTENT_TYPE,
            Self::Binary(_) => CONTENT_TYPE_OCTET_STREAM,
        }
    }
}

/// The response returned by a Blank view.
///
/// Views can modify the body, status, and headers before
/// Blank serializes the response and sends it to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlankRes {
    /// HTTP response status.
    status: &'static str,

    /// Response body.
    body: BlankBody,

    /// Additional response headers.
    headers: BlankHeaders,

    /// An optional custom content type.
    content_type: Option<String>,
}

impl BlankRes {
    /// Creates a new Blank response.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a JSON response.
    ///
    /// Accepts a JSON string for now. Serialization of Rust values
    /// can be added later without changing the response structure.
    pub fn json(&mut self, content: impl Into<String>) -> &mut Self {
        self.body = BlankBody::Json(BlankJSON::new(content));
        self.content_type = None;
        self
    }

    /// Sets an HTML response.
    ///
    /// Accepts an HTML string for now.
    pub fn html(&mut self, content: impl Into<String>) -> &mut Self {
        self.body = BlankBody::Html(BlankHTML::new(content));
        self.content_type = None;
        self
    }

    /// Sets a plain-text response.
    pub fn text(&mut self, content: impl Into<String>) -> &mut Self {
        self.body = BlankBody::Text(BlankText::new(content));
        self.content_type = None;
        self
    }

    /// Sets a binary response.
    pub fn binary(&mut self, content: impl Into<Vec<u8>>) -> &mut Self {
        self.body = BlankBody::Binary(content.into());
        self.content_type = Some(CONTENT_TYPE_OCTET_STREAM.to_string());
        self
    }

    /// Sets the HTTP response status.
    pub fn status(&mut self, status: &'static str) -> &mut Self {
        self.status = status;
        self
    }

    /// Sets a successful 200 response.
    pub fn ok(&mut self) -> &mut Self {
        self.status(HTTP_OK)
    }

    /// Sets a 201 Created response.
    pub fn created(&mut self) -> &mut Self {
        self.status(HTTP_CREATED)
    }

    /// Sets a 204 No Content response and clears the body.
    pub fn no_content(&mut self) -> &mut Self {
        self.status(HTTP_NO_CONTENT).text("")
    }

    /// Sets a 400 Bad Request response.
    pub fn bad_request(&mut self) -> &mut Self {
        self.status(HTTP_BAD_REQUEST).text(BODY_BAD_REQUEST)
    }

    /// Sets a 401 Unauthorized response.
    pub fn unauthorized(&mut self) -> &mut Self {
        self.status(HTTP_UNAUTHORIZED).text(BODY_UNAUTHORIZED)
    }

    /// Sets a 403 Forbidden response.
    pub fn forbidden(&mut self) -> &mut Self {
        self.status(HTTP_FORBIDDEN).text(BODY_FORBIDDEN)
    }

    /// Sets a 404 Not Found response.
    pub fn not_found(&mut self) -> &mut Self {
        self.status(HTTP_NOT_FOUND).text(BODY_NOT_FOUND)
    }

    /// Sets a 405 Method Not Allowed response.
    pub fn method_not_allowed(&mut self) -> &mut Self {
        self.status(HTTP_METHOD_NOT_ALLOWED)
            .text(BODY_METHOD_NOT_ALLOWED)
    }

    /// Sets a 408 Request Timeout response.
    pub fn request_timeout(&mut self) -> &mut Self {
        self.status(HTTP_REQUEST_TIMEOUT).text(BODY_REQUEST_TIMEOUT)
    }

    /// Sets a 500 Internal Server Error response.
    pub fn internal_server_error(&mut self) -> &mut Self {
        self.status(HTTP_INTERNAL_SERVER_ERROR)
            .text(BODY_INTERNAL_SERVER_ERROR)
    }

    /// Sets a 301 Moved Permanently response and its location.
    pub fn permanent_redirect(&mut self, location: impl Into<String>) -> &mut Self {
        let location = location.into();

        if !is_valid_header_value(&location) {
            return self;
        }

        self.status(HTTP_MOVED_PERMANENTLY)
            .set_header(HTTP_RESPONSE_LOCATION, location)
            .text("")
    }

    /// Sets a 413 Content Too Large response.
    pub fn payload_too_large(&mut self) -> &mut Self {
        self.status(HTTP_PAYLOAD_TOO_LARGE)
            .text(BODY_CONTENT_TOO_LARGE)
    }

    /// Sets a 429 Too Many Requests response.
    pub fn too_many_requests(&mut self) -> &mut Self {
        self.status(HTTP_TOO_MANY_REQUESTS)
            .text(BODY_TOO_MANY_REQUESTS)
    }

    /// Sets a 502 Bad Gateway response.
    pub fn bad_gateway(&mut self) -> &mut Self {
        self.status(HTTP_BAD_GATEWAY).text(BODY_BAD_GATEWAY)
    }

    /// Sets a 504 Gateway Timeout response.
    pub fn gateway_timeout(&mut self) -> &mut Self {
        self.status(HTTP_GATEWAY_TIMEOUT).text(BODY_GATEWAY_TIMEOUT)
    }

    /// Sets a 503 Service Unavailable response.
    pub fn service_unavailable(&mut self) -> &mut Self {
        self.status(HTTP_SERVICE_UNAVAILABLE)
            .text(BODY_SERVICE_UNAVAILABLE)
    }

    /// Overrides the response content type.
    pub fn content_type(&mut self, content_type: impl Into<String>) -> &mut Self {
        let content_type = content_type.into();

        // Reject line breaks so a content type cannot inject headers.
        if !content_type.contains('\r') && !content_type.contains('\n') {
            self.content_type = Some(content_type);
        }

        self
    }

    /// Adds a response header.
    ///
    /// Header names are normalized to lowercase. Invalid names or
    /// values are ignored to prevent header injection.
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

    /// Alias for `header`, matching the plural helper style.
    pub fn headers(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.header(name, value)
    }

    /// Replaces all values for a response header.
    pub fn set_header(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        let name = name.into();
        let value = value.into();

        if !is_valid_header_name(&name) || !is_valid_header_value(&value) {
            return self;
        }

        self.headers.insert(name.to_ascii_lowercase(), vec![value]);
        self
    }

    /// Removes a response header.
    pub fn remove_header(&mut self, name: &str) -> &mut Self {
        self.headers.remove(&name.to_ascii_lowercase());
        self
    }

    /// Returns all values for a response header.
    pub fn header_values(&self, name: &str) -> Option<&[String]> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(Vec::as_slice)
    }

    /// Adds a simple cookie to the response.
    ///
    /// Defaults to Path=/, HttpOnly, and SameSite=Lax.
    /// The caller can use `header("Set-Cookie", ...)` for
    /// more advanced cookie attributes.
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

    /// Expires a cookie in the browser.
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

    /// Redirects the client to another location.
    pub fn redirect(&mut self, location: impl Into<String>) -> &mut Self {
        let location = location.into();

        if !is_valid_header_value(&location) {
            return self;
        }

        self.status(HTTP_FOUND)
            .set_header(HTTP_RESPONSE_LOCATION, location)
            .text("")
    }

    /// Returns the response body as bytes.
    pub fn body(&self) -> &[u8] {
        self.body.content()
    }

    /// Returns the response body content type.
    pub fn response_content_type(&self) -> &str {
        self.content_type
            .as_deref()
            .unwrap_or_else(|| self.body.content_type())
    }
}

impl Default for BlankRes {
    fn default() -> Self {
        Self {
            status: HTTP_OK,
            body: BlankBody::Text(BlankText::new("")),
            headers: HashMap::new(),
            content_type: None,
        }
    }
}

/// Checks whether a string is a valid HTTP header name.
fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

/// Checks whether a header value can safely be written to a response.
fn is_valid_header_value(value: &str) -> bool {
    !value.contains('\r') && !value.contains('\n')
}

/// Checks whether a cookie name is safe to put in a Set-Cookie header.
fn is_valid_cookie_name(name: &str) -> bool {
    is_valid_header_name(name)
}

/// Checks whether a cookie value is safe for the simple cookie helper.
fn is_valid_cookie_value(value: &str) -> bool {
    !value
        .bytes()
        .any(|byte| byte <= 0x20 || byte == 0x7f || byte == b';' || byte == b',')
}

// A simple macro for creating the final HTTP response for a client.
macro_rules! http_return {
    (
        // response code
        $res_code:expr,
        // response content
        $content:expr,
        // content type
        $content_type:expr,
        // additional headers
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

/// The core of Blank, a web server that listens on a TCP port.
///
/// The route registry is private. Each route points to a view function
/// that receives a parsed request and fills a response.
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
    /// Returns a new Blank with a default port.
    pub fn new() -> Self {
        Self {
            tcp_port: DEFAULT_PORT,
            host: LOCALHOST.to_string(),
            routes: HashMap::new(),
        }
    }

    /// Sets the host on which Blank will listen.
    pub fn host(&mut self, host: impl Into<String>) -> &mut Self {
        self.host = host.into();
        self
    }

    /// Returns the configured host.
    pub fn get_host(&self) -> &str {
        &self.host
    }

    /// Sets the TCP port on which Blank will listen.
    pub fn port(&mut self, port: Port) -> &mut Self {
        self.tcp_port = port;
        self
    }

    /// Returns the configured TCP port.
    pub fn get_port(&self) -> Port {
        self.tcp_port
    }

    /// Registers a route and its view function.
    ///
    /// Registering an existing route replaces its previous view.
    /// Routes are normalized to start with a slash.
    pub fn route_func(&mut self, route: &str, view: BlankView) -> &mut Self {
        let route = normalize_route(route);
        self.routes.insert(route, view);
        self
    }

    /// Checks whether a route has been registered.
    pub fn has_route(&self, route: &str) -> bool {
        self.routes.contains_key(&normalize_route(route))
    }

    /// Removes a registered route.
    pub fn remove_route(&mut self, route: &str) -> bool {
        self.routes.remove(&normalize_route(route)).is_some()
    }

    /// Removes every registered route.
    pub fn clear_routes(&mut self) -> &mut Self {
        self.routes.clear();
        self
    }

    /// Returns the number of registered routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Runs the server and waits until it stops or encounters an error.
    ///
    /// This is the blocking entry point. It uses blocking `accept()` and
    /// lets the operating system sleep until a connection is ready.
    pub fn serve_await(&self) -> Result<(), BlankError> {
        let server = bind_server(&self.host, self.tcp_port)?;
        run_server(
            server,
            Arc::new(self.routes.clone()),
            Arc::new(AtomicBool::new(false)),
        )
    }

    /// Alias for `serve_await()`.
    ///
    /// `serve()` intentionally has the same blocking behavior.
    pub fn serve(&self) -> Result<(), BlankError> {
        self.serve_await()
    }

    /// Starts the server on a background operating-system thread.
    ///
    /// The returned handle can stop and join that thread with
    /// `BlankServer::async_stop()`.
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

/// A handle to a server started by `Blank::serve_async()`.
///
/// Stopping the handle wakes the blocking listener and joins its thread.
pub struct BlankServer {
    stop: Arc<AtomicBool>,
    address: SocketAddr,
    thread: Option<JoinHandle<Result<(), BlankError>>>,
}

impl BlankServer {
    /// Returns the actual local address selected by the operating system.
    ///
    /// This is especially useful when Blank was configured with port `0`.
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    /// Stops the background server and waits for its thread to finish.
    ///
    /// Active request workers are allowed to finish before the server exits.
    pub fn async_stop(mut self) -> Result<(), BlankError> {
        self.stop_and_join()
    }

    /// Signals the listener, wakes its blocking accept call, and joins it.
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

impl Drop for BlankServer {
    fn drop(&mut self) {
        // Do not leave a server thread running when its handle is dropped.
        let _ = self.stop_and_join();
    }
}

/// Binds a TCP listener to the configured host and port.
fn bind_server(host: &str, port: Port) -> Result<TcpListener, BlankError> {
    TcpListener::bind(format!("{}:{}", host, port)).map_err(|_| ERROR_BIND_SERVER)
}

/// Converts an unspecified bind address to a loopback address for shutdown.
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

/// Runs the listener and dispatches accepted clients to bounded workers.
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
/// Joins workers that have already completed.
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

/// Writes a small overload response and closes the client connection.
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

/// Normalizes a route to begin with a slash.
fn normalize_route(route: &str) -> String {
    if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{}", route)
    }
}

/// Describes why an HTTP request could not be read completely.
enum RequestReadError {
    BadRequest,
    HeaderTooLarge,
    BodyTooLarge,
    NotImplemented,
    TimedOut,
    Io,
}

impl RequestReadError {
    /// Returns the HTTP status and body for this read error.
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

/// Reads one complete HTTP request from a blocking client stream.
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

/// Writes the complete response before one absolute write deadline.
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

/// Reads Content-Length and Transfer-Encoding from a raw header block.
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

/// Finds the first occurrence of a byte sequence.
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// A parsed HTTP request containing the complete request data.
struct ParsedRequest {
    request: BlankReq,
}

/// Parses an HTTP request header block and its body.
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

/// Parses URL-encoded query or form data.
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

/// Decodes one application/x-www-form-urlencoded value.
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

/// Converts one hexadecimal byte into its numeric value.
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Adds one parsed field while enforcing the configured form limits.
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

/// Parses text fields from a multipart/form-data request body.
///
/// File parts are deliberately skipped in this first form implementation.
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

/// Returns a quoted or unquoted parameter from a Content-Disposition value.
fn disposition_parameter(disposition: &str, key: &str) -> Option<String> {
    disposition.split(';').skip(1).find_map(|parameter| {
        let (name, value) = parameter.trim().split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case(key)
            .then(|| value.trim().trim_matches('"').to_string())
    })
}

/// Checks whether a Content-Disposition parameter exists.
fn disposition_parameter_exists(disposition: &str, key: &str) -> bool {
    disposition_parameter(disposition, key).is_some()
}

/// Runs one accepted client connection on its own bounded worker thread.
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

/// from Port or u16.
impl From<Port> for Blank {
    /// Returns a new Blank with specified TCP port.
    ///
    /// port is an u16.
    fn from(port: Port) -> Self {
        Self {
            tcp_port: port,
            host: LOCALHOST.to_string(),
            routes: HashMap::new(),
        }
    }
}

/// Implement Default for fun lol.
impl Default for Blank {
    fn default() -> Self {
        // return what new() returns
        Self::new()
    }
}

// The tests use the macro from tests.rs because it needs access
// to Blank's private fields and constructors.
#[cfg(test)]
mod tests {
    // import the macro test
    use crate::server_tests;

    // Use
    server_tests!();
}

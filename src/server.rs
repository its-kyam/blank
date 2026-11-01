// server.rs
//! This file manages Blank objects.
//!
//! Blank objects are basically servers.
//!
//! Implements all related traits and methods as well.
//! Blank can be used to create a server, register routes,
//! bind it to a TCP port, and listen for requests.
//!
//! The runtime uses Rust futures and a small cooperative
//! executor. It has no external dependencies.

// Constants
use crate::constants::{
    BlankError, CONTENT_TYPE_HTML, CONTENT_TYPE_JSON, CONTENT_TYPE_OCTET_STREAM, CONTENT_TYPE_TEXT,
    DEFAULT_PORT, ERROR_ACCEPT_CONNECTION, ERROR_BIND_SERVER, ERROR_SET_NONBLOCKING,
    HTTP_BAD_GATEWAY, HTTP_BAD_REQUEST, HTTP_BUFFER, HTTP_CREATED, HTTP_FORBIDDEN, HTTP_FOUND,
    HTTP_GATEWAY_TIMEOUT, HTTP_HEADER_END, HTTP_HEADER_FIELDS_TOO_LARGE,
    HTTP_INTERNAL_SERVER_ERROR, HTTP_METHOD_NOT_ALLOWED, HTTP_MOVED_PERMANENTLY, HTTP_NO_CONTENT,
    HTTP_NOT_FOUND, HTTP_NOT_IMPLEMENTED, HTTP_OK, HTTP_PAYLOAD_TOO_LARGE, HTTP_REQUEST_TIMEOUT,
    HTTP_REQUEST_TIMEOUT_MS, HTTP_SERVICE_UNAVAILABLE, HTTP_TOO_MANY_REQUESTS, HTTP_UNAUTHORIZED,
    HTTP_VERSION_NOT_SUPPORTED, HTTP_WRITE_TIMEOUT_MS, LOCALHOST, MAX_ACCEPTS_PER_TICK,
    MAX_ACTIVE_CONNECTIONS, MAX_HTTP_HEADER_SIZE, REAL_HTTP_METHODS, RUNTIME_POLL_INTERVAL_MS,
    SUPPORTED_HTTP_METHODS, SUPPORTED_HTTP_VERSIONS,
};

// Imports
use std::collections::HashMap;
use std::future::Future;
use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::net::{TcpListener, TcpStream};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

/// An Alias of an u16 that is to make Ports.
/// Port is basically a TCP port.
pub type Port = u16;

/// An alias of String.
pub type Host = String;

/// An alias for request headers.
///
/// A real `BlankReq` struct will replace this alias later.
/// Header names are stored in lowercase and repeated headers
/// are stored as multiple values in the same vector.
pub type BlankReq = HashMap<String, Vec<String>>;

/// An alias for response headers.
pub type BlankHeaders = HashMap<String, Vec<String>>;

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
        self.status(HTTP_BAD_REQUEST).text("Bad Request")
    }

    /// Sets a 401 Unauthorized response.
    pub fn unauthorized(&mut self) -> &mut Self {
        self.status(HTTP_UNAUTHORIZED).text("Unauthorized")
    }

    /// Sets a 403 Forbidden response.
    pub fn forbidden(&mut self) -> &mut Self {
        self.status(HTTP_FORBIDDEN).text("Forbidden")
    }

    /// Sets a 404 Not Found response.
    pub fn not_found(&mut self) -> &mut Self {
        self.status(HTTP_NOT_FOUND).text("Not Found")
    }

    /// Sets a 405 Method Not Allowed response.
    pub fn method_not_allowed(&mut self) -> &mut Self {
        self.status(HTTP_METHOD_NOT_ALLOWED)
            .text("Method Not Allowed")
    }

    /// Sets a 408 Request Timeout response.
    pub fn request_timeout(&mut self) -> &mut Self {
        self.status(HTTP_REQUEST_TIMEOUT).text("Request Timeout")
    }

    /// Sets a 500 Internal Server Error response.
    pub fn internal_server_error(&mut self) -> &mut Self {
        self.status(HTTP_INTERNAL_SERVER_ERROR)
            .text("Internal Server Error")
    }

    /// Sets a 301 Moved Permanently response and its location.
    pub fn permanent_redirect(&mut self, location: impl Into<String>) -> &mut Self {
        let location = location.into();

        if !is_valid_header_value(&location) {
            return self;
        }

        self.status(HTTP_MOVED_PERMANENTLY)
            .set_header("Location", location)
            .text("")
    }

    /// Sets a 413 Content Too Large response.
    pub fn payload_too_large(&mut self) -> &mut Self {
        self.status(HTTP_PAYLOAD_TOO_LARGE)
            .text("Content Too Large")
    }

    /// Sets a 429 Too Many Requests response.
    pub fn too_many_requests(&mut self) -> &mut Self {
        self.status(HTTP_TOO_MANY_REQUESTS)
            .text("Too Many Requests")
    }

    /// Sets a 502 Bad Gateway response.
    pub fn bad_gateway(&mut self) -> &mut Self {
        self.status(HTTP_BAD_GATEWAY).text("Bad Gateway")
    }

    /// Sets a 504 Gateway Timeout response.
    pub fn gateway_timeout(&mut self) -> &mut Self {
        self.status(HTTP_GATEWAY_TIMEOUT).text("Gateway Timeout")
    }

    /// Sets a 503 Service Unavailable response.
    pub fn service_unavailable(&mut self) -> &mut Self {
        self.status(HTTP_SERVICE_UNAVAILABLE)
            .text("Service Unavailable")
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
            "Set-Cookie",
            format!("{}={}; Path=/; HttpOnly; SameSite=Lax", name, value),
        )
    }

    /// Expires a cookie in the browser.
    pub fn clear_cookie(&mut self, name: impl Into<String>) -> &mut Self {
        let name = name.into();

        if !is_valid_cookie_name(&name) {
            return self;
        }

        self.header(
            "Set-Cookie",
            format!("{}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax", name),
        )
    }

    /// Redirects the client to another location.
    pub fn redirect(&mut self, location: impl Into<String>) -> &mut Self {
        let location = location.into();

        if !is_valid_header_value(&location) {
            return self;
        }

        self.status(HTTP_FOUND)
            .set_header("Location", location)
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
            "HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n",
            $res_code,
            content.len(),
            $content_type,
        )
        .into_bytes();

        // Add every stored header value.
        for (name, values) in ($headers).iter() {
            for value in values {
                response.extend_from_slice(format!("{}: {}\r\n", name, value).as_bytes());
            }
        }

        // Finish the headers and append the response body.
        response.extend_from_slice(b"\r\n");
        response.extend_from_slice(content);

        response
    }};
}

/// The core of Blank, a web server that listens on a TCP port.
///
/// The route registry is private. Each route points to a view function
/// that receives a request and fills a response.
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

    /// Function that runs the server on the specified port.
    ///
    /// The runtime is a small cooperative executor:
    /// - The listener and client streams are non-blocking.
    /// - Every client is handled by an async future.
    /// - Pending futures yield back to the executor.
    /// - The executor polls active futures without spawning a thread
    ///   per connection.
    ///
    /// This first standard-library-only executor polls its tasks at a
    /// fixed interval. It is portable, but a future reactor can replace
    /// the polling scheduler for better scalability.
    pub fn serve(&self) -> Result<(), BlankError> {
        // Bind the server to the configured host and port.
        let server = TcpListener::bind(format!("{}:{}", self.host, self.tcp_port))
            .map_err(|_| ERROR_BIND_SERVER)?;

        // Make the listener non-blocking so accept() never stalls
        // the entire coroutine scheduler.
        server
            .set_nonblocking(true)
            .map_err(|_| ERROR_SET_NONBLOCKING)?;

        // Keep a stable snapshot of the route registry for all tasks.
        let routes = Arc::new(self.routes.clone());

        // A no-op waker is sufficient because this executor polls
        // every pending task on each scheduler pass.
        let waker = Waker::from(Arc::new(BlankWake));
        let mut context = Context::from_waker(&waker);

        // Every accepted client becomes its own Future.
        // The Future is a coroutine state machine generated by async fn.
        let mut tasks: Vec<Pin<Box<dyn Future<Output = ()> + Send>>> = Vec::new();

        // Run forever until the listener encounters a fatal error.
        loop {
            // Accept connections in bounded batches.
            // This prevents a continuous stream of new clients from
            // starving the tasks that are already running.
            let mut accepted = 0;

            loop {
                if accepted >= MAX_ACCEPTS_PER_TICK {
                    break;
                }

                match server.accept() {
                    Ok((client, _address)) => {
                        accepted += 1;

                        // Keep the number of active tasks bounded.
                        // Dropping an accepted stream closes excess connections.
                        if tasks.len() >= MAX_ACTIVE_CONNECTIONS {
                            continue;
                        }

                        // Every accepted stream must also be non-blocking.
                        if client.set_nonblocking(true).is_err() {
                            continue;
                        }

                        // Create a coroutine task for this client.
                        tasks.push(Box::pin(handle_connection(client, Arc::clone(&routes))));
                    }
                    // No more connections are ready right now.
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    // Retry an interrupted except operation.
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    // A fatal listener error should be returned to the caller.
                    Err(_) => return Err(ERROR_ACCEPT_CONNECTION),
                }
            }

            // Poll every active client coroutine once.
            let mut index = 0;

            while index < tasks.len() {
                match tasks[index].as_mut().poll(&mut context) {
                    // The connection has finished; remove its task.
                    Poll::Ready(()) => {
                        tasks.swap_remove(index);
                    }
                    // The connection needs more socket readiness.
                    // Leave it in the list for the next scheduler pass.
                    Poll::Pending => {
                        index += 1;
                    }
                }
            }

            // This is the portable polling interval.
            // It prevents a busy loop when every socket is waiting.
            thread::sleep(Duration::from_millis(RUNTIME_POLL_INTERVAL_MS));
        }
    }
}

/// Normalizes a route to begin with a slash.
fn normalize_route(route: &str) -> String {
    if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{}", route)
    }
}

/// A no-op waker used by Blank's polling executor.
struct BlankWake;

impl Wake for BlankWake {
    fn wake(self: Arc<Self>) {}
}

/// A small standard-library replacement for `std::future::poll_fn`.
///
/// It lets the socket helpers return Pending when a non-blocking
/// read or write would block, then resume on the next executor pass.
struct PollFn<F, T> {
    function: F,

    // Keep the output type in the future's type so it is constrained
    // by the Future implementation.
    _output: PhantomData<fn() -> T>,
}

impl<F, T> Future for PollFn<F, T>
where
    F: FnMut(&mut Context<'_>) -> Poll<T> + Unpin,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        (self.get_mut().function)(context)
    }
}

/// Creates a future from a polling closure.
fn poll_fn<F, T>(function: F) -> PollFn<F, T>
where
    F: FnMut(&mut Context<'_>) -> Poll<T> + Unpin,
{
    PollFn {
        function,
        _output: PhantomData,
    }
}

/// Reads one HTTP header block from a non-blocking client stream.
async fn read_http_request(client: &mut TcpStream) -> io::Result<Option<Vec<u8>>> {
    // Store all bytes read from the client.
    let mut request = Vec::new();

    // Reuse the configured HTTP buffer size.
    let mut buffer = HTTP_BUFFER;

    // Prevent a client from keeping a coroutine alive forever
    // by connecting and never finishing its request headers.
    let started = Instant::now();

    loop {
        // Try to read. If the socket blocked, yield to the executor.
        let bytes_read = poll_fn(|_context| {
            if started.elapsed() >= Duration::from_millis(HTTP_REQUEST_TIMEOUT_MS) {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "HTTP request timed out",
                )));
            }

            match client.read(&mut buffer) {
                Ok(bytes_read) => Poll::Ready(Ok(bytes_read)),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => Poll::Pending,
                Err(error) => Poll::Ready(Err(error)),
            }
        })
        .await?;

        // The client closed its write side.
        if bytes_read == 0 {
            return if request.is_empty() {
                Ok(None)
            } else {
                Ok(Some(request))
            };
        }

        // Append the newly received bytes.
        request.extend_from_slice(&buffer[..bytes_read]);

        // Check whether the complete header block has arrived.
        if let Some(header_end) = request
            .windows(HTTP_HEADER_END.len())
            .position(|window| window == HTTP_HEADER_END)
        {
            // Only the header block is currently parsed.
            if header_end > MAX_HTTP_HEADER_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "HTTP request headers are too large",
                ));
            }

            return Ok(Some(request));
        }

        // Stop clients from sending unlimited incomplete headers.
        if request.len() > MAX_HTTP_HEADER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HTTP request headers are too large",
            ));
        }
    }
}

/// Writes all response bytes to a non-blocking client stream.
async fn write_all_async(client: &mut TcpStream, content: &[u8]) -> io::Result<()> {
    // Keep track of how much data has already been written.
    let mut written = 0;

    // Do not let a client that never reads hold a task forever.
    let started = Instant::now();

    while written < content.len() {
        // Try to write the remaining bytes.
        let bytes_written = poll_fn(|_context| {
            if started.elapsed() >= Duration::from_millis(HTTP_WRITE_TIMEOUT_MS) {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "HTTP response write timed out",
                )));
            }

            match client.write(&content[written..]) {
                Ok(0) => Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "failed to write HTTP response",
                ))),
                Ok(bytes_written) => Poll::Ready(Ok(bytes_written)),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => Poll::Pending,
                Err(error) => Poll::Ready(Err(error)),
            }
        })
        .await?;

        written += bytes_written;
    }

    Ok(())
}

/// A parsed HTTP request containing its method, route, version,
/// and headers.
struct ParsedRequest {
    method: String,
    route: String,
    version: String,
    headers: BlankReq,
}

/// Parses an HTTP request header block.
fn parse_http_request(request: &[u8]) -> Result<ParsedRequest, &'static str> {
    // Find the end of the HTTP headers.
    let Some(header_end) = request
        .windows(HTTP_HEADER_END.len())
        .position(|window| window == HTTP_HEADER_END)
    else {
        return Err(HTTP_BAD_REQUEST);
    };

    // Convert the header bytes into text.
    let header_text = String::from_utf8_lossy(&request[..header_end]);

    // Parse the request line and all following header lines.
    let mut lines = header_text.lines();

    // The first line contains method, route, and version.
    let Some(line) = lines.next() else {
        return Err(HTTP_BAD_REQUEST);
    };

    let mut parts = line.split_whitespace();

    let (Some(method), Some(route), Some(version)) = (parts.next(), parts.next(), parts.next())
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
    if !route.starts_with('/') {
        return Err(HTTP_BAD_REQUEST);
    }

    // Store every request header, including repeated header names.
    let mut headers: BlankReq = HashMap::new();

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

        // Reject malformed header names.
        if !is_valid_header_name(name) {
            return Err(HTTP_BAD_REQUEST);
        }

        headers
            .entry(name.to_ascii_lowercase())
            .or_default()
            .push(value.to_string());
    }

    // The query string is not part of the route registry key.
    let route = route.split('?').next().unwrap_or(route).to_string();

    Ok(ParsedRequest {
        method: method.to_string(),
        route,
        version: version.to_string(),
        headers,
    })
}

/// Runs one accepted client connection as a coroutine.
async fn handle_connection(mut client: TcpStream, routes: Arc<HashMap<String, BlankView>>) {
    // Read the HTTP request without blocking other tasks.
    let request = match read_http_request(&mut client).await {
        Ok(Some(request)) => request,
        Ok(None) => return,
        Err(error) => {
            // A request that exceeds the header limit receives 431.
            let (status, body) = match error.kind() {
                io::ErrorKind::InvalidData => {
                    (HTTP_HEADER_FIELDS_TOO_LARGE, "Request headers too large")
                }
                io::ErrorKind::TimedOut => (HTTP_REQUEST_TIMEOUT, "Request Timeout"),
                _ => (HTTP_BAD_REQUEST, "Bad Request"),
            };

            let response = http_return!(
                status,
                body.as_bytes(),
                CONTENT_TYPE_TEXT,
                &HashMap::<String, Vec<String>>::new()
            );

            let _ = write_all_async(&mut client, &response).await;
            return;
        }
    };

    // Parse the request into Blank's request representation.
    let response = match parse_http_request(&request) {
        Ok(request) => {
            // Keep method and version available for future BlankReq fields.
            let _method = request.method;
            let _version = request.version;

            // Create an output object for the selected view.
            let mut out = BlankRes::new();

            // Find the view registered for this route.
            if let Some(view) = routes.get(&request.route) {
                // Run the view with the request headers and mutable response.
                view(&request.headers, &mut out);
            } else {
                // The requested route does not exist.
                out.status(HTTP_NOT_FOUND).text("Not Found");
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
                HTTP_BAD_REQUEST => "Bad Request",
                HTTP_VERSION_NOT_SUPPORTED => "HTTP Version Not Supported",
                HTTP_NOT_IMPLEMENTED => "Not Implemented",
                HTTP_METHOD_NOT_ALLOWED => "Method Not Allowed",
                HTTP_REQUEST_TIMEOUT => "Request Timeout",
                _ => "Request Error",
            };

            http_return!(
                status,
                body.as_bytes(),
                CONTENT_TYPE_TEXT,
                &HashMap::<String, Vec<String>>::new()
            )
        }
    };

    // Send the response. A failed client write simply ends this task.
    let _ = write_all_async(&mut client, &response).await;
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

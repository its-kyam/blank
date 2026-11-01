// server.rs
//! This file manages Blank objects
//!
//! Blank objects are basically servers.
//!
//! Implements all related traits and methods as well.
//! Blank can be used to create a server, bind
//! it to a TCP port, and listen to it for requests
//!
//! Also uses a macro from tests.rs to run tests that need
//! access to Blank private fields

// Constants
// Constants
use crate::constants::{
    BlankError, DEFAULT_PORT, ERROR_BIND_SERVER, HTTP_BAD_REQUEST, HTTP_BUFFER, HTTP_HEADER_END,
    HTTP_METHOD_NOT_ALLOWED, HTTP_NOT_IMPLEMENTED, HTTP_OK, HTTP_VERSION_NOT_SUPPORTED, LOCALHOST,
    REAL_HTTP_METHODS, SUPPORTED_HTTP_METHODS, SUPPORTED_HTTP_VERSIONS,
};

// Imports
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;

/// An Alias of an u16 that is to make Ports
/// Port is basically a TCP port
pub type Port = u16;

/// An alias of String
pub type Host = String;

/// The core of blank, a web server
/// that listens on a TCP port
// use a cfg_attr so that our unit tests
// can use tcp_port
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Blank {
    /// TCP Port on which the server listens on
    tcp_port: Port,

    /// Host on which the server listens...
    ///
    /// localhost is the only available host right now
    host: Host,
}

// A simple macro for a finished parse for the client
macro_rules! http_return {
    (
        // response code
        $res_code:expr,
        // response content
        $content:expr,
        // content type
        $content_type:expr,
        // close connection
        $close:expr,
        // additional headers
        $headers:expr
    ) => {{
        let content = $content;

        let mut response = format!(
            "HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: {}\r\n",
            $res_code,
            content.as_bytes().len(),
            $content_type,
            if $close { "close" } else { "keep-alive" },
        );

        for (name, values) in $headers {
            for value in values {
                response.push_str(&format!("{}: {}\r\n", name, value));
            }
        }

        response.push_str(&format!("\r\n{}", content));

        response
    }};

    (
        // response code
        $res_code:expr,
        // response content
        $content:expr,
        // content type
        $content_type:expr,
        // close connection
        $close:expr
    ) => {
        http_return!(
            $res_code,
            $content,
            $content_type,
            $close,
            HashMap::<&str, Vec<&str>>::new()
        )
    };

    (
        // response code
        $res_code:expr,
        // response content
        $content:expr,
        // content type
        $content_type:expr
    ) => {
        http_return!($res_code, $content, $content_type, true)
    };

    (
        // response code
        $res_code:expr,
        // response content
        $content:expr
    ) => {
        http_return!($res_code, $content, "text/plain")
    };

    (
        // response code
        $res_code:expr
    ) => {
        http_return!($res_code, "", "text/plain")
    };

    () => {
        http_return!("200 OK")
    };
}

impl Blank {
    /// Returns a new Blank with a default port
    pub fn new() -> Self {
        Self {
            tcp_port: DEFAULT_PORT,
            host: LOCALHOST.to_string(),
        }
    }

    /// Function that runs the server on the specified port
    /// `tcp_port`, returns Result<(), BlankError>
    ///
    /// takes return_stuff (a static &str) that is
    pub fn serve(&self, return_stuff: &'static str) -> Result<(), BlankError> {
        // Use an Ok(var) approach to checking whether bind
        // is successful
        if let Ok(server) = TcpListener::bind(
            // Use a format expression like this:
            format!("{}:{}", self.host, self.tcp_port),
        ) {
            // listen for each request
            for request in server.incoming() {
                // Check if the client can connect
                let Ok(mut client) = request else {
                    continue;
                };

                // Make an empty Vec so that read_to_end can read it
                let mut req = Vec::new();

                // Make a buffer to read it
                let mut buffer = HTTP_BUFFER;

                // whether it is parsed
                let mut parsed: bool = false;

                // Read it
                loop {
                    // make bytes_read
                    let bytes_read = match client.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => break,
                    };

                    // extend
                    req.extend_from_slice(&buffer[..bytes_read]);

                    // check if its ending
                    if req
                        .windows(HTTP_HEADER_END.len())
                        .any(|window| window == HTTP_HEADER_END)
                    {
                        break;
                    }
                }

                // Use Vec<&str> instead of String as some
                // headers may have multiple occurrences as
                // well as be of List type
                let mut headers: HashMap<&str, Vec<&str>> = HashMap::new();

                // return headers
                let mut return_headers: HashMap<&str, Vec<&str>> = HashMap::new();

                // Find the end of the HTTP headers
                let Some(header_end) = req
                    .windows(HTTP_HEADER_END.len())
                    .position(|window| window == HTTP_HEADER_END)
                else {
                    continue;
                };

                // Convert header bytes into text
                let header_text = String::from_utf8_lossy(&req[..header_end]);

                // Parse the bytes
                let mut lines = header_text.lines();

                // Make a default response
                let mut response = http_return!(HTTP_BAD_REQUEST);

                // Get the first line so that we can extract stuff
                // from it
                if let Some(line) = lines.next() {
                    let (http_method, http_route, http_version, extra) = {
                        let mut lt = line.trim().split_whitespace();

                        // return it else change the response
                        (lt.next(), lt.next(), lt.next(), lt.next())
                    };

                    // Check whether all three parts exist
                    if let (Some(http_method), Some(http_route), Some(http_version)) =
                        (http_method, http_route, http_version)
                    {
                        // route will be used when route handling is implemented
                        let _ = http_route;

                        // Check if the request line has extra arguments
                        if extra.is_some() {
                            response = http_return!(HTTP_BAD_REQUEST);
                        }
                        // Check whether the HTTP version is supported
                        else if !SUPPORTED_HTTP_VERSIONS.contains(&http_version) {
                            response = http_return!(HTTP_VERSION_NOT_SUPPORTED);
                        }
                        // Check whether the HTTP method is supported
                        else if SUPPORTED_HTTP_METHODS.contains(&http_method) {
                            parsed = true;
                        }
                        // Check whether it is a real HTTP method
                        else if REAL_HTTP_METHODS.contains(&http_method) {
                            response = http_return!(HTTP_NOT_IMPLEMENTED);
                        }
                        // The HTTP method is invalid
                        else {
                            response = http_return!(HTTP_METHOD_NOT_ALLOWED);
                        }
                    }
                }

                // Check whether the request was successfully parsed
                if parsed {
                    response = http_return!(HTTP_OK, return_stuff);
                }

                // Write data to the client
                if client.write_all(response.as_bytes()).is_err() {
                    continue;
                };
            }
        } else {
            return Err(ERROR_BIND_SERVER);
        }

        Ok(())
    }
}

/// from Port or u16
impl From<Port> for Blank {
    /// returns a new Blank with specified TCP port
    ///
    /// port is an u16
    // already public
    fn from(port: Port) -> Self {
        Self {
            tcp_port: port,
            host: LOCALHOST.to_string(),
        }
    }
}

/// Implement Default for fun lol
impl Default for Blank {
    fn default() -> Self {
        // return what new() returns
        Self::new()
    }
}

// the tests
#[cfg(test)]
mod tests {
    // import the macro test
    use crate::server_tests;

    // Use
    server_tests!(9022, "COOL STUFF!", 300,);
}

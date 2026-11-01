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
use crate::constants::{BlankError, DEFAULT_PORT, ERROR_BIND_SERVER, HTTP_BUFFER, LOCALHOST};

// Imports
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

impl Blank {
    /// Returns a new Blank with a default port
    pub fn new() -> Self {
        Self {
            tcp_port: DEFAULT_PORT,
            host: LOCALHOST.to_string(),
        }
    }

    /// Function that runs the server on the specified port
    /// `tcp_port`, returns Result<(), &str>
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
                    if req.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }

                // make a hardcoded response
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{}",
                    return_stuff.len(),
                    return_stuff
                );

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
    server_tests!(
        // use a random port number
        9022,
        // use a random return string
        "Why bro....",
    );
}

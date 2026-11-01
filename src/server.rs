// server.rs
//! Manages Blank objects
//! Blank objects are basically servers.
//!
//! Implements all related traits and methods as well.
//! Blank can be used to create a server, bind
//! it to a TCP port, and listen to it for requests

// Constants
use crate::constants::DEFAULT_PORT;

/// A Alias of a u16 that is to make Ports
/// Port is basically a TCP port
pub type Port = u16;

/// The core of blank, a web server
/// that listens on a TCP port
// use a cfg_attr so that our unit tests
// can use tcp_port
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Blank {
    /// TCP Port on which the server listens on
    tcp_port: Port,
}

impl Blank {
    /// Returns a new Blank with a default port
    pub fn new() -> Self {
        Self {
            tcp_port: DEFAULT_PORT,
        }
    }
}

/// from Port or u16
impl From<Port> for Blank {
    /// returns a new Blank with specified TCP port
    ///
    /// port is a u16
    // alredy public
    fn from(port: Port) -> Self {
        Self { tcp_port: port }
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

    // test stuff
    use super::Blank;

    // constants used
    use crate::constants::DEFAULT_PORT;

    server_tests!(
        // use a random port number
        9022
    );
}

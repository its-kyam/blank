// constants.rs
//! Stores all constants used by Blank
//! Also stores user-facing strings
//! Contains errors (Just &str Lol)  as well
//!
//! BlankError type is implemented.
//!
//! Current constants are:
//!     - LOCALHOST
//!     - DEFAULT_PORT
//!
//! Current strings:
//!     - HELLO_MESSAGE
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

/// Error type for this crate
pub type BlankError = u8;

// Errors
/// Error on fail to bind to a tcp port
pub const ERROR_BIND_SERVER: BlankError = 1;

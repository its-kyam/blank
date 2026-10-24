// constants.rs
//! Stores all constants used by Blank
//! Also stores user-facing strings
//!
//! There are no current constants
//! Current strings:
//!     - hello_message

use crate::server::Port;

/// Message returned by function 'hello_blank'
pub(crate) const HELLO_MESSAGE: &str = "Hello Bro";

/// Default tcp_port of Blank object returned by
/// Blank::new
pub(crate) const DEFAULT_PORT: Port = 7600;

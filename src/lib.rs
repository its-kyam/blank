// lib.rs
//! This is blank,
//! The ultimate web framework
//!
//! Code example:
//! ```rust
//! use blank::{hello_blank, Blank};
//!
//! fn main() {
//!     println!("{}", hello_blank());
//!
//!     // make two servers
//!     let my_server = Blank::from(1300);
//!     let my_server2 = Blank::new();
//! }
//! ```

// mod all files
pub mod constants;
mod server;

// use compile-only on tests.rs
// use macro_use so that the tests macro can be there
#[cfg(test)]
#[macro_use]
mod tests;

// make module contents publicly available
pub use server::{Blank, BlankHeaders, BlankReq, BlankView};

use constants::HELLO_MESSAGE;

pub fn hello_blank() -> &'static str {
    HELLO_MESSAGE
}

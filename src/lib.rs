// lib.rs
//! This is blank,
//! The ultimate web framework
//!
//! ### A simple example
//!
//! ```rust
//! use blank_web::{Blank, BlankRes};
//!
//! fn main() -> () {
//!     let mut server = Blank::new();
//!
//!    server.route_func("/home", |_rq, out| {
//!        out.html("<h1>Hello Bro!</h1>");
//!     });
//!
//!     server.route_func("/", |_rq, out| {
//!         out.redirect("/home");
//!    });
//!
//!     server.serve().unwrap();
//!  }
//! ```

// mod all files
pub mod constants;
pub mod server;

// use compile-only on tests.rs
// use macro_use so that the tests macro can be there
#[cfg(test)]
#[macro_use]
mod tests;

// make module contents publicly available
pub use server::{Blank, BlankHeaders, BlankReq, BlankRes, BlankView};

use constants::HELLO_MESSAGE;

pub fn hello_blank() -> &'static str {
    HELLO_MESSAGE
}

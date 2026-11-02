// lib.rs
//! # This is Blank, a blazing-fast, developer-friendly, coroutine-based rust web framework
//! Simply put, the ultimate web framework
//!
//! When you use Blank (yes that's its name!),
//! you will find that its very developer-friendly while
//! serving users extremely fast, just look at the
//! example below to understand:
//!
//!  ### Example
//!
//! ```rust
//! use blank_web::{Blank, BlankRes};
//! use std::time::Duration;
//! use std::thread::sleep;
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
//!     let working = server.serve_async();
//!
//!     sleep(Duration::from_millis(5000));
//!     working.unwrap().async_stop();
//!  }
//! ```
//!
//! ### The basics of blank servers
//!
//! To start off, when you create a server using Blank,
//! you will use the Blank struct from the server module
//! (make it mutable so that you can register views).
//!
//! The default port when you use `Blank::new()` is 7600,
//! use the method `server.port(2020)` to set the
//! tcp port on which the server listens on. Use `Blank::from(2020)`
//! if you don't want to use an extra method
//!
//! Also, there is a type alias Port, which is u16, and can be
//! imported from server::Port
//!
//!  When you're in production, use`server.host("0.0.0.0")`
//! to let external traffic access the server, you can also
//! make the server listen on other hosts by using the same
//! method. The default port is `localhost`, there is also
//! an extra step when binding that makes sure that
//! it refers to both ipV6 or ipV4 or both.
//!
//! ```rust
//! use blank_web::{Blank};
//!
//! fn main() {
//!     // The TCP port on which this server runs on is
//!     // 7600
//!     let mut server = Blank::new();
//!     
//!     // Use get_port() method to get the tcp port on
//!     // which a constructed Blank will listen on
//!     assert_eq!(server.get_port(), 7600);
//!
//!     server.port(2020);
//!     assert_eq!(server.get_port(), 2020);
//!
//!     // Use constructor from() to set port
//!     // during construction
//!     let new_server = Blank::from(2050);
//!     assert_eq!(new_server.get_port(), 2050);
//!
//!     // Host is... localhost
//!     // like get_port, get_host() will return the host
//!     // of the server
//!     assert_eq!(server.get_host(), "localhost");
//!
//!     // set the host with host()
//!     server.host("0.0.0.0");
//!     assert_eq!(server.get_host(), "0.0.0.0");
//! }
//! ```
//!

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

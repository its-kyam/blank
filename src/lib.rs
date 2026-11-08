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
//! The default port when you use
//! <a href="server/struct.Blank.html#method.new">`Blank::new()` </a>
//! is `7600`,
//! use the method
//! <a href="server/struct.Blank.html#method.port">`server.port(2020)`</a>
//! to set the tcp port on which the server listens on. Use
//! <a href="server/struct.Blank.html#method.from">`Blank::from(2020)`</a>
//! if you don't want to use an extra method
//!
//! Also, there is a type alias Port, which is u16, and can be
//! imported from
//! <a href="server/type.Port.html">`server::Port`</a>
//!
//!  When you're in production, use
//! <a href="server/struct.Blank.html#method.host">`server.host("0.0.0.0")`</a>
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
//! ### How to write a view
//! Its actually really simple! You can use it like express
//! by calling the
//! <a href="server/struct.Blank.html#method.route_func">`server.route_func("/", |rq, out| {})`</a>
//! supplying a route and a closure; the closure taking
//! two arguments whose types are `server::BlankReq`,
//! and `server::BlankRes` respectively.
//!
//! Also, the BlankRes and BlankReq objects can be
//! used to access a ton of resources, you can read their
//! whole documentation at
//! <a href="server/struct.BlankRes.html">`server::BlankRes`</a>
//! and
//! <a href="server/struct.BlankReq.html">`server::BlankReq`</a>
//!
//! An example view is given below:
//!
//! ```rust
//! use blank_web::Blank;
//!
//! fn main() {
//!     let mut server = Blank::new();
//!
//!     server.route_func("/", |rq, out| {
//!         // We'll check the request method, and then we'll do a basic JSON
//!         if rq.is_get() {
//!             out.json(r#"{ "message": "Hello from the Blank Server!" }"#);
//!         } else {
//!             out.method_not_allowed();
//!         }
//!     });
//! }
//! ```
//!
//! ### How to run a server
//! Now we come to the most important part of Blank,
//! running the server!
//!
//! You can do this in two ways, asynchronously, or
//! synchronously, the latter of which is better if your only
//! job is to run the server. You can still use asynchronous
//! if you want (which is what you'll see here as we can't afford
//! our doctests to run forever!).
//!
//! The
//! <a href="server/struct.Blank.html#method.serve_async">`server.serve_async()`</a>
//! method starts a new thread and runs the main server
//! thread in that thread. You can use
//! <a href="server/struct.Blank.html#method.serve_await">`server.serve_await()`</a>
//! to start the main server thread in the same main thread
//!
//! The function
//! <a href="server/struct.Blank.html#method.serve_async">`server.serve_async()`</a>
//! returns a
//! <a href="server/struct.BlankServer.html">`server::BlankServer`</a>
//! which can be used to stop the server (
//! <a href="server/struct.BlankServer.html#method.async_stop">`server.async_stop()`</a>
//! ) or more advanced stuff.
//!
//! There's also a function
//! <a href="server/struct.Blank.html#method.serve">`server.serve()`</a>
//! which is an alias to
//! <a href="server/struct.Blank.html#method.serve_await">`server.serve_await()`</a>.
//!
//! How to run your server:
//! ```rust
//! use blank_web::Blank;
//!
//! fn main() {
//!     let mut server = Blank::from(13536);
//!
//!     server.route_func("/", |_rq, out| {
//!         out.text("Running our server!");
//!     });
//!
//!     let handle = server.serve_async();
//!
//!     handle.unwrap().async_stop();
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

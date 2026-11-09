//! # This is Blank, a blazing-fast, developer-friendly, coroutine-based Rust web framework
//!
//! Simply put, the ultimate web framework.
//!
//! When you use Blank (yes, that's its name!), you will find
//! that it is developer-friendly while serving users extremely
//! fast. Just look at the example below to understand:
//!
//! ### Example
//!
//! ```rust
//! use blank_web::{Blank, BlankRes};
//! use std::time::Duration;
//! use std::thread::sleep;
//!
//! fn main() -> () {
//!     let mut server = Blank::new();
//!
//!     server.route_func("/home", |_rq, out| {
//!         out.html("<h1>Hello Bro!</h1>");
//!     });
//!
//!     server.route_func("/", |_rq, out| {
//!         out.redirect("/home");
//!     });
//!
//!     let working = server.serve_async();
//!
//!     sleep(Duration::from_millis(5000));
//!     working.unwrap().async_stop();
//! }
//! ```
//!
//! ### The basics of Blank servers
//!
//! To start off, when you create a server using Blank,
//! you will use the [`Blank`] struct (make it mutable so
//! that you can register views).
//!
//! The default port when you use [`Blank::new()`] is `7600`.
//! Use the method [`Blank::port()`] to set the TCP port
//! on which the server listens.
//!
//! Alternatively, use [`Blank::from()`] if you don't want
//! to use an extra method to configure the port during
//! construction.
//!
//! There is also a type alias [`Port`], which is `u16`.
//!
//! When you're in production, use [`Blank::host()`] with
//! `"0.0.0.0"` to let external traffic access the server.
//! You can also make the server listen on other hosts by
//! using the same method.
//!
//! The default host is `localhost`. Blank also performs
//! address resolution when binding, allowing it to handle
//! IPv4, IPv6, or both, depending on the resolved addresses.
//!
//! ```rust
//! use blank_web::Blank;
//!
//! fn main() {
//!     // The TCP port on which this server runs is 7600.
//!     let mut server = Blank::new();
//!
//!     // Use get_port() to get the TCP port on which
//!     // a constructed Blank instance will listen.
//!     assert_eq!(server.get_port(), 7600);
//!
//!     server.port(2020);
//!     assert_eq!(server.get_port(), 2020);
//!
//!     // Use from() to set the port during construction.
//!     let new_server = Blank::from(2050);
//!     assert_eq!(new_server.get_port(), 2050);
//!
//!     // The default host is localhost.
//!     // Like get_port(), get_host() returns the host
//!     // configured for the server.
//!     assert_eq!(server.get_host(), "localhost");
//!
//!     // Set the host using host().
//!     server.host("0.0.0.0");
//!     assert_eq!(server.get_host(), "0.0.0.0");
//! }
//! ```
//!
//! ### How to write a view
//!
//! It's actually really simple! You can use Blank like
//! Express by calling [`Blank::route_func()`], supplying
//! a route and a closure.
//!
//! The closure takes two arguments:
//!
//! - [`BlankReq`], which provides access to the incoming request.
//! - [`BlankRes`], which is used to construct the outgoing response.
//!
//! Both objects provide methods for accessing and working
//! with HTTP request and response data.
//!
//! You can read their complete documentation at
//! [`BlankReq`] and [`BlankRes`].
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
//!         // Check the request method, then return JSON.
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
//!
//! Now we come to the most important part of Blank:
//! running the server!
//!
//! You can do this in two ways: asynchronously or
//! synchronously. The synchronous approach is generally
//! suitable when your only job is to run the server.
//!
//! You can also use the asynchronous approach when you
//! need to control the server from another part of your
//! application.
//!
//! The [`Blank::serve_async()`] method starts the server
//! in a background thread and returns a [`BlankServer`]
//! handle.
//!
//! You can use [`Blank::serve_await()`] to run the server
//! in the calling thread instead.
//!
//! The [`BlankServer`] handle can be used to stop a
//! background server through [`BlankServer::async_stop()`].
//!
//! There's also [`Blank::serve()`], which is an alias
//! for [`Blank::serve_await()`].
//!
//! Here's how to run your server:
//!
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
pub use server::{Blank, BlankHeaders, BlankReq, BlankRes, BlankServer, BlankView, Host, Port};

use constants::HELLO_MESSAGE;

pub fn hello_blank() -> &'static str {
    HELLO_MESSAGE
}

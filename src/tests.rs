// tests.rs
//! This module helps blank to run tests
//! when writing the framework
//!
//! Only rust tests
//!
//! Current tests
//!     - hello_test
//!     - blank_constructor_test (as a macro)

// crate imports
use crate::constants::HELLO_MESSAGE;
use crate::hello_blank;

#[test]
/// Tests whether the function hello_blank() is working
fn hello_test() {
    assert_eq!(hello_blank(), HELLO_MESSAGE);
}

/// A macro used to access private fields in server.rs
#[macro_export]
// use macros to access private fields
macro_rules! server_tests {
    (
        // for Blank::from test
        $some_random_port:expr,

        // for Blank::serve
        $some_random_return_string:expr,

        // for Blank::serve
        $some_random_deadline:expr,
    ) => {
        // imports
        use std::io::{Read, Write};
        use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
        use std::thread;
        use std::time::Duration;

        // use super for crate imports
        use super::Blank;
        use super::{DEFAULT_PORT, LOCALHOST};

        /// Tests whether all Constructors of Blank are working
        #[test]
        fn blank_constructor_test() {
            // test constructor new
            assert_eq!(
                Blank::new(),
                Blank {
                    tcp_port: DEFAULT_PORT,
                    host: LOCALHOST.to_string(),
                }
            );

            // test constructor from
            assert_eq!(
                Blank::from($some_random_port),
                Blank {
                    tcp_port: $some_random_port,
                    host: LOCALHOST.to_string(),
                }
            );

            // test constructor default
            // should be same as new()
            // new() is already tested
            assert_eq!(Blank::default(), Blank::new());
        }

        #[test]
        /// Tests whether serve() function is working
        fn server_test() {
            let server = Blank::from($some_random_port);

            // save the host
            let host = server.host.clone();

            let return_stuff = $some_random_return_string;

            // Start server in a separate thread
            // We can use unwrap here as panics are what
            // are supposed to happen
            let _server_func = thread::spawn(move || {
                server.serve(return_stuff).unwrap();
            });

            // Wait for server to become available

            // address
            // use format for this
            let addr: SocketAddr = format!("{}:{}", host, $some_random_port)
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap();

            // make a client object that can be used to request the server
            let mut client = TcpStream::connect_timeout(
                &addr,
                // make a deadline of milliseconds
                Duration::from_millis($some_random_deadline),
            )
            .unwrap();

            // Request an endpoint and read the response
            client
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .unwrap();

            // Read the response.
            let mut res = String::new();
            client.read_to_string(&mut res).unwrap();

            // stop the server
            client.shutdown(std::net::Shutdown::Both).unwrap();

            // Evaluate the response
            assert!(res.contains("HTTP/1.1 200 OK"));
            assert!(res.contains(return_stuff));
        }
    };
}

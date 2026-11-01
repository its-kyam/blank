// tests.rs
//! This module helps blank to run tests
//! when writing the framework
//!
//! Only rust tests
//!
//! Current tests
//!     - hello_test
//!     - blank_constructor_test (as a macro)
//!     - server_get_test
//!     - server_bad_request_missing_parts_test
//!     - server_bad_request_extra_parts_test
//!     - server_http_version_not_supported_test
//!     - server_method_not_implemented_test
//!     - server_invalid_method_test

// crate imports
use crate::constants::HELLO_MESSAGE;
use crate::hello_blank;

#[test]
/// Tests whether the function hello_blank() is working
fn hello_test() {
    assert_eq!(hello_blank(), HELLO_MESSAGE);
}

/// A macro used to create individual HTTP tests
#[doc(hidden)]
#[macro_export]
macro_rules! http_test_case {
    (
        // test name
        $test_name:ident,

        // server port
        $port:expr,

        // return string
        $return_stuff:expr,

        // deadline
        $deadline:expr,

        // request
        $request:expr,

        // expected response code
        $expected_status:expr,

        // expected response content
        $expected_content:expr
        $(,)?
    ) => {
        #[test]
        fn $test_name() {
            run_http_test(
                $port,
                $return_stuff,
                $deadline,
                $request,
                $expected_status,
                $expected_content,
            );
        }
    };
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
        use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
        use std::thread;
        use std::time::{Duration, Instant};

        // use super for crate imports
        use super::Blank;
        use super::{
            DEFAULT_PORT, HTTP_BAD_REQUEST, HTTP_METHOD_NOT_ALLOWED, HTTP_NOT_IMPLEMENTED, HTTP_OK,
            HTTP_VERSION_NOT_SUPPORTED, LOCALHOST,
        };

        /// Shared function for running HTTP tests
        fn run_http_test(
            // TCP port
            port: u16,

            // return string
            return_stuff: &'static str,

            // deadline in milliseconds
            deadline: u64,

            // HTTP request
            request: &[u8],

            // expected response code
            expected_status: &str,

            // expected response content
            expected_content: &str,
        ) {
            let server = Blank::from(port);

            // save the host
            let host = server.host.clone();

            // Start server in a separate thread
            // We can use unwrap here as panics are what
            // are supposed to happen
            let _server_func = thread::spawn(move || {
                server.serve(return_stuff).unwrap();
            });

            // address
            // use format for this
            let addr: SocketAddr = format!("{}:{}", host, port)
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap();

            // Wait for server to become available
            let deadline_at = Instant::now() + Duration::from_millis(deadline);

            let mut client = loop {
                let now = Instant::now();

                assert!(now < deadline_at, "Server did not become available");

                let timeout = (deadline_at - now).min(Duration::from_millis(50));

                match TcpStream::connect_timeout(&addr, timeout) {
                    Ok(client) => break client,

                    Err(_) => {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            };

            // Request an endpoint and read the response
            client.write_all(request).unwrap();

            // Read the response.
            let mut res = String::new();
            client.read_to_string(&mut res).unwrap();

            // stop the server
            client.shutdown(Shutdown::Both).unwrap();

            // Evaluate the response
            let expected_status = format!("HTTP/1.1 {}", expected_status);

            assert!(
                res.starts_with(&expected_status),
                "Expected {}, got:\n{}",
                expected_status,
                res
            );

            // Check the response content if it exists
            if !expected_content.is_empty() {
                assert!(
                    res.contains(expected_content),
                    "Expected response content {}, got:\n{}",
                    expected_content,
                    res
                );
            }
        }

        // Tests whether all Constructors of Blank are working
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

        // HTTP TESTS

        $crate::http_test_case!(
            server_get_test,
            $some_random_port,
            $some_random_return_string,
            $some_random_deadline,
            b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_OK,
            $some_random_return_string,
        );

        $crate::http_test_case!(
            server_bad_request_missing_parts_test,
            $some_random_port + 1,
            $some_random_return_string,
            $some_random_deadline,
            b"GET\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_BAD_REQUEST,
            "",
        );

        $crate::http_test_case!(
            server_bad_request_extra_parts_test,
            $some_random_port + 2,
            $some_random_return_string,
            $some_random_deadline,
            b"GET / HTTP/1.1 EXTRA\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_BAD_REQUEST,
            "",
        );

        $crate::http_test_case!(
            server_http_version_not_supported_test,
            $some_random_port + 3,
            $some_random_return_string,
            $some_random_deadline,
            b"GET / HTTP/2.0\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_VERSION_NOT_SUPPORTED,
            "",
        );

        $crate::http_test_case!(
            server_method_not_implemented_test,
            $some_random_port + 4,
            $some_random_return_string,
            $some_random_deadline,
            b"POST / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_NOT_IMPLEMENTED,
            "",
        );

        $crate::http_test_case!(
            server_invalid_method_test,
            $some_random_port + 5,
            $some_random_return_string,
            $some_random_deadline,
            b"G@T / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_METHOD_NOT_ALLOWED,
            "",
        );
    };
}

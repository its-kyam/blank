//! This module helps Blank to run tests
//! when writing the framework.
//!
//! Only Rust tests.
//!
//! Current tests:
//!     - hello_test
//!     - blank_constructor_test (as a macro)
//!     - server_bind_failure_test
//!     - server_get_test
//!     - server_http_10_test
//!     - server_bad_request_missing_parts_test
//!     - server_bad_request_extra_parts_test
//!     - server_bad_request_empty_request_line_test
//!     - server_http_version_not_supported_test
//!     - server_method_not_implemented_test
//!     - server_invalid_method_test
//!     - server_invalid_method_lowercase_test

// crate imports
use crate::constants::HELLO_MESSAGE;
use crate::hello_blank;

#[test]
/// Tests whether the function hello_blank() is working.
fn hello_test() {
    assert_eq!(hello_blank(), HELLO_MESSAGE);
}

/// A macro used to create individual HTTP tests.
#[doc(hidden)]
#[macro_export]
macro_rules! http_test_case {
    (
        // test name
        $test_name:ident,

        // server port
        $port:expr,

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
                $deadline,
                $request,
                $expected_status,
                $expected_content,
            );
        }
    };
}

/// A macro used to access private fields in server.rs.
#[macro_export]
macro_rules! server_tests {
    // Use default arguments when no arguments are supplied.
    () => {
        $crate::server_tests!(9022, 300);
    };

    // Use the default deadline when only a port is supplied.
    ($port:expr $(,)?) => {
        $crate::server_tests!($port, 300);
    };

    (
        // server port
        $some_random_port:expr,

        // deadline
        $some_random_deadline:expr
        $(,)?
    ) => {
        // imports
        use std::collections::HashMap;
        use std::io::{Read, Write};
        use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
        use std::thread;
        use std::time::{Duration, Instant};

        // use super for crate imports
        use super::{Blank, BlankReq, BlankRes};
        use super::{
            DEFAULT_PORT, ERROR_BIND_SERVER, HTTP_BAD_REQUEST, HTTP_METHOD_NOT_ALLOWED,
            HTTP_NOT_IMPLEMENTED, HTTP_OK, HTTP_VERSION_NOT_SUPPORTED, LOCALHOST,
        };

        // A simple route used by HTTP tests.
        fn test_view(_rq: &BlankReq, out: &mut BlankRes) {
            out.text("Blank test response");
        }

        /// Shared function for running HTTP tests.
        fn run_http_test(
            // TCP port
            port: u16,

            // deadline in milliseconds
            deadline: u64,

            // HTTP request
            request: &[u8],

            // expected response code
            expected_status: &str,

            // expected response content
            expected_content: &str,
        ) {
            let mut server = Blank::from(port);

            // Register the root route.
            server.route_func("/", test_view);

            // save the host
            let host = server.host.clone();

            // Start server in a separate thread.
            // We can use unwrap here as panics are what
            // are supposed to happen.
            let _server_func = thread::spawn(move || {
                server.serve().unwrap();
            });

            // address
            // use format for this
            let addr: SocketAddr = format!("{}:{}", host, port)
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap();

            // Wait for server to become available.
            let deadline_at = Instant::now() + Duration::from_millis(deadline);

            let mut client = loop {
                let now = Instant::now();

                assert!(
                    now < deadline_at,
                    "Server did not become available on port {}",
                    port
                );

                let timeout = (deadline_at - now).min(Duration::from_millis(50));

                match TcpStream::connect_timeout(&addr, timeout) {
                    Ok(client) => break client,

                    Err(_) => {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            };

            // Prevent the test from waiting forever if the server
            // does not return a response.
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            client
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            // Request an endpoint and read the response.
            client.write_all(request).unwrap();

            // Read the response.
            let mut res = String::new();
            client.read_to_string(&mut res).unwrap();

            // Stop the client connection.
            let _ = client.shutdown(Shutdown::Both);

            // Evaluate the response.
            let expected_status = format!("HTTP/1.1 {}", expected_status);

            assert!(
                res.starts_with(&expected_status),
                "Expected {}, got:\n{}",
                expected_status,
                res
            );

            // Check the response content if it exists.
            if !expected_content.is_empty() {
                assert!(
                    res.contains(expected_content),
                    "Expected response content {}, got:\n{}",
                    expected_content,
                    res
                );
            }
        }

        // Tests whether all constructors of Blank are working.
        #[test]
        fn blank_constructor_test() {
            // test constructor new
            assert_eq!(
                Blank::new(),
                Blank {
                    tcp_port: DEFAULT_PORT,
                    host: LOCALHOST.to_string(),
                    routes: HashMap::new(),
                }
            );

            // test constructor from
            assert_eq!(
                Blank::from($some_random_port),
                Blank {
                    tcp_port: $some_random_port,
                    host: LOCALHOST.to_string(),
                    routes: HashMap::new(),
                }
            );

            // test constructor default
            // should be same as new()
            // new() is already tested
            assert_eq!(Blank::default(), Blank::new());
        }

        // SERVER TESTS

        // Tests whether Blank can successfully bind to a port
        // and returns the appropriate error when the port is occupied.
        #[test]
        fn server_bind_failure_test() {
            let port = $some_random_port + 11;

            // Occupy the port before starting Blank.
            let _occupied_listener = TcpListener::bind(format!("{}:{}", LOCALHOST, port)).unwrap();

            // Blank should fail to bind to the occupied port.
            assert_eq!(Blank::from(port).serve(), Err(ERROR_BIND_SERVER));
        }

        // Tests whether the server can successfully handle
        // a valid HTTP GET request.
        $crate::http_test_case!(
            server_get_test,
            $some_random_port,
            $some_random_deadline,
            b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_OK,
            "Blank test response",
        );

        // Tests whether the server accepts HTTP/1.0 requests.
        $crate::http_test_case!(
            server_http_10_test,
            $some_random_port + 1,
            $some_random_deadline,
            b"GET / HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_OK,
            "Blank test response",
        );

        // Tests whether the server rejects a request
        // with missing request-line parts.
        $crate::http_test_case!(
            server_bad_request_missing_parts_test,
            $some_random_port + 2,
            $some_random_deadline,
            b"GET\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_BAD_REQUEST,
            "",
        );

        // Tests whether the server rejects a request
        // with extra request-line parts.
        $crate::http_test_case!(
            server_bad_request_extra_parts_test,
            $some_random_port + 3,
            $some_random_deadline,
            b"GET / HTTP/1.1 EXTRA\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_BAD_REQUEST,
            "",
        );

        // Tests whether the server rejects an empty request line.
        $crate::http_test_case!(
            server_bad_request_empty_request_line_test,
            $some_random_port + 4,
            $some_random_deadline,
            b"\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_BAD_REQUEST,
            "",
        );

        // Tests whether the server rejects unsupported HTTP versions.
        $crate::http_test_case!(
            server_http_version_not_supported_test,
            $some_random_port + 5,
            $some_random_deadline,
            b"GET / HTTP/2.0\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_VERSION_NOT_SUPPORTED,
            "",
        );

        // Tests whether the server recognizes registered HTTP methods
        // that are not currently implemented by Blank.
        $crate::http_test_case!(
            server_method_not_implemented_test,
            $some_random_port + 6,
            $some_random_deadline,
            b"POST / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_NOT_IMPLEMENTED,
            "",
        );

        // Tests whether the server rejects unregistered HTTP methods.
        $crate::http_test_case!(
            server_invalid_method_test,
            $some_random_port + 7,
            $some_random_deadline,
            b"G@T / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_METHOD_NOT_ALLOWED,
            "",
        );

        // Tests whether HTTP methods are checked with the correct casing.
        $crate::http_test_case!(
            server_invalid_method_lowercase_test,
            $some_random_port + 8,
            $some_random_deadline,
            b"get / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_METHOD_NOT_ALLOWED,
            "",
        );
    };
}

// tests.rs
//! This module helps Blank to run tests
//! when writing the framework.
//!
//! Only Rust tests.
//!
//! Current tests:
//!     - hello_test
//!     - blank_constructor_test (as a macro)
//!     - blank_configuration_test
//!     - blank_route_registry_test
//!     - blank_request_default_test
//!     - blank_request_accessors_test
//!     - blank_request_get_method_test
//!     - blank_request_binary_body_test
//!     - blank_request_query_edge_tests
//!     - blank_request_form_content_type_tests
//!     - blank_output_types_test
//!     - blank_response_builder_test
//!     - blank_response_statuses_test
//!     - blank_response_headers_test
//!     - blank_response_cookies_test
//!     - blank_response_redirects_test
//!     - blank_response_content_type_reset_test
//!     - server_bind_failure_test
//!     - server_get_test
//!     - server_post_test
//!     - server_post_form_test
//!     - server_post_multipart_test
//!     - server_post_json_test
//!     - server_binary_response_test
//!     - server_response_headers_test
//!     - server_route_snapshot_test
//!     - server_async_invalid_host_test
//!     - server_http_10_test
//!     - server_bad_request_missing_parts_test
//!     - server_bad_request_extra_parts_test
//!     - server_bad_request_empty_request_line_test
//!     - server_bad_request_invalid_target_test
//!     - server_bad_request_invalid_header_test
//!     - server_http_version_not_supported_test
//!     - server_method_not_implemented_test
//!     - server_invalid_method_test
//!     - server_invalid_method_lowercase_test
//!     - server_route_not_found_test
//!     - server_response_status_codes_test
//!     - server_maximum_body_accepted_test
//!     - server_payload_too_large_test
//!     - server_headers_too_large_test
//!     - server_transfer_encoding_not_implemented_test
//!     - server_content_length_tests
//!     - server_query_and_form_error_tests
//!     - server_method_classification_test
//!     - server_supported_versions_test
//!     - server_method_version_matrix_test
//!     - server_async_lifecycle_tests
//!     - server_async_drop_test
//!     - server_concurrent_requests_test
//!     - request_parser_tests
//!     - request_parser_additional_edge_tests
//!     - request_body_metadata_tests
//!     - request_body_metadata_boundary_tests
//!     - request_body_size_boundary_tests
//!     - request_reader_tests
//!     - urlencoded_parser_tests
//!     - form_field_limit_tests
//!     - multipart_parser_tests
//!     - internal_helper_tests
//!     - header_and_cookie_validation_tests
//!     - request_read_error_status_tests
//!     - service_unavailable_response_test
//!     - server_error_constants_test
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
        use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
        use std::thread;
        use std::time::{Duration, Instant};

        // use super for crate imports and private server items
        use super::{
            find_bytes, hex_value, insert_form_value, is_valid_cookie_name,
            is_valid_cookie_value, is_valid_header_name, is_valid_header_value,
            loopback_address, normalize_route, parse_http_request, parse_multipart_form,
            parse_urlencoded, read_http_request, reap_workers, request_body_metadata, url_decode,
            write_service_unavailable,
            Blank, BlankBody, BlankForms, BlankHTML, BlankJSON, BlankReq,
            BlankRes, BlankServer, BlankText, BlankView, RequestReadError,
        };
        use super::{
            BlankError, CONTENT_TYPE_FORM_URLENCODED, CONTENT_TYPE_HTML, CONTENT_TYPE_JSON,
            CONTENT_TYPE_MULTIPART_FORM_DATA, CONTENT_TYPE_OCTET_STREAM, CONTENT_TYPE_TEXT,
            DEFAULT_PORT, ERROR_ACCEPT_CONNECTION, ERROR_BIND_SERVER,
            ERROR_SERVER_THREAD_PANIC, ERROR_SPAWN_SERVER_THREAD, HTTP_BAD_GATEWAY,
            HTTP_BAD_REQUEST, HTTP_CREATED, HTTP_FORBIDDEN, HTTP_FOUND,
            HTTP_GATEWAY_TIMEOUT, HTTP_HEADER_FIELDS_TOO_LARGE, HTTP_INTERNAL_SERVER_ERROR,
            HTTP_METHOD_NOT_ALLOWED, HTTP_MOVED_PERMANENTLY, HTTP_NO_CONTENT, HTTP_NOT_FOUND,
            HTTP_NOT_IMPLEMENTED, HTTP_OK, HTTP_PAYLOAD_TOO_LARGE, HTTP_REQUEST_TIMEOUT,
            HTTP_SERVICE_UNAVAILABLE, HTTP_TOO_MANY_REQUESTS, HTTP_UNAUTHORIZED,
            HTTP_VERSION_NOT_SUPPORTED, LOCALHOST, MAX_FORM_FIELDS, MAX_FORM_FIELD_SIZE,
            MAX_HTTP_BODY_SIZE, MAX_HTTP_HEADER_SIZE, REAL_HTTP_METHODS,
            SUPPORTED_HTTP_METHODS, SUPPORTED_HTTP_VERSIONS
        };

        // A simple route used by HTTP tests.
        fn test_view(_rq: &BlankReq, out: &mut BlankRes) {
            out.text("Blank test response");
        }

        // A route used to test binary response bodies.
        fn binary_view(_rq: &BlankReq, out: &mut BlankRes) {
            out.binary(vec![0x00, 0x7f, 0x80, 0xff]);
        }

        // A route used to test response headers and cookies over TCP.
        fn response_headers_view(_rq: &BlankReq, out: &mut BlankRes) {
            out.header("X-Custom", "one")
                .headers("X-Custom", "two")
                .cookie("session", "abc123")
                .clear_cookie("old_session")
                .text("Header response");
        }

        // A route used to test route replacement.
        fn replacement_view(_rq: &BlankReq, out: &mut BlankRes) {
            out.text("Replacement response");
        }

        // A route that returns the values parsed by BlankReq.
        fn request_view(rq: &BlankReq, out: &mut BlankRes) {
            let repeated_headers = rq
                .header_values("x-repeat")
                .unwrap_or(&[])
                .join("|");
            let query_values = rq
                .query_values("tag")
                .unwrap_or(&[])
                .join("|");
            let form_values = rq
                .form_values("tag")
                .unwrap_or(&[])
                .join("|");

            out.text(format!(
                "method={};route={};version={};host={};repeat={};query_from={};query_tag={};body={};content_type={};form_name={};form_tag={};is_get={};is_post={}",
                rq.method(),
                rq.route(),
                rq.version(),
                rq.header("host").unwrap_or(""),
                repeated_headers,
                rq.query_value("from").unwrap_or(""),
                query_values,
                rq.body_text().unwrap_or("<binary>"),
                rq.content_type().unwrap_or(""),
                rq.form("name").unwrap_or(""),
                form_values,
                rq.is_get(),
                rq.is_post(),
            ));
        }

        // A route used to test every response status.
        fn status_view(rq: &BlankReq, out: &mut BlankRes) {
            match rq.route() {
                "/ok" => {
                    out.ok().text("OK view");
                }
                "/created" => {
                    out.created().text("Created view");
                }
                "/no-content" => {
                    out.no_content();
                }
                "/moved-permanently" => {
                    out.permanent_redirect("/destination");
                }
                "/found" => {
                    out.redirect("/destination");
                }
                "/bad-request" => {
                    out.bad_request();
                }
                "/unauthorized" => {
                    out.unauthorized();
                }
                "/forbidden" => {
                    out.forbidden();
                }
                "/method-not-allowed" => {
                    out.method_not_allowed();
                }
                "/request-timeout" => {
                    out.request_timeout();
                }
                "/payload-too-large" => {
                    out.payload_too_large();
                }
                "/too-many-requests" => {
                    out.too_many_requests();
                }
                "/header-fields-too-large" => {
                    out.status(HTTP_HEADER_FIELDS_TOO_LARGE)
                        .text("Request headers too large");
                }
                "/internal-server-error" => {
                    out.internal_server_error();
                }
                "/not-implemented" => {
                    out.status(HTTP_NOT_IMPLEMENTED).text("Not Implemented");
                }
                "/bad-gateway" => {
                    out.bad_gateway();
                }
                "/service-unavailable" => {
                    out.service_unavailable();
                }
                "/gateway-timeout" => {
                    out.gateway_timeout();
                }
                "/version-not-supported" => {
                    out.status(HTTP_VERSION_NOT_SUPPORTED)
                        .text("HTTP Version Not Supported");
                }
                _ => {
                    out.text("Unknown status route");
                }
            }
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
            run_http_test_with_view(
                port,
                deadline,
                request,
                expected_status,
                expected_content,
                "/",
                test_view,
                false,
            );
        }

        /// Shared function for running an HTTP test with a selected view.
        fn run_http_test_with_view(
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

            // registered route
            route: &str,

            // view function
            view: BlankView,

            // whether the client should close its write side
            close_write: bool,
        ) {
            let mut server = Blank::from(port);
            server.host("127.0.0.1").route_func(route, view);

            // Start the server in a managed background thread.
            let server_handle = server.serve_async().unwrap();

            // The actual address is available even when port zero was supplied.
            let addr = server_handle.local_addr();
            let deadline_at = Instant::now() + Duration::from_millis(deadline);

            // Wait for the server to become available.
            let mut client = loop {
                let now = Instant::now();

                assert!(
                    now < deadline_at,
                    "Server did not become available on port {}",
                    addr.port()
                );

                let timeout = (deadline_at - now).min(Duration::from_millis(50));

                match TcpStream::connect_timeout(&addr, timeout) {
                    Ok(client) => break client,
                    Err(_) => thread::sleep(Duration::from_millis(5)),
                }
            };

            // Prevent the test from waiting forever if the server does not return.
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            client
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            // Request an endpoint and read the response.
            client.write_all(request).unwrap();

            if close_write {
                let _ = client.shutdown(Shutdown::Write);
            }

            // Read the response.
            let mut res = String::new();
            client.read_to_string(&mut res).unwrap();

            // Stop the client connection.
            let _ = client.shutdown(Shutdown::Both);

            // Stop and join the server instead of leaking a server thread.
            server_handle.async_stop().unwrap();

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

        /// Runs several requests against one server and checks every response.
        fn run_http_batch_test(cases: &[(&str, &str, &str)]) {
            let mut server = Blank::new();
            server.host("127.0.0.1").port(0);

            // Register every route except the deliberate missing-route case.
            for (route, _, _) in cases {
                if *route != "/missing" {
                    server.route_func(route, status_view);
                }
            }

            let server_handle = server.serve_async().unwrap();
            let addr = server_handle.local_addr();

            for (route, expected_status, expected_content) in cases {
                let request = format!(
                    "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                    route
                );
                let mut client = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
                    .unwrap_or_else(|error| panic!("Could not connect to {}: {}", addr, error));
                client
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                client.write_all(request.as_bytes()).unwrap();

                let mut response = String::new();
                client.read_to_string(&mut response).unwrap();
                let _ = client.shutdown(Shutdown::Both);

                let expected_status = format!("HTTP/1.1 {}", expected_status);
                assert!(
                    response.starts_with(&expected_status),
                    "Expected {}, got:\n{}",
                    expected_status,
                    response
                );

                if !expected_content.is_empty() {
                    assert!(
                        response.contains(*expected_content),
                        "Expected response content {}, got:\n{}",
                        expected_content,
                        response
                    );
                }
            }

            server_handle.async_stop().unwrap();
        }

        /// Sends one request and returns the complete response as bytes.
        fn send_raw_request(address: SocketAddr, request: &[u8]) -> Vec<u8> {
            let mut client = TcpStream::connect_timeout(&address, Duration::from_secs(2))
                .unwrap_or_else(|error| panic!("Could not connect to {}: {}", address, error));

            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            client
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            client.write_all(request).unwrap();

            let mut response = Vec::new();
            client.read_to_end(&mut response).unwrap();
            let _ = client.shutdown(Shutdown::Both);
            response
        }

        /// Starts a temporary server, sends one request, and returns raw response bytes.
        fn run_http_bytes_test(request: &[u8], route: &str, view: BlankView) -> Vec<u8> {
            let mut server = Blank::new();
            server.host("127.0.0.1").port(0).route_func(route, view);

            let server_handle = server.serve_async().unwrap();
            let response = send_raw_request(server_handle.local_addr(), request);
            server_handle.async_stop().unwrap();
            response
        }

        /// Returns a parsed request for parser and accessor tests.
        fn parse_request(request: &[u8]) -> Result<BlankReq, &'static str> {
            parse_http_request(request).map(|parsed| parsed.request)
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

        // Tests whether Blank's host and port configuration are working.
        #[test]
        fn blank_configuration_test() {
            let mut server = Blank::new();

            assert_eq!(server.get_host(), LOCALHOST);
            assert_eq!(server.get_port(), DEFAULT_PORT);

            server.host("127.0.0.1").port(8088);

            assert_eq!(server.get_host(), "127.0.0.1");
            assert_eq!(server.get_port(), 8088);
        }

        // Tests whether route registration, replacement, lookup, and removal work.
        #[test]
        fn blank_route_registry_test() {
            let mut server = Blank::new();

            assert_eq!(server.route_count(), 0);
            assert!(!server.has_route("/"));

            // Routes are normalized to begin with a slash.
            server.route_func("health", test_view);
            assert!(server.has_route("/health"));
            assert!(server.has_route("health"));
            assert_eq!(server.route_count(), 1);

            // Registering an existing route replaces its previous view.
            server.route_func("/health", request_view);
            assert_eq!(server.route_count(), 1);

            // Removing a route works with either route format.
            assert!(server.remove_route("health"));
            assert!(!server.has_route("/health"));
            assert!(!server.remove_route("/health"));

            // Clearing routes removes every registered route.
            server.route_func("one", test_view).route_func("two", test_view);
            assert_eq!(server.route_count(), 2);
            assert_eq!(server.clear_routes().route_count(), 0);
        }

        // Tests whether BlankReq's default value and every accessor work.
        #[test]
        fn blank_request_default_test() {
            let request = BlankReq::default();

            assert_eq!(request.method(), "");
            assert_eq!(request.route(), "");
            assert_eq!(request.version(), "");
            assert!(request.headers().is_empty());
            assert_eq!(request.header("host"), None);
            assert_eq!(request.header_values("host"), None);
            assert!(request.body().is_empty());
            assert_eq!(request.body_text(), Ok(""));
            assert_eq!(request.content_type(), None);
            assert!(request.query().is_empty());
            assert_eq!(request.query_value("name"), None);
            assert_eq!(request.query_values("name"), None);
            assert!(request.forms().is_empty());
            assert_eq!(request.form("name"), None);
            assert_eq!(request.form_values("name"), None);
            assert!(!request.is_get());
            assert!(!request.is_post());
        }

        // Tests whether BlankReq parses and exposes every request component.
        #[test]
        fn blank_request_accessors_test() {
            let raw = b"POST /submit?from=browser&tag=one&tag=two HTTP/1.1\r\nHost: localhost\r\nX-Repeat: first\r\nx-repeat: second\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"ok\":true}";
            let request = parse_request(raw).unwrap();

            assert_eq!(request.method(), "POST");
            assert_eq!(request.route(), "/submit");
            assert_eq!(request.version(), "HTTP/1.1");
            assert_eq!(request.header("HOST"), Some("localhost"));
            assert_eq!(request.header_values("x-repeat").unwrap().len(), 2);
            assert_eq!(request.header("X-REPEAT"), Some("first"));
            assert_eq!(request.headers().get("x-repeat").unwrap().len(), 2);

            // Deref keeps the previous header-map access pattern working.
            assert_eq!(request.get("host").unwrap()[0], "localhost");

            assert_eq!(request.body(), b"{\"ok\":true}");
            assert_eq!(request.body_text().unwrap(), "{\"ok\":true}");
            assert_eq!(request.content_type(), Some("application/json"));

            assert_eq!(request.query_value("from"), Some("browser"));
            assert_eq!(request.query_values("tag").unwrap(), &["one", "two"]);
            assert_eq!(request.query().get("tag").unwrap().len(), 2);

            assert!(request.forms().is_empty());
            assert_eq!(request.form("missing"), None);
            assert_eq!(request.form_values("missing"), None);
            assert!(!request.is_get());
            assert!(request.is_post());
        }

        // Tests whether GET requests are identified correctly.
        #[test]
        fn blank_request_get_method_test() {
            let request = parse_request(b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();

            assert!(request.is_get());
            assert!(!request.is_post());
            assert_eq!(request.method(), "GET");
            assert_eq!(request.version(), "HTTP/1.0");
        }

        // Tests whether binary request bodies remain intact when text decoding fails.
        #[test]
        fn blank_request_binary_body_test() {
            let request = parse_request(
                b"POST /upload HTTP/1.1\r\nContent-Length: 3\r\nContent-Type: application/octet-stream\r\n\r\n\xff\x00A",
            )
            .unwrap();

            assert_eq!(request.body(), &[0xff, 0x00, b'A']);
            assert!(request.body_text().is_err());
            assert_eq!(request.content_type(), Some(CONTENT_TYPE_OCTET_STREAM));
            assert!(request.is_post());
        }

        // Tests whether BlankRes defaults and response body types are working.
        #[test]
        fn blank_output_types_test() {
            let json = BlankJSON::new("{\"ok\":true}");
            let html = BlankHTML::new("<h1>Blank</h1>");
            let text = BlankText::new("Blank");

            assert_eq!(json.content(), "{\"ok\":true}");
            assert_eq!(html.content(), "<h1>Blank</h1>");
            assert_eq!(text.content(), "Blank");
            assert_eq!(BlankJSON::CONTENT_TYPE, CONTENT_TYPE_JSON);
            assert_eq!(BlankHTML::CONTENT_TYPE, CONTENT_TYPE_HTML);
            assert_eq!(BlankText::CONTENT_TYPE, CONTENT_TYPE_TEXT);

            assert_eq!(BlankBody::Json(json).content_type(), CONTENT_TYPE_JSON);
            assert_eq!(BlankBody::Html(html).content_type(), CONTENT_TYPE_HTML);
            assert_eq!(BlankBody::Text(text).content_type(), CONTENT_TYPE_TEXT);
            let binary = BlankBody::Binary(vec![0, 1, 255]);
            assert_eq!(binary.content(), &[0, 1, 255]);
            assert_eq!(binary.content_type(), CONTENT_TYPE_OCTET_STREAM);
        }

        // Tests whether every BlankRes body builder is working.
        #[test]
        fn blank_response_builder_test() {
            let mut response = BlankRes::new();
            assert_eq!(response, BlankRes::default());
            assert_eq!(response.status, HTTP_OK);
            assert!(response.body().is_empty());
            assert_eq!(response.response_content_type(), CONTENT_TYPE_TEXT);

            response.json("{}");
            assert_eq!(response.body(), b"{}");
            assert_eq!(response.response_content_type(), CONTENT_TYPE_JSON);

            response.html("<p>Blank</p>");
            assert_eq!(response.body(), b"<p>Blank</p>");
            assert_eq!(response.response_content_type(), CONTENT_TYPE_HTML);

            response.text("Hello");
            assert_eq!(response.body(), b"Hello");
            assert_eq!(response.response_content_type(), CONTENT_TYPE_TEXT);

            response.binary(vec![0, 1, 2, 255]);
            assert_eq!(response.body(), &[0, 1, 2, 255]);
            assert_eq!(response.response_content_type(), CONTENT_TYPE_OCTET_STREAM);

            response.content_type("application/custom");
            assert_eq!(response.response_content_type(), "application/custom");

            // Invalid content types must not replace the previous value.
            response.content_type("text/plain\r\nInjected: true");
            assert_eq!(response.response_content_type(), "application/custom");
        }

        // Tests whether every HTTP status builder and status code is available.
        #[test]
        fn blank_response_statuses_test() {
            let statuses = [
                HTTP_OK,
                HTTP_CREATED,
                HTTP_NO_CONTENT,
                HTTP_MOVED_PERMANENTLY,
                HTTP_FOUND,
                HTTP_BAD_REQUEST,
                HTTP_UNAUTHORIZED,
                HTTP_FORBIDDEN,
                HTTP_NOT_FOUND,
                HTTP_METHOD_NOT_ALLOWED,
                HTTP_REQUEST_TIMEOUT,
                HTTP_PAYLOAD_TOO_LARGE,
                HTTP_TOO_MANY_REQUESTS,
                HTTP_HEADER_FIELDS_TOO_LARGE,
                HTTP_INTERNAL_SERVER_ERROR,
                HTTP_NOT_IMPLEMENTED,
                HTTP_BAD_GATEWAY,
                HTTP_SERVICE_UNAVAILABLE,
                HTTP_GATEWAY_TIMEOUT,
                HTTP_VERSION_NOT_SUPPORTED,
            ];

            // Every declared status can be stored without being modified.
            for status in statuses {
                let mut response = BlankRes::new();
                response.status(status);
                assert_eq!(response.status, status);
            }

            let mut response = BlankRes::new();
            assert_eq!(response.ok().status, HTTP_OK);
            assert_eq!(response.created().status, HTTP_CREATED);
            response.text("remove this body");
            assert_eq!(response.no_content().status, HTTP_NO_CONTENT);
            assert!(response.body().is_empty());

            assert_eq!(response.bad_request().status, HTTP_BAD_REQUEST);
            assert_eq!(response.unauthorized().status, HTTP_UNAUTHORIZED);
            assert_eq!(response.forbidden().status, HTTP_FORBIDDEN);
            assert_eq!(response.not_found().status, HTTP_NOT_FOUND);
            assert_eq!(response.method_not_allowed().status, HTTP_METHOD_NOT_ALLOWED);
            assert_eq!(response.request_timeout().status, HTTP_REQUEST_TIMEOUT);
            assert_eq!(
                response.payload_too_large().status,
                HTTP_PAYLOAD_TOO_LARGE
            );
            assert_eq!(response.too_many_requests().status, HTTP_TOO_MANY_REQUESTS);
            assert_eq!(response.internal_server_error().status, HTTP_INTERNAL_SERVER_ERROR);
            assert_eq!(response.bad_gateway().status, HTTP_BAD_GATEWAY);
            assert_eq!(response.service_unavailable().status, HTTP_SERVICE_UNAVAILABLE);
            assert_eq!(response.gateway_timeout().status, HTTP_GATEWAY_TIMEOUT);
        }

        // Tests whether response headers can be added, replaced, and removed.
        #[test]
        fn blank_response_headers_test() {
            let mut response = BlankRes::new();

            response.header("X-Test", "one").headers("X-Test", "two");
            assert_eq!(response.header_values("x-test").unwrap(), &["one", "two"]);

            response.set_header("X-Test", "replacement");
            assert_eq!(
                response.header_values("X-TEST").unwrap(),
                &["replacement"]
            );

            // Invalid names and values must not enter the response.
            response.header("Invalid Header", "value");
            response.header("X-Newline", "first\r\nInjected: true");
            response.set_header("Bad:Name", "value");
            assert_eq!(response.header_values("invalid header"), None);
            assert_eq!(response.header_values("x-newline"), None);
            assert_eq!(response.header_values("bad:name"), None);

            response.remove_header("X-TEST");
            assert_eq!(response.header_values("x-test"), None);

            // Removing a header that does not exist is also safe.
            response.remove_header("missing");
            assert_eq!(response.header_values("missing"), None);
        }

        // Tests whether response cookies are created and cleared safely.
        #[test]
        fn blank_response_cookies_test() {
            let mut response = BlankRes::new();

            response.cookie("session", "abc123");
            response.cookie("theme", "dark");
            response.clear_cookie("session");

            let cookies = response.header_values("set-cookie").unwrap();
            assert_eq!(cookies.len(), 3);
            assert_eq!(
                cookies[0],
                "session=abc123; Path=/; HttpOnly; SameSite=Lax"
            );
            assert_eq!(cookies[1], "theme=dark; Path=/; HttpOnly; SameSite=Lax");
            assert!(cookies[2].starts_with("session=; Path=/; Max-Age=0"));

            // Unsafe cookie names and values are ignored.
            response.cookie("bad name", "value");
            response.cookie("safe", "value; Secure");
            response.clear_cookie("bad\nname");
            assert_eq!(response.header_values("set-cookie").unwrap().len(), 3);
        }

        // Tests whether redirects set the appropriate status and location.
        #[test]
        fn blank_response_redirects_test() {
            let mut response = BlankRes::new();

            response.permanent_redirect("/permanent");
            assert_eq!(response.status, HTTP_MOVED_PERMANENTLY);
            assert_eq!(response.header_values("location").unwrap(), &["/permanent"]);
            assert!(response.body().is_empty());

            response.redirect("/temporary");
            assert_eq!(response.status, HTTP_FOUND);
            assert_eq!(response.header_values("location").unwrap(), &["/temporary"]);
            assert!(response.body().is_empty());

            // Unsafe locations must not change the current response.
            response.redirect("/bad\r\nInjected: true");
            assert_eq!(response.status, HTTP_FOUND);
            assert_eq!(response.header_values("location").unwrap(), &["/temporary"]);

            // Permanent redirects reject unsafe locations as well.
            response.permanent_redirect("/bad\nInjected: true");
            assert_eq!(response.status, HTTP_FOUND);
            assert_eq!(response.header_values("location").unwrap(), &["/temporary"]);
        }

        // Tests whether changing a response body restores its default content type.
        #[test]
        fn blank_response_content_type_reset_test() {
            let mut response = BlankRes::new();

            response.text("first").content_type("application/custom");
            assert_eq!(response.response_content_type(), "application/custom");

            response.json("{}");
            assert_eq!(response.response_content_type(), CONTENT_TYPE_JSON);

            response.content_type("application/custom").html("<p>Blank</p>");
            assert_eq!(response.response_content_type(), CONTENT_TYPE_HTML);

            response.content_type("application/custom").binary(vec![0, 255]);
            assert_eq!(response.response_content_type(), CONTENT_TYPE_OCTET_STREAM);

            response.content_type("application/custom").text("last");
            assert_eq!(response.response_content_type(), CONTENT_TYPE_TEXT);
        }

        // SERVER TESTS

        // Tests whether Blank can successfully bind to a port
        // and returns the appropriate error when the port is occupied.
        #[test]
        fn server_bind_failure_test() {
            // Let the operating system select an unused port, then occupy it.
            let occupied_listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = occupied_listener.local_addr().unwrap().port();

            // Configure Blank to use the exact same address.
            let mut server = Blank::from(port);
            server.host("127.0.0.1");

            // Every server entry point must report the bind failure.
            assert_eq!(server.serve(), Err(ERROR_BIND_SERVER));
            assert_eq!(server.serve_await(), Err(ERROR_BIND_SERVER));
            assert_eq!(server.serve_async().err(), Some(ERROR_BIND_SERVER));

            drop(occupied_listener);
        }

        // Tests whether the server can successfully handle a valid HTTP GET request.
        $crate::http_test_case!(
            server_get_test,
            0,
            $some_random_deadline,
            b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_OK,
            "Blank test response",
        );

        // Tests whether the server can successfully handle a valid HTTP POST request.
        $crate::http_test_case!(
            server_post_test,
            0,
            $some_random_deadline,
            b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            HTTP_OK,
            "Blank test response",
        );

        // Tests whether the server parses URL-encoded POST forms and query values.
        #[test]
        fn server_post_form_test() {
            let body = "name=Kyam+Dev&tag=one&tag=two";
            let request = format!(
                "POST /?from=browser&tag=a&tag=b HTTP/1.1\r\nHost: localhost\r\nX-Repeat: first\r\nx-repeat: second\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                CONTENT_TYPE_FORM_URLENCODED,
                body.len(),
                body
            );

            run_http_test_with_view(
                0,
                $some_random_deadline,
                request.as_bytes(),
                HTTP_OK,
                "method=POST;route=/;version=HTTP/1.1;host=localhost;repeat=first|second;query_from=browser;query_tag=a|b;body=name=Kyam+Dev&tag=one&tag=two;content_type=application/x-www-form-urlencoded;form_name=Kyam Dev;form_tag=one|two;is_get=false;is_post=true",
                "/",
                request_view,
                false,
            );
        }

        // Tests whether multipart form fields are parsed and file fields are skipped.
        #[test]
        fn server_post_multipart_test() {
            let boundary = "blank-test-boundary";
            let body = format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\nAlice\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"tag\"\r\n\r\none\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"tag\"\r\n\r\ntwo\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"upload\"; filename=\"note.txt\"\r\nContent-Type: text/plain\r\n\r\nignored file bytes\r\n--{boundary}--\r\n"
            );
            let request = format!(
                "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Type: {}; boundary={}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                CONTENT_TYPE_MULTIPART_FORM_DATA,
                boundary,
                body.len(),
                body
            );

            run_http_test_with_view(
                0,
                $some_random_deadline,
                request.as_bytes(),
                HTTP_OK,
                "form_name=Alice;form_tag=one|two;is_get=false;is_post=true",
                "/",
                request_view,
                false,
            );
        }

        // Tests whether raw JSON POST bodies remain available through BlankReq.
        #[test]
        fn server_post_json_test() {
            let body = "{\"name\":\"Blank\"}";
            let request = format!(
                "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                CONTENT_TYPE_JSON,
                body.len(),
                body
            );

            run_http_test_with_view(
                0,
                $some_random_deadline,
                request.as_bytes(),
                HTTP_OK,
                "body={\"name\":\"Blank\"};content_type=application/json; charset=utf-8;form_name=;form_tag=;is_get=false;is_post=true",
                "/",
                request_view,
                false,
            );
        }

        // Tests whether binary responses preserve every byte over TCP.
        #[test]
        fn server_binary_response_test() {
            let response = run_http_bytes_test(
                b"GET /binary HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                "/binary",
                binary_view,
            );

            let header_end = find_bytes(&response, b"\r\n\r\n").unwrap();
            let headers = std::str::from_utf8(&response[..header_end]).unwrap();
            let body = &response[header_end + 4..];

            assert!(headers.starts_with("HTTP/1.1 200 OK"));
            assert!(headers.contains("Content-Length: 4"));
            assert!(headers.contains("Content-Type: application/octet-stream"));
            assert!(headers.contains("Connection: close"));
            assert_eq!(body, &[0x00, 0x7f, 0x80, 0xff]);
        }

        // Tests whether custom response headers and cookies are serialized correctly.
        #[test]
        fn server_response_headers_test() {
            let response = run_http_bytes_test(
                b"GET /headers HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                "/headers",
                response_headers_view,
            );

            let response = String::from_utf8(response).unwrap();
            assert!(response.starts_with("HTTP/1.1 200 OK"));
            assert!(response.contains("x-custom: one\r\n"));
            assert!(response.contains("x-custom: two\r\n"));
            assert!(response.contains("set-cookie: session=abc123; Path=/; HttpOnly; SameSite=Lax\r\n"));
            assert!(response.contains("set-cookie: old_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax\r\n"));
            assert!(response.ends_with("Header response"));
        }

        // Tests whether a running server keeps its route snapshot unchanged.
        #[test]
        fn server_route_snapshot_test() {
            let mut server = Blank::new();
            server
                .host("127.0.0.1")
                .port(0)
                .route_func("replace", test_view)
                .route_func("existing", test_view);

            // Replacing a route before startup must use the newest view.
            server.route_func("/replace", replacement_view);

            let server_handle = server.serve_async().unwrap();
            let address = server_handle.local_addr();

            // Routes added after startup belong to the next server instance.
            server.route_func("late", test_view);

            let replaced = send_raw_request(
                address,
                b"GET /replace HTTP/1.1\r\nHost: localhost\r\n\r\n",
            );
            let existing = send_raw_request(
                address,
                b"GET /existing HTTP/1.1\r\nHost: localhost\r\n\r\n",
            );
            let late = send_raw_request(
                address,
                b"GET /late HTTP/1.1\r\nHost: localhost\r\n\r\n",
            );

            let replaced = String::from_utf8(replaced).unwrap();
            let existing = String::from_utf8(existing).unwrap();
            let late = String::from_utf8(late).unwrap();

            assert!(replaced.starts_with("HTTP/1.1 200 OK"));
            assert!(replaced.contains("Replacement response"));
            assert!(existing.starts_with("HTTP/1.1 200 OK"));
            assert!(existing.contains("Blank test response"));
            assert!(late.starts_with("HTTP/1.1 404 Not Found"));

            server_handle.async_stop().unwrap();
        }

        // Tests whether invalid host configuration returns the bind error.
        #[test]
        fn server_async_invalid_host_test() {
            let mut server = Blank::new();
            server.host("127.0.0.1\0").port(0);

            assert_eq!(server.serve_async().err(), Some(ERROR_BIND_SERVER));
        }

        // Tests whether the server accepts HTTP/1.0 requests.
        $crate::http_test_case!(
            server_http_10_test,
            0,
            $some_random_deadline,
            b"GET / HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_OK,
            "Blank test response",
        );

        // Tests whether the server rejects requests with missing request-line parts.
        $crate::http_test_case!(
            server_bad_request_missing_parts_test,
            0,
            $some_random_deadline,
            b"GET\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_BAD_REQUEST,
            "Bad Request",
        );

        // Tests whether the server rejects requests with extra request-line parts.
        $crate::http_test_case!(
            server_bad_request_extra_parts_test,
            0,
            $some_random_deadline,
            b"GET / HTTP/1.1 EXTRA\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_BAD_REQUEST,
            "Bad Request",
        );

        // Tests whether the server rejects an empty request line.
        $crate::http_test_case!(
            server_bad_request_empty_request_line_test,
            0,
            $some_random_deadline,
            b"\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_BAD_REQUEST,
            "Bad Request",
        );

        // Tests whether the server rejects request targets that are not origin-form paths.
        $crate::http_test_case!(
            server_bad_request_invalid_target_test,
            0,
            $some_random_deadline,
            b"GET https://example.com/ HTTP/1.1\r\nHost: localhost\r\n\r\n",
            HTTP_BAD_REQUEST,
            "Bad Request",
        );

        // Tests whether the server rejects malformed request headers.
        $crate::http_test_case!(
            server_bad_request_invalid_header_test,
            0,
            $some_random_deadline,
            b"GET / HTTP/1.1\r\nInvalid Header: value\r\n\r\n",
            HTTP_BAD_REQUEST,
            "Bad Request",
        );

        // Tests whether the server rejects unsupported HTTP versions.
        $crate::http_test_case!(
            server_http_version_not_supported_test,
            0,
            $some_random_deadline,
            b"GET / HTTP/2.0\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_VERSION_NOT_SUPPORTED,
            "HTTP Version Not Supported",
        );

        // Tests whether registered HTTP methods that Blank does not implement return 501.
        // POST is now supported, so PUT is used here instead.
        $crate::http_test_case!(
            server_method_not_implemented_test,
            0,
            $some_random_deadline,
            b"PUT / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_NOT_IMPLEMENTED,
            "Not Implemented",
        );

        // Tests whether the server rejects unregistered HTTP methods.
        $crate::http_test_case!(
            server_invalid_method_test,
            0,
            $some_random_deadline,
            b"G@T / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_METHOD_NOT_ALLOWED,
            "Method Not Allowed",
        );

        // Tests whether HTTP methods are checked with the correct casing.
        $crate::http_test_case!(
            server_invalid_method_lowercase_test,
            0,
            $some_random_deadline,
            b"get / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_METHOD_NOT_ALLOWED,
            "Method Not Allowed",
        );

        // Tests whether routes that are not registered return 404.
        $crate::http_test_case!(
            server_route_not_found_test,
            0,
            $some_random_deadline,
            b"GET /not-registered HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            HTTP_NOT_FOUND,
            "Not Found",
        );

        // Tests whether every response status can be serialized by the server.
        #[test]
        fn server_response_status_codes_test() {
            let cases = [
                ("/ok", HTTP_OK, "OK view"),
                ("/created", HTTP_CREATED, "Created view"),
                ("/no-content", HTTP_NO_CONTENT, "Content-Length: 0"),
                ("/moved-permanently", HTTP_MOVED_PERMANENTLY, "location: /destination"),
                ("/found", HTTP_FOUND, "location: /destination"),
                ("/bad-request", HTTP_BAD_REQUEST, "Bad Request"),
                ("/unauthorized", HTTP_UNAUTHORIZED, "Unauthorized"),
                ("/forbidden", HTTP_FORBIDDEN, "Forbidden"),
                ("/missing", HTTP_NOT_FOUND, "Not Found"),
                ("/method-not-allowed", HTTP_METHOD_NOT_ALLOWED, "Method Not Allowed"),
                ("/request-timeout", HTTP_REQUEST_TIMEOUT, "Request Timeout"),
                ("/payload-too-large", HTTP_PAYLOAD_TOO_LARGE, "Content Too Large"),
                ("/too-many-requests", HTTP_TOO_MANY_REQUESTS, "Too Many Requests"),
                (
                    "/header-fields-too-large",
                    HTTP_HEADER_FIELDS_TOO_LARGE,
                    "Request headers too large",
                ),
                (
                    "/internal-server-error",
                    HTTP_INTERNAL_SERVER_ERROR,
                    "Internal Server Error",
                ),
                ("/not-implemented", HTTP_NOT_IMPLEMENTED, "Not Implemented"),
                ("/bad-gateway", HTTP_BAD_GATEWAY, "Bad Gateway"),
                ("/service-unavailable", HTTP_SERVICE_UNAVAILABLE, "Service Unavailable"),
                ("/gateway-timeout", HTTP_GATEWAY_TIMEOUT, "Gateway Timeout"),
                (
                    "/version-not-supported",
                    HTTP_VERSION_NOT_SUPPORTED,
                    "HTTP Version Not Supported",
                ),
            ];

            run_http_batch_test(&cases);
        }

        // Tests whether a request at the maximum body size is still accepted.
        #[test]
        fn server_maximum_body_accepted_test() {
            let body = "x".repeat(MAX_HTTP_BODY_SIZE);
            let mut request = format!(
                "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            request.push_str(&body);

            run_http_test(
                0,
                $some_random_deadline,
                request.as_bytes(),
                HTTP_OK,
                "Blank test response",
            );
        }

        // Tests whether oversized request bodies return 413.
        #[test]
        fn server_payload_too_large_test() {
            let request = format!(
                "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
                MAX_HTTP_BODY_SIZE + 1
            );

            run_http_test(
                0,
                $some_random_deadline,
                request.as_bytes(),
                HTTP_PAYLOAD_TOO_LARGE,
                "Content Too Large",
            );
        }

        // Tests whether oversized request headers return 431.
        #[test]
        fn server_headers_too_large_test() {
            let value = "a".repeat(MAX_HTTP_HEADER_SIZE + 1);
            let request = format!(
                "GET / HTTP/1.1\r\nX-Large: {}\r\n\r\n",
                value
            );

            run_http_test(
                0,
                $some_random_deadline,
                request.as_bytes(),
                HTTP_HEADER_FIELDS_TOO_LARGE,
                "Request headers too large",
            );
        }

        // Tests whether unsupported transfer encodings return 501.
        $crate::http_test_case!(
            server_transfer_encoding_not_implemented_test,
            0,
            $some_random_deadline,
            b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
            HTTP_NOT_IMPLEMENTED,
            "Transfer encoding not supported",
        );

        // Tests malformed, duplicate, and incomplete Content-Length values.
        #[test]
        fn server_content_length_tests() {
            let malformed = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: nope\r\n\r\n";
            run_http_test(
                0,
                $some_random_deadline,
                malformed,
                HTTP_BAD_REQUEST,
                "Bad Request",
            );

            let duplicate = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n";
            run_http_test(
                0,
                $some_random_deadline,
                duplicate,
                HTTP_BAD_REQUEST,
                "Bad Request",
            );

            let incomplete = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nabc";
            run_http_test_with_view(
                0,
                $some_random_deadline,
                incomplete,
                HTTP_BAD_REQUEST,
                "Bad Request",
                "/",
                test_view,
                true,
            );
        }

        // Tests malformed query and URL-encoded form values.
        #[test]
        fn server_query_and_form_error_tests() {
            let malformed_query = b"GET /?name=%GG HTTP/1.1\r\nHost: localhost\r\n\r\n";
            run_http_test(
                0,
                $some_random_deadline,
                malformed_query,
                HTTP_BAD_REQUEST,
                "Bad Request",
            );

            let malformed_form = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 7\r\n\r\nname=%G";
            run_http_test(
                0,
                $some_random_deadline,
                malformed_form,
                HTTP_BAD_REQUEST,
                "Bad Request",
            );

            let malformed_multipart = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Type: multipart/form-data\r\nContent-Length: 0\r\n\r\n";
            run_http_test(
                0,
                $some_random_deadline,
                malformed_multipart,
                HTTP_BAD_REQUEST,
                "Bad Request",
            );
        }

        // Tests method classification for every registered HTTP method.
        #[test]
        fn server_method_classification_test() {
            for method in REAL_HTTP_METHODS {
                let request = format!("{} / HTTP/1.1\r\nHost: localhost\r\n\r\n", method);
                let result = parse_request(request.as_bytes());

                if SUPPORTED_HTTP_METHODS.contains(method) {
                    assert!(result.is_ok(), "Supported method {} was rejected", method);
                } else {
                    assert_eq!(
                        result,
                        Err(HTTP_NOT_IMPLEMENTED),
                        "Registered method {} should return 501",
                        method
                    );
                }
            }

            let unknown = parse_request(b"BREW / HTTP/1.1\r\nHost: localhost\r\n\r\n");
            assert_eq!(unknown, Err(HTTP_METHOD_NOT_ALLOWED));
        }

        // Tests all HTTP versions currently declared as supported.
        #[test]
        fn server_supported_versions_test() {
            for version in SUPPORTED_HTTP_VERSIONS {
                let request = format!("GET / {}\r\nHost: localhost\r\n\r\n", version);
                assert!(parse_request(request.as_bytes()).is_ok());
            }
        }

        // Tests whether the asynchronous server can be stopped and releases its port.
        #[test]
        fn server_async_lifecycle_tests() {
            let mut server = Blank::new();
            server.host("127.0.0.1").port(0).route_func("/", test_view);

            let handle: BlankServer = server.serve_async().unwrap();
            let address: SocketAddr = handle.local_addr();
            assert_ne!(address.port(), 0);

            let mut client = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            client
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();

            let mut response = String::new();
            client.read_to_string(&mut response).unwrap();
            assert!(response.starts_with("HTTP/1.1 200 OK"));
            assert!(response.contains("Blank test response"));

            // async_stop must stop and join the server thread.
            handle.async_stop().unwrap();

            // The listener must have released its address after shutdown.
            let _released = TcpListener::bind(address).unwrap();
        }

        // Tests whether dropping a BlankServer handle also stops its thread.
        #[test]
        fn server_async_drop_test() {
            let mut server = Blank::new();
            server.host("127.0.0.1").port(0);

            let handle = server.serve_async().unwrap();
            let address = handle.local_addr();
            drop(handle);

            // Drop joins the server thread and releases the listening socket.
            let _released = TcpListener::bind(address).unwrap();
        }

        // Tests whether one server can handle multiple simultaneous clients.
        #[test]
        fn server_concurrent_requests_test() {
            let mut server = Blank::new();
            server.host("127.0.0.1").port(0).route_func("/", test_view);

            let handle = server.serve_async().unwrap();
            let address = handle.local_addr();
            let mut clients = Vec::new();

            for _ in 0..8 {
                clients.push(thread::spawn(move || {
                    let mut client = TcpStream::connect_timeout(&address, Duration::from_secs(2))
                        .unwrap();
                    client
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    client
                        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                        .unwrap();

                    let mut response = String::new();
                    client.read_to_string(&mut response).unwrap();
                    assert!(response.starts_with("HTTP/1.1 200 OK"));
                    assert!(response.contains("Blank test response"));
                }));
            }

            for client in clients {
                client.join().unwrap();
            }

            handle.async_stop().unwrap();
        }

        // Tests the request parser's request-line and header edge cases.
        #[test]
        fn request_parser_tests() {
            assert_eq!(parse_request(b""), Err(HTTP_BAD_REQUEST));
            assert_eq!(parse_request(b"\r\n\r\n"), Err(HTTP_BAD_REQUEST));
            assert_eq!(
                parse_request(b"GET / HTTP/1.1 EXTRA\r\n\r\n"),
                Err(HTTP_BAD_REQUEST)
            );
            assert_eq!(
                parse_request(b"GET / HTTP/3.0\r\n\r\n"),
                Err(HTTP_VERSION_NOT_SUPPORTED)
            );
            assert_eq!(
                parse_request(b"GET https://example.com/ HTTP/1.1\r\n\r\n"),
                Err(HTTP_BAD_REQUEST)
            );
            assert_eq!(
                parse_request(b"GET / HTTP/1.1\r\nInvalid Header: value\r\n\r\n"),
                Err(HTTP_BAD_REQUEST)
            );
            assert_eq!(
                parse_request(b"GET / HTTP/1.1\r\nMissingColon\r\n\r\n"),
                Err(HTTP_BAD_REQUEST)
            );

            // Repeated headers are preserved and their names are lowercased.
            let request = parse_request(
                b"GET /path?x=1 HTTP/1.1\r\nHost: localhost\r\nX-Value: one\r\nx-value: two\r\n\r\n",
            )
            .unwrap();
            assert_eq!(request.header_values("X-VALUE").unwrap(), &["one", "two"]);
            assert!(request.headers().contains_key("x-value"));
            assert!(!request.headers().contains_key("X-Value"));

            // Additional bytes after the declared body are not part of the body.
            let request = parse_request(
                b"POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\nabcEXTRA",
            )
            .unwrap();
            assert_eq!(request.body(), b"abc");
        }

        // Tests additional request-line, header, and query edge cases.
        #[test]
        fn request_parser_additional_edge_tests() {
            // Request lines must be valid UTF-8.
            assert_eq!(
                parse_request(b"GET / \xff\r\n\r\n"),
                Err(HTTP_BAD_REQUEST)
            );

            // Asterisk-form and absolute-form targets are not supported.
            assert_eq!(
                parse_request(b"GET * HTTP/1.1\r\n\r\n"),
                Err(HTTP_BAD_REQUEST)
            );
            assert_eq!(
                parse_request(b"GET https://example.com/ HTTP/1.1\r\n\r\n"),
                Err(HTTP_BAD_REQUEST)
            );

            // Header names must be present and contain only token characters.
            assert_eq!(
                parse_request(b"GET / HTTP/1.1\r\n: empty-name\r\n\r\n"),
                Err(HTTP_BAD_REQUEST)
            );
            assert_eq!(
                parse_request(b"GET / HTTP/1.1\r\nBad@Name: value\r\n\r\n"),
                Err(HTTP_BAD_REQUEST)
            );

            // Header values are trimmed and header names are case-insensitive.
            let request = parse_request(
                b"GET /path HTTP/1.1\r\n Host :   localhost  \r\nX-Empty: \r\n\r\n",
            )
            .unwrap();
            assert_eq!(request.header("host"), Some("localhost"));
            assert_eq!(request.header("x-empty"), Some(""));

            // The first question mark separates the path from the query.
            let request = parse_request(
                b"GET /path?value=one?two&value=three HTTP/1.1\r\n\r\n",
            )
            .unwrap();
            assert_eq!(request.route(), "/path");
            assert_eq!(request.query_values("value").unwrap(), &["one?two", "three"]);

            // Unsupported versions are rejected before method classification.
            assert_eq!(
                parse_request(b"BREW / HTTP/9.9\r\n\r\n"),
                Err(HTTP_VERSION_NOT_SUPPORTED)
            );
        }

        // Tests Content-Length and Transfer-Encoding parsing.
        #[test]
        fn request_body_metadata_tests() {
            assert!(matches!(
                request_body_metadata(b"POST / HTTP/1.1\r\nContent-Length: 12\r\n"),
                Ok((12, false))
            ));
            assert!(matches!(
                request_body_metadata(b"POST / HTTP/1.1\r\ncontent-length: 0\r\n"),
                Ok((0, false))
            ));
            assert!(matches!(
                request_body_metadata(
                    b"POST / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 1\r\n"
                ),
                Err(RequestReadError::BadRequest)
            ));
            assert!(matches!(
                request_body_metadata(b"POST / HTTP/1.1\r\nContent-Length: -1\r\n"),
                Err(RequestReadError::BadRequest)
            ));
            assert!(matches!(
                request_body_metadata(b"POST / HTTP/1.1\r\nContent-Length: abc\r\n"),
                Err(RequestReadError::BadRequest)
            ));
            assert!(matches!(
                request_body_metadata(b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n"),
                Ok((0, true))
            ));
            assert!(matches!(
                request_body_metadata(b"POST / HTTP/1.1\r\nTransfer-Encoding:\r\n"),
                Ok((0, false))
            ));
        }

        // Tests Content-Length parsing at numeric and protocol boundaries.
        #[test]
        fn request_body_metadata_boundary_tests() {
            assert!(matches!(
                request_body_metadata(b"GET / HTTP/1.1\r\n"),
                Ok((0, false))
            ));
            assert!(matches!(
                request_body_metadata(b"POST / HTTP/1.1\r\nContent-Length:  42  \r\n"),
                Ok((42, false))
            ));

            // The largest usize is valid as a parsed number, even though the
            // request reader will reject it as larger than Blank's body limit.
            let largest = format!("POST / HTTP/1.1\r\nContent-Length: {}\r\n", usize::MAX);
            assert!(matches!(
                request_body_metadata(largest.as_bytes()),
                Ok((length, false)) if length == usize::MAX
            ));

            // A value larger than usize cannot be represented.
            let overflow = format!("POST / HTTP/1.1\r\nContent-Length: {}0\r\n", usize::MAX);
            assert!(matches!(
                request_body_metadata(overflow.as_bytes()),
                Err(RequestReadError::BadRequest)
            ));

            // Transfer-Encoding is recognized regardless of header casing.
            assert!(matches!(
                request_body_metadata(b"POST / HTTP/1.1\r\ntransfer-encoding: chunked\r\n"),
                Ok((0, true))
            ));
            assert!(matches!(
                request_body_metadata(
                    b"POST / HTTP/1.1\r\nContent-Length: 1\r\nTransfer-Encoding: chunked\r\n"
                ),
                Ok((1, true))
            ));
        }

        // Tests the exact accepted and rejected request-body size boundaries.
        #[test]
        fn request_body_size_boundary_tests() {
            let body = vec![b'x'; MAX_HTTP_BODY_SIZE];
            let mut request = format!(
                "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
                MAX_HTTP_BODY_SIZE
            )
            .into_bytes();
            request.extend_from_slice(&body);

            let parsed = parse_request(&request).unwrap();
            assert_eq!(parsed.body().len(), MAX_HTTP_BODY_SIZE);
            assert_eq!(parsed.body().first(), Some(&b'x'));
            assert_eq!(parsed.body().last(), Some(&b'x'));

            let oversized = format!(
                "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
                MAX_HTTP_BODY_SIZE + 1
            );
            assert_eq!(parse_request(oversized.as_bytes()), Err(HTTP_PAYLOAD_TOO_LARGE));
        }

        // Tests query decoding for empty values, repeated names, and escaped bytes.
        #[test]
        fn blank_request_query_edge_tests() {
            let request = parse_request(
                b"GET /search?empty=&flag&space=hello+world&escaped=%26%3D%2B&tag=one&tag=two HTTP/1.1\r\n\r\n",
            )
            .unwrap();

            assert_eq!(request.query_value("empty"), Some(""));
            assert_eq!(request.query_value("flag"), Some(""));
            assert_eq!(request.query_value("space"), Some("hello world"));
            assert_eq!(request.query_value("escaped"), Some("&=+"));
            assert_eq!(request.query_values("tag").unwrap(), &["one", "two"]);
            assert_eq!(request.query_value("missing"), None);
            assert_eq!(request.query_values("missing"), None);
        }

        // Tests URL-encoded form parsing with content-type parameters and unknown types.
        #[test]
        fn blank_request_form_content_type_tests() {
            let body = b"name=Blank+Web&tag=one&tag=two";
            let request_text = format!(
                "POST / HTTP/1.1\r\nContent-Type: Application/X-Www-Form-Urlencoded; charset=UTF-8\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            let mut request = request_text.into_bytes();
            request.extend_from_slice(body);

            let parsed = parse_request(&request).unwrap();
            assert_eq!(parsed.form("name"), Some("Blank Web"));
            assert_eq!(parsed.form_values("tag").unwrap(), &["one", "two"]);

            // Unknown content types keep the raw body but do not parse form fields.
            let unknown = b"POST / HTTP/1.1\r\nContent-Type: application/custom\r\nContent-Length: 8\r\n\r\nname=One";
            let parsed = parse_request(unknown).unwrap();
            assert_eq!(parsed.body(), b"name=One");
            assert!(parsed.forms().is_empty());
        }

        // Tests every registered method against every supported HTTP version.
        #[test]
        fn server_method_version_matrix_test() {
            for version in SUPPORTED_HTTP_VERSIONS {
                for method in REAL_HTTP_METHODS {
                    let raw = format!("{} / {}\r\n\r\n", method, version);
                    let parsed = parse_request(raw.as_bytes());

                    if SUPPORTED_HTTP_METHODS.contains(method) {
                        assert!(parsed.is_ok(), "{} {} should be supported", method, version);
                    } else {
                        assert_eq!(
                            parsed,
                            Err(HTTP_NOT_IMPLEMENTED),
                            "{} {} should return 501",
                            method,
                            version
                        );
                    }
                }

                let unknown = format!("BREW / {}\r\n\r\n", version);
                assert_eq!(parse_request(unknown.as_bytes()), Err(HTTP_METHOD_NOT_ALLOWED));
            }

            // An unsupported version always takes precedence over method errors.
            for method in REAL_HTTP_METHODS {
                let raw = format!("{} / HTTP/9.9\r\n\r\n", method);
                assert_eq!(parse_request(raw.as_bytes()), Err(HTTP_VERSION_NOT_SUPPORTED));
            }
        }

        // Tests reading complete requests, empty connections, and incomplete bodies.
        #[test]
        fn request_reader_tests() {
            // An empty client connection returns None.
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let writer = TcpStream::connect(address).unwrap();
            let (mut reader, _) = listener.accept().unwrap();
            writer.shutdown(Shutdown::Write).unwrap();
            assert!(matches!(read_http_request(&mut reader), Ok(None)));

            // A complete request is returned with its body intact.
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let mut writer = TcpStream::connect(address).unwrap();
            let (mut reader, _) = listener.accept().unwrap();
            writer
                .write_all(b"POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc")
                .unwrap();
            writer.shutdown(Shutdown::Write).unwrap();
            let request = match read_http_request(&mut reader) {
                Ok(Some(request)) => request,
                _ => panic!("Complete HTTP request was not read"),
            };
            assert!(request.ends_with(b"abc"));

            // An incomplete body is a bad request, not a successful partial request.
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let mut writer = TcpStream::connect(address).unwrap();
            let (mut reader, _) = listener.accept().unwrap();
            writer
                .write_all(b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\nabc")
                .unwrap();
            writer.shutdown(Shutdown::Write).unwrap();
            assert!(matches!(
                read_http_request(&mut reader),
                Err(RequestReadError::BadRequest)
            ));

            // A header block exactly at the configured limit is accepted.
            let prefix = "GET / HTTP/1.1\r\nX: ";
            let header = format!(
                "{}{}",
                prefix,
                "a".repeat(MAX_HTTP_HEADER_SIZE - prefix.len())
            );
            assert_eq!(header.len(), MAX_HTTP_HEADER_SIZE);
            let raw = format!("{}\r\n\r\n", header);
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let mut writer = TcpStream::connect(address).unwrap();
            let (mut reader, _) = listener.accept().unwrap();
            writer.write_all(raw.as_bytes()).unwrap();
            writer.shutdown(Shutdown::Write).unwrap();
            assert!(matches!(read_http_request(&mut reader), Ok(Some(_))));

            // A header block that grows beyond the limit is rejected.
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let mut writer = TcpStream::connect(address).unwrap();
            let (mut reader, _) = listener.accept().unwrap();
            let oversized = format!(
                "GET / HTTP/1.1\r\nX-Large: {}\r\n",
                "a".repeat(MAX_HTTP_HEADER_SIZE + 1)
            );
            writer.write_all(oversized.as_bytes()).unwrap();
            writer.shutdown(Shutdown::Write).unwrap();
            assert!(matches!(
                read_http_request(&mut reader),
                Err(RequestReadError::HeaderTooLarge)
            ));

            // Unsupported transfer encodings are rejected before body parsing.
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let mut writer = TcpStream::connect(address).unwrap();
            let (mut reader, _) = listener.accept().unwrap();
            writer
                .write_all(b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n")
                .unwrap();
            assert!(matches!(
                read_http_request(&mut reader),
                Err(RequestReadError::NotImplemented)
            ));

            // Oversized Content-Length values are rejected without reading the body.
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let mut writer = TcpStream::connect(address).unwrap();
            let (mut reader, _) = listener.accept().unwrap();
            let oversized = format!(
                "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
                MAX_HTTP_BODY_SIZE + 1
            );
            writer.write_all(oversized.as_bytes()).unwrap();
            assert!(matches!(
                read_http_request(&mut reader),
                Err(RequestReadError::BodyTooLarge)
            ));
        }

        // Tests URL decoding, hexadecimal parsing, and URL-encoded field handling.
        #[test]
        fn urlencoded_parser_tests() {
            assert_eq!(hex_value(b'0'), Some(0));
            assert_eq!(hex_value(b'9'), Some(9));
            assert_eq!(hex_value(b'a'), Some(10));
            assert_eq!(hex_value(b'F'), Some(15));
            assert_eq!(hex_value(b'g'), None);

            assert_eq!(url_decode(b"Hello+World"), Some("Hello World".to_string()));
            assert_eq!(url_decode(b"%2F%41%7e"), Some("/A~".to_string()));
            assert_eq!(url_decode(b"plain"), Some("plain".to_string()));
            assert_eq!(url_decode(b"%"), None);
            assert_eq!(url_decode(b"%G0"), None);
            assert_eq!(url_decode(b"%FF"), None);

            let forms = parse_urlencoded(b"name=One&name=Two&space=Hello+World&slash=%2F&empty=")
                .unwrap();
            assert_eq!(forms.get("name").unwrap(), &["One", "Two"]);
            assert_eq!(forms.get("space").unwrap(), &["Hello World"]);
            assert_eq!(forms.get("slash").unwrap(), &["/"]);
            assert_eq!(forms.get("empty").unwrap(), &[""]);
            assert!(parse_urlencoded(b"name=%GG").is_err());
            assert!(parse_urlencoded(b"name=%FF").is_err());
            assert!(parse_urlencoded(b"").unwrap().is_empty());
            assert!(parse_urlencoded(b"&&").unwrap().is_empty());
        }

        // Tests form field limits and repeated field insertion.
        #[test]
        fn form_field_limit_tests() {
            let mut forms = BlankForms::new();

            assert!(insert_form_value(&mut forms, "name".to_string(), "Blank".to_string()).is_ok());
            assert!(insert_form_value(&mut forms, "name".to_string(), "Web".to_string()).is_ok());
            assert_eq!(forms.get("name").unwrap(), &["Blank", "Web"]);

            assert!(insert_form_value(&mut forms, String::new(), "value".to_string()).is_err());
            assert!(insert_form_value(
                &mut forms,
                "maximum".to_string(),
                "x".repeat(MAX_FORM_FIELD_SIZE)
            )
            .is_ok());
            assert!(insert_form_value(
                &mut forms,
                "large".to_string(),
                "x".repeat(MAX_FORM_FIELD_SIZE + 1)
            )
            .is_err());

            let mut full = BlankForms::new();
            for index in 0..MAX_FORM_FIELDS {
                assert!(insert_form_value(
                    &mut full,
                    format!("field{}", index),
                    "value".to_string()
                )
                .is_ok());
            }
            assert_eq!(full.len(), MAX_FORM_FIELDS);
            assert!(insert_form_value(&mut full, "overflow".to_string(), "value".to_string()).is_err());
        }

        // Tests multipart fields, repeated names, file skipping, and invalid boundaries.
        #[test]
        fn multipart_parser_tests() {
            let boundary = "test-boundary";
            let body = format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\nAlice\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"tag\"\r\n\r\none\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"tag\"\r\n\r\ntwo\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"file.txt\"\r\nContent-Type: text/plain\r\n\r\nnot parsed as a form field\r\n--{boundary}--\r\n"
            );
            let content_type = format!("multipart/form-data; boundary={}", boundary);
            let forms = parse_multipart_form(body.as_bytes(), &content_type).unwrap();

            assert_eq!(forms.get("name").unwrap(), &["Alice"]);
            assert_eq!(forms.get("tag").unwrap(), &["one", "two"]);
            assert!(!forms.contains_key("file"));

            let quoted = format!("multipart/form-data; boundary=\"{}\"", boundary);
            assert!(parse_multipart_form(body.as_bytes(), &quoted).is_ok());
            assert!(parse_multipart_form(body.as_bytes(), "multipart/form-data").is_err());
            assert!(parse_multipart_form(b"not multipart", &content_type).is_err());
            assert!(parse_multipart_form(
                b"--test-boundary\r\nMalformed-Part\r\n--test-boundary--\r\n",
                &content_type
            )
            .is_err());

            assert_eq!(
                super::disposition_parameter(
                    "form-data; name=\"profile\"; filename=\"avatar.png\"",
                    "name"
                ),
                Some("profile".to_string())
            );
            assert!(super::disposition_parameter_exists(
                "form-data; name=\"profile\"; filename=\"avatar.png\"",
                "filename"
            ));
            assert!(!super::disposition_parameter_exists("form-data; name=x", "filename"));
        }

        // Tests route normalization and byte-sequence lookup helpers.
        #[test]
        fn internal_helper_tests() {
            assert_eq!(normalize_route("/health"), "/health");
            assert_eq!(normalize_route("health"), "/health");
            assert_eq!(normalize_route(""), "/");

            assert_eq!(find_bytes(b"abcdef", b"cd"), Some(2));
            assert_eq!(find_bytes(b"abcdef", b"gh"), None);
            assert_eq!(find_bytes(b"ab", b"abc"), None);
            assert_eq!(find_bytes(b"abc", b""), None);

            // Unspecified IPv4 and IPv6 addresses become loopback addresses.
            let ipv4_any: SocketAddr = "0.0.0.0:7600".parse().unwrap();
            let ipv4_loopback: SocketAddr = "127.0.0.1:7600".parse().unwrap();
            assert_eq!(loopback_address(ipv4_any), ipv4_loopback);

            let ipv6_any: SocketAddr = "[::]:7600".parse().unwrap();
            let ipv6_loopback: SocketAddr = "[::1]:7600".parse().unwrap();
            assert_eq!(loopback_address(ipv6_any), ipv6_loopback);

            // Already-specific addresses remain unchanged.
            let specific: SocketAddr = "192.0.2.10:7600".parse().unwrap();
            assert_eq!(loopback_address(specific), specific);

            // Active workers remain in the list until they finish.
            let (release, wait) = std::sync::mpsc::channel();
            let worker = thread::spawn(move || {
                let _ = wait.recv();
            });
            let mut workers = vec![worker];
            reap_workers(&mut workers);
            assert_eq!(workers.len(), 1);

            // Completed workers are joined and removed from the list.
            release.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(1);
            while !workers[0].is_finished() && Instant::now() < deadline {
                thread::yield_now();
            }
            assert!(workers[0].is_finished());
            reap_workers(&mut workers);
            assert!(workers.is_empty());
        }

        // Tests header and cookie validation helpers.
        #[test]
        fn header_and_cookie_validation_tests() {
            assert!(is_valid_header_name("Content-Type"));
            assert!(is_valid_header_name("x-custom_header~value"));
            assert!(!is_valid_header_name(""));
            assert!(!is_valid_header_name("Bad Header"));
            assert!(!is_valid_header_name("Bad:Header"));

            assert!(is_valid_header_value("plain value"));
            assert!(is_valid_header_value(""));
            assert!(!is_valid_header_value("first\r\nsecond"));
            assert!(!is_valid_header_value("first\nsecond"));

            assert!(is_valid_cookie_name("session_id"));
            assert!(!is_valid_cookie_name("bad name"));
            assert!(is_valid_cookie_value("abc123"));
            assert!(is_valid_cookie_value(""));
            assert!(!is_valid_cookie_value("has space"));
            assert!(!is_valid_cookie_value("semi;colon"));
            assert!(!is_valid_cookie_value("comma,value"));
            assert!(!is_valid_cookie_value("line\nfeed"));
        }

        // Tests the status and body returned for every request-read error.
        #[test]
        fn request_read_error_status_tests() {
            assert_eq!(
                RequestReadError::BadRequest.response(),
                (HTTP_BAD_REQUEST, "Bad Request")
            );
            assert_eq!(
                RequestReadError::HeaderTooLarge.response(),
                (HTTP_HEADER_FIELDS_TOO_LARGE, "Request headers too large")
            );
            assert_eq!(
                RequestReadError::BodyTooLarge.response(),
                (HTTP_PAYLOAD_TOO_LARGE, "Content Too Large")
            );
            assert_eq!(
                RequestReadError::NotImplemented.response(),
                (HTTP_NOT_IMPLEMENTED, "Transfer encoding not supported")
            );
            assert_eq!(
                RequestReadError::TimedOut.response(),
                (HTTP_REQUEST_TIMEOUT, "Request Timeout")
            );
            assert_eq!(
                RequestReadError::Io.response(),
                (HTTP_BAD_REQUEST, "Bad Request")
            );
        }

        // Tests whether the overload helper writes an actual 503 response.
        #[test]
        fn service_unavailable_response_test() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();

            let worker = thread::spawn(move || {
                let (client, _) = listener.accept().unwrap();
                write_service_unavailable(client);
            });

            let mut client = TcpStream::connect(address).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            let mut response = String::new();
            client.read_to_string(&mut response).unwrap();
            worker.join().unwrap();

            assert!(response.starts_with("HTTP/1.1 503 Service Unavailable"));
            assert!(response.contains("Service Unavailable"));
            assert!(response.contains("Content-Type: text/plain; charset=utf-8"));
        }

        // Tests that the public error values remain stable.
        #[test]
        #[allow(deprecated)]
        fn server_error_constants_test() {
            let errors: [BlankError; 4] = [
                ERROR_BIND_SERVER,
                ERROR_ACCEPT_CONNECTION,
                ERROR_SPAWN_SERVER_THREAD,
                ERROR_SERVER_THREAD_PANIC,
            ];

            assert_eq!(errors, [1, 3, 4, 5]);
        }
    };
}

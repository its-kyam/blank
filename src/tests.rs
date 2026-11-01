// tests.rs
//! This module helps blank to run tests
//! when writing the framework
//!
//! Only rust tests
//!
//! Current tests
//!     - hello_test
//!     - blank_constructor_test (as a macro)

use crate::constants::{DEFAULT_PORT, HELLO_MESSAGE};
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
        $some_random_port:expr
    ) => {
        #[test]
        fn blank_constructor_test() {
            // test constructor new
            assert_eq!(Blank::new(), Blank { tcp_port: DEFAULT_PORT });

            // test constructor from
            assert_eq!(Blank::from($some_random_port), Blank { tcp_port: $some_random_port });

            // test constructor default
            // should be same as new()
            // new() is alredy tested
            assert_eq!(Blank::default(), Blank::new());
        }
    };
}

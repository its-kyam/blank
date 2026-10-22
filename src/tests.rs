// tests.rs
//! This module helps blank to run tests 
//! when writing the framework
//! 
//! Only rust tests
//! 
//! Current tests
//!     - hello_test

use crate::hello_blank;
use crate::constants::HELLO_MESSAGE;

#[cfg(test)]
/// Tests whether the function hello_blank() is working
fn hello_test() {
    assert_eq!(hello_blank(), HELLO_MESSAGE);
}

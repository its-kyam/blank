// lib.rs
//! This is blank,
//! The ultimate web framework

// mod all files
mod server;
mod constants;

// use compile-only on tests.rs
// use macro_use so that the tests macro can be there
#[cfg(test)]
#[macro_use]
mod tests;

use constants::HELLO_MESSAGE;

pub fn hello_blank() -> &'static str {
    HELLO_MESSAGE
}

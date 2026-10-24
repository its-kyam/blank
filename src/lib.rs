// lib.rs
//! This is blank,
//! The ultimate web framework

// use compile-only on tests.rs
mod constants;
#[cfg(test)]
mod tests;

// mod all files
mod server;

use constants::HELLO_MESSAGE;

pub fn hello_blank() -> &'static str {
    HELLO_MESSAGE
}

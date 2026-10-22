// lib.rs
//! This is blank,
//! The ultimate web framework

mod tests;
mod constants;

use constants::HELLO_MESSAGE;

pub fn hello_blank() -> &str {
    HELLO_MESSAGE
}

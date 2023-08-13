//! SMTP client and server library.

#![warn(clippy::pedantic)]

pub mod command;
pub mod ehlo;
pub mod message;
pub mod server;

pub use server::Server;

mod io;

/// The maximum number of bytes in a line including the CRLF.
pub const LINE_LIMIT: usize = 1000;

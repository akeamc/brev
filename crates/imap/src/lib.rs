#![warn(clippy::pedantic)]

use std::{fmt, num::NonZeroU32};

pub mod authenticate;
pub mod command;
pub mod protocol;
pub mod response;
pub mod sequence;
pub mod server;

use response::StatusResponse;
pub use server::Server;

/// A unique identifier for a message.
///
/// See [RFC 9051](https://datatracker.ietf.org/doc/html/rfc9051#section-2.3.1.1).
#[derive(Debug, Clone, Copy)]
pub struct Uid(pub NonZeroU32);

impl fmt::Display for Uid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tag(pub String);

impl<T: Into<String>> From<T> for Tag {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub type Result<T, E = StatusResponse> = std::result::Result<T, E>;

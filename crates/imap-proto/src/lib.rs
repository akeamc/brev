use std::{
    fmt::{self, Write},
    num::NonZeroU32,
};

use response::StatusResponse;

pub mod exists;
pub mod flags;
pub mod recent;

pub mod command;
pub mod response;
pub mod sequence;

/// Format a parenthesized list ([Section 4.4] of RFC9051).
/// The data items are delimeted by spaces and the list is bounded
/// at each end by parentheses.
///
/// ```
/// struct Numbers(Vec<u32>);
///
/// impl std::fmt::Display for Numbers {
///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
///         imap_proto::fmt_paren_list(f, &self.0)
///     }
/// }
///
/// let numbers = Numbers(vec![1, 2, 3]);
/// assert_eq!(numbers.to_string(), "(1 2 3)");
/// ```
///
/// [Section 4.4]: https://www.rfc-editor.org/rfc/rfc9051.html#name-parenthesized-list
pub fn fmt_paren_list<T: fmt::Display>(
    f: &mut fmt::Formatter<'_>,
    iter: impl IntoIterator<Item = T>,
) -> fmt::Result {
    let mut iter = iter.into_iter().peekable();
    f.write_char('(')?;
    while let Some(t) = iter.next() {
        t.fmt(f)?;
        if iter.peek().is_some() {
            f.write_char(' ')?;
        }
    }
    f.write_char(')')
}

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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
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

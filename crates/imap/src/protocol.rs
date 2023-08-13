use std::fmt::{self, Write};

pub mod capability;
pub mod exists;
pub mod fetch;
pub mod flags;
pub mod list;
pub mod recent;
pub mod select;
pub mod status;

/// Format a parenthesized list ([Section 4.4] of RFC9051).
/// The data items are delimeted by spaces and the list is bounded
/// at each end by parentheses.
///
/// [Section 4.4]: https://www.rfc-editor.org/rfc/rfc9051.html#name-parenthesized-list
pub fn fmt_paren_list<'a, T: fmt::Display>(
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

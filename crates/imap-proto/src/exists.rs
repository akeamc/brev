use std::fmt;

/// The EXISTS response reports the number of messages in the mailbox.
/// This response occurs as a result of a SELECT or EXAMINE command and
/// if the size of the mailbox changes (e.g., new messages).
///
/// <https://www.rfc-editor.org/rfc/rfc9051.html#section-7.4.1>
pub struct Response(pub u32);

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} EXISTS", self.0)
    }
}

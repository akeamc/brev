use std::fmt;

use crate::Uid;

util::flags! {
    pub Items: u8 {
        /// The number of messages in the mailbox.
        (1 << 0, "MESSAGES", MESSAGES);
        /// The next unique identifier value of the mailbox.
        (1 << 1, "UIDNEXT", UIDNEXT);
        /// The unique identifier validity value of the mailbox.
        (1 << 2, "UIDVALIDITY", UIDVALIDITY);
        /// The number of messages that do not have the \Seen flag set.
        (1 << 3, "UNSEEN", UNSEEN);
        /// The number of messages that have the \Deleted flag set.
        (1 << 4, "DELETED", DELETED);
        /// The total size of the mailbox in octets.
        (1 << 5, "SIZE", SIZE);
    }
}

#[derive(Debug, Default)]
pub struct Response {
    pub mailbox: String,
    /// See [`Items::MESSAGES`].
    pub messages: Option<u32>,
    /// See [`Items::UIDNEXT`].
    pub uid_next: Option<Uid>,
    /// See [`Items::UIDVALIDITY`].
    pub uid_validity: Option<Uid>,
    /// See [`Items::UNSEEN`].
    pub unseen: Option<u32>,
    /// See [`Items::DELETED`].
    pub deleted: Option<u32>,
    /// See [`Items::SIZE`].
    pub size: Option<u32>,
}

macro_rules! fmt {
    ($f:ident, {
        $(($name:literal, $value:ident);)+
    }) => {
        let mut first = true;
        $(
            if let Some(value) = $value {
                if !std::mem::take(&mut first) {
                    write!($f, " ")?;
                }
                write!($f, "{value} {}", $name)?;
            }
        )+
    };
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            mailbox,
            messages,
            uid_next,
            uid_validity,
            unseen,
            deleted,
            size,
        } = self;

        write!(f, "STATUS {} (", mailbox)?;
        fmt!(f, {
            ("MESSAGES", messages);
            ("UIDNEXT", uid_next);
            ("UIDVALIDITY", uid_validity);
            ("UNSEEN", unseen);
            ("DELETED", deleted);
            ("SIZE", size);
        });
        write!(f, ")")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fmt() {
        assert_eq!(
            super::Response {
                mailbox: "INBOX".to_string(),
                unseen: Some(3),
                deleted: Some(1),
                ..Default::default()
            }
            .to_string(),
            "STATUS INBOX (3 UNSEEN 1 DELETED)"
        )
    }
}

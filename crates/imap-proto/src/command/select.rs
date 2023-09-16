use std::fmt;

use crate::{exists, flags, Tag, Uid};

use super::list::ListItem;

pub struct Response {
    pub flags: flags::Response,
    pub exists: exists::Response,
    pub uid_validity: u32,
    pub next_uid: Uid,
    pub mailbox: ListItem,
    pub tag: Tag,
    pub read_only: bool,
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "* {}\r\n", self.flags)?;
        write!(f, "* {}\r\n", self.exists)?;
        write!(f, "* OK [UIDVALIDITY {}] UIDs valid\r\n", self.uid_validity)?;
        write!(f, "* OK [UIDNEXT {}] Predicted next UID\r\n", self.next_uid)?;
        self.mailbox.fmt(f)?;
        write!(
            f,
            "{} OK [{}] Done\r\n",
            self.tag,
            if self.read_only {
                "READ-ONLY"
            } else {
                "READ-WRITE"
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{command::list::Attributes, flags::Flag};

    use super::*;

    #[test]
    fn fmt() {
        assert_eq!(
            super::Response {
                flags: flags::Response(vec![Flag::Seen, Flag::Answered, Flag::Flagged]),
                exists: exists::Response(37),
                uid_validity: 3857529045,
                next_uid: Uid(4392.try_into().unwrap()),
                mailbox: ListItem::new("Drafts", Attributes::DRAFTS),
                tag: "A0016".into(),
                read_only: false,
            }
            .to_string()
            .lines()
            .collect::<Vec<_>>(),
            [
                "* FLAGS (\\Seen \\Answered \\Flagged)",
                "* 37 EXISTS",
                "* OK [UIDVALIDITY 3857529045] UIDs valid",
                "* OK [UIDNEXT 4392] Predicted next UID",
                "* LIST (\\Drafts) NIL \"Drafts\"",
                "A0016 OK [READ-WRITE] Done"
            ]
        );
    }
}

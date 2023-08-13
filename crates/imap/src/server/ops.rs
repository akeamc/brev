pub mod select {
    use crate::{
        protocol::{
            self, exists,
            flags::{self, Flag},
            list::ListItem,
        },
        Tag, Uid,
    };

    pub struct Response {
        pub flags: Vec<Flag>,
        pub exists: u32,
        pub uid_validity: u32,
        pub next_uid: Uid,
        pub mailbox: ListItem,
        pub read_only: bool,
    }

    impl Response {
        pub(crate) fn with_tag(self, tag: impl Into<Tag>) -> protocol::select::Response {
            let Self {
                flags,
                exists,
                uid_validity,
                next_uid,
                mailbox,
                read_only,
            } = self;

            protocol::select::Response {
                flags: flags::Response(flags),
                exists: exists::Response(exists),
                uid_validity,
                next_uid,
                mailbox,
                tag: tag.into(),
                read_only,
            }
        }
    }
}

pub struct Operation {}

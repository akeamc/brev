use imap_proto::{
    command::{self, CommandName},
    Tag,
};

use super::{
    queue::{Channel, Queue},
    session::SelectedState,
};

pub mod select {
    use imap_proto::{
        command, exists,
        flags::{self, Flag},
        Tag, Uid,
    };

    use super::IntoTaggedResponse;

    #[derive(Debug)]
    pub struct Request {
        pub mailbox: String,
        pub read_only: bool,
    }

    impl From<command::Select> for Request {
        fn from(command::Select { mailbox }: command::Select) -> Self {
            Self {
                mailbox,
                read_only: false,
            }
        }
    }

    impl From<command::Examine> for Request {
        fn from(command::Examine { mailbox }: command::Examine) -> Self {
            Self {
                mailbox,
                read_only: true,
            }
        }
    }

    #[derive(Debug)]
    pub struct Response {
        pub flags: Vec<Flag>,
        pub exists: u32,
        pub uid_validity: u32,
        pub next_uid: Uid,
        pub mailbox: command::list::ListItem,
        pub read_only: bool,
    }

    impl IntoTaggedResponse for Response {
        fn into_tagged_response(self, tag: Tag) -> String {
            let Self {
                flags,
                exists,
                uid_validity,
                next_uid,
                mailbox,
                read_only,
            } = self;

            command::select::Response {
                flags: flags::Response(flags),
                exists: exists::Response(exists),
                uid_validity,
                next_uid,
                mailbox,
                tag,
                read_only,
            }
            .to_string()
        }
    }
}

pub mod list {
    use imap_proto::{command, response::StatusResponse, Tag};

    use super::IntoTaggedResponse;

    #[derive(Debug)]
    pub struct Request(pub command::List);

    impl From<command::List> for Request {
        fn from(command: command::List) -> Self {
            Self(command)
        }
    }

    #[derive(Debug)]
    pub struct Response {
        pub list_items: Vec<command::list::ListItem>,
    }

    impl IntoTaggedResponse for Response {
        fn into_tagged_response(self, tag: Tag) -> String {
            let Self { list_items } = self;
            let res = command::list::Response { list_items };
            let status = StatusResponse::ok("LIST completed").with_tag(tag);
            format!("{res}{status}")
        }
    }
}

pub mod fetch {
    use imap_proto::Tag;

    use crate::server::session::SelectedState;

    use super::IntoTaggedResponse;

    #[derive(Debug)]
    pub struct Request {
        pub command: imap_proto::command::Fetch,
        pub selected: SelectedState,
    }

    #[derive(Debug)]
    pub struct Response {}

    impl IntoTaggedResponse for Response {
        fn into_tagged_response(self, tag: Tag) -> String {
            todo!()
        }
    }
}

pub mod create {
    use imap_proto::{command, Tag};

    #[derive(Debug)]
    pub struct Request {
        pub mailbox: String,
    }

    impl From<command::Create> for Request {
        fn from(command::Create { mailbox }: command::Create) -> Self {
            Self { mailbox }
        }
    }

    #[derive(Debug)]
    pub struct Response {}

    impl super::IntoTaggedResponse for Response {
        fn into_tagged_response(self, tag: Tag) -> String {
            todo!()
        }
    }
}

pub(crate) trait IntoTaggedResponse {
    fn into_tagged_response(self, tag: Tag) -> String;
}

pub trait IntoOperation {
    type Context;

    fn into_operation(self, queue: &mut Queue, tag: Tag, context: Self::Context) -> Operation;
}

macro_rules! into_operation {
    ($variant:ident($value:ty)) => {
        impl IntoOperation for $value {
            type Context = ();

            fn into_operation(self, queue: &mut Queue, tag: Tag, _context: ()) -> Operation {
                Operation::$variant(self.into(), queue.insert(tag, CommandName::$variant))
            }
        }
    };
}

into_operation!(List(command::List));
into_operation!(Select(command::Select));
into_operation!(Select(command::Examine)); // EXAMINE is the same as SELECT, but read-only
into_operation!(Create(command::Create));

impl IntoOperation for command::Fetch {
    type Context = SelectedState;

    fn into_operation(self, queue: &mut Queue, tag: Tag, context: Self::Context) -> Operation {
        Operation::Fetch(
            fetch::Request {
                selected: context,
                command: self,
            },
            queue.insert(tag, CommandName::Fetch),
        )
    }
}

macro_rules! operations {
    ($($variant:ident,)*) => {
        paste::paste! {
            pub enum Operation {
                $(
                    $variant([<$variant:lower>]::Request, Channel<[<$variant:lower>]::Response>),
                )*
            }

            pub enum Response {
                $(
                    $variant([<$variant:lower>]::Response),
                )*
            }

            $(
                impl From<[<$variant:lower>]::Response> for Response {
                    fn from(value: [<$variant:lower>]::Response) -> Self {
                        Self::$variant(value)
                    }
                }
            )*
        }
    }
}

operations! {
    Select,
    List,
    Fetch,
    Create,
}

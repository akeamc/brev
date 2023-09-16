use std::{
    borrow::Cow,
    str::{FromStr, Utf8Error},
};

use auth::sasl::MechanismKind;
use nom::{
    bytes::complete::{tag, take_while},
    character::complete::{space0, space1},
    combinator::{map, map_res, opt},
    sequence::delimited,
    IResult,
};
use secrecy::SecretString;
use tracing::debug;

use crate::{
    response::{self, StatusResponse, TaggedStatusResponse},
    sequence, Tag,
};

use self::capability::Capabilities;

pub mod capability;
pub mod fetch;
pub mod list;
pub mod select;
pub mod status;

pub struct Request<T> {
    pub tag: Tag,
    pub data: T,
}

impl<T> Request<T> {
    pub fn new(tag: impl Into<Tag>, data: T) -> Self {
        Self {
            tag: tag.into(),
            data,
        }
    }

    pub fn into_parts(self) -> (T, Request<()>) {
        let Self { tag, data } = self;

        (data, Request { tag, data: () })
    }

    pub fn into_res(
        self,
        status: response::Status,
        message: impl Into<Cow<'static, str>>,
    ) -> TaggedStatusResponse {
        StatusResponse::new(status, message).with_tag(self.tag)
    }

    pub fn ok(self, message: impl Into<Cow<'static, str>>) -> TaggedStatusResponse {
        self.into_res(response::Status::Ok, message)
    }

    pub fn no(self, message: impl Into<Cow<'static, str>>) -> TaggedStatusResponse {
        self.into_res(response::Status::No, message)
    }

    pub fn bad(self, message: impl Into<Cow<'static, str>>) -> TaggedStatusResponse {
        self.into_res(response::Status::Bad, message)
    }
}

impl Request<()> {
    pub fn empty(tag: impl Into<Tag>) -> Self {
        Self {
            tag: tag.into(),
            data: (),
        }
    }
}

impl From<Tag> for Request<()> {
    fn from(tag: Tag) -> Self {
        Self::empty(tag)
    }
}

#[derive(Debug)]
pub struct TaggedCommand {
    pub tag: Tag,
    pub command: Command,
}

macro_rules! args {
    ($name:ident {
        $(
            $(#[$outer:meta])*
            $arg:ident: $T:ty,
        )+
    } $syntax:literal) => {
        #[derive(Debug)]
        pub struct $name {
            $(
                $(#[$outer])*
                pub $arg: $T,
            )+
        }

        impl ParseArgs for $name {
            const SYNTAX: &'static str = $syntax;

            fn parse(i: &str, _is_uid: bool) -> IResult<&str, Self> {
                $(
                    let (i, $arg) = <$T as ParseArg>::parse_arg(i)?;
                )+

                Ok((i, Self {
                    $(
                        $arg,
                    )+
                }))
            }
        }
    };
}

args!(Authenticate {
    mechanism: MechanismKind,
    initial_response: Option<String>,
} "<mechanism> [<initial-response>]");

args!(Login {
    username: String,
    password: SecretString,
} "<username> <password>");

impl From<Login> for auth::Credentials {
    fn from(value: Login) -> Self {
        Self {
            username: value.username,
            password: value.password,
        }
    }
}

args!(Enable {
    capabilities: Capabilities,
} "<capability> [<capability> ...]");

args!(Select {
    mailbox: String,
} "<mailbox>");

args!(Examine {
    mailbox: String,
} "<mailbox>");

args!(Create {
    mailbox: String,
} "<mailbox>");

args!(Delete {
    mailbox: String,
} "<mailbox>");

args!(Rename {
    existing: String,
    new: String,
} "<existing> <new>");

args!(Subscribe {
    mailbox: String,
} "<mailbox>");

args!(Unsubscribe {
    mailbox: String,
} "<mailbox>");

args!(List {
    reference: String,
    mailbox: String,
} "<reference> <mailbox>");

// #[derive(Debug)]
// pub struct List {
//     options: Option<String>,
//     reference: String,
//     mailbox: String,
// }

// impl ParseArgs for List {
//     const SYNTAX: &'static str = "[<options>] <reference> <mailbox>";

//     fn parse(i: &str, is_uid: bool) -> IResult<&str, Self>
//     where
//         Self: Sized,
//     {
//         todo!()
//     }
// }

args!(Status {
    mailbox: String,
    items: status::Items,
} "<mailbox> <status-data-item> [<status-data-item> ...]");

args!(Append {
    mailbox: String,
    // flags: Option<Vec<Flag>>,
    flags: String,
    date_time: Option<String>,
    // message: Option<String>,
} "<mailbox> [<flags>] [<date-time>] [<literal>]");

#[derive(Debug)]
pub struct Expunge {
    is_uid: bool,
}

#[derive(Debug)]
pub struct Fetch {
    is_uid: bool,
    sequence_set: sequence::Set,
    items: fetch::Items,
}

impl ParseArgs for Fetch {
    const SYNTAX: &'static str = "<sequence set> <fetch attribute> [<fetch attribute> ...]";

    fn parse(i: &str, is_uid: bool) -> IResult<&str, Self>
    where
        Self: Sized,
    {
        let (i, sequence_set) = sequence::Set::parse(i)?;
        let (i, _) = space1(i)?;
        let (i, items) = fetch::Items::parse(i)?;
        Ok((
            i,
            Self {
                is_uid,
                sequence_set,
                items,
            },
        ))
    }
}

trait ParseArgs {
    const SYNTAX: &'static str;

    fn parse(i: &str, is_uid: bool) -> IResult<&str, Self>
    where
        Self: Sized;
}

macro_rules! parse_args {
    ($args:ident, $i:expr, $is_uid:expr) => {{
        let (_, out) = $args::parse($i, $is_uid).or_syntax_err(const_format::concatcp!(
            "Syntax: ",
            paste::paste! {
                stringify!([<$args:upper>])
            },
            " ",
            <$args>::SYNTAX,
        ))?;
        Command::$args(out)
    }};
    ($args:ident, $i:expr) => {
        parse_args!($args, $i, false)
    };
}

#[derive(Debug)]
pub enum Command {
    // Any state
    Capability,
    Noop,
    Logout,
    // Non-authenticated state
    Starttls,
    Authenticate(Authenticate),
    Login(Login),
    // Authenticated state
    Enable(Enable),
    Select(Select),
    Examine(Examine),
    Create(Create),
    Delete(Delete),
    Rename(Rename),
    Subscribe(Subscribe),
    Unsubscribe(Unsubscribe),
    List(List),
    Namespace,
    Status(Status),
    Append,
    Idle,
    // Selected state
    Close,
    Unselect,
    Expunge(Expunge),
    Search { is_uid: bool },
    Fetch(Fetch),
    Store { is_uid: bool },
    Copy { is_uid: bool },
    Move { is_uid: bool },
}

#[derive(Debug)]
pub enum CommandName {
    Capability,
    Noop,
    Logout,
    Starttls,
    Authenticate,
    Login,
    Enable,
    Select,
    Examine,
    Create,
    Delete,
    Rename,
    Subscribe,
    Unsubscribe,
    List,
    Namespace,
    Status,
    Append,
    Idle,
    Close,
    Unselect,
    Expunge,
    Search,
    Fetch,
    Store,
    Copy,
    Move,
}

impl Command {
    pub fn name(&self) -> CommandName {
        match self {
            Command::Capability => CommandName::Capability,
            Command::Noop => CommandName::Noop,
            Command::Logout => CommandName::Logout,
            Command::Starttls => CommandName::Starttls,
            Command::Authenticate(_) => CommandName::Authenticate,
            Command::Login(_) => CommandName::Login,
            Command::Enable(_) => CommandName::Enable,
            Command::Select(_) => CommandName::Select,
            Command::Examine(_) => CommandName::Examine,
            Command::Create(_) => CommandName::Create,
            Command::Delete(_) => CommandName::Delete,
            Command::Rename(_) => CommandName::Rename,
            Command::Subscribe(_) => CommandName::Subscribe,
            Command::Unsubscribe(_) => CommandName::Unsubscribe,
            Command::List(_) => CommandName::List,
            Command::Namespace => CommandName::Namespace,
            Command::Status(_) => CommandName::Status,
            Command::Append => CommandName::Append,
            Command::Idle => CommandName::Idle,
            Command::Close => CommandName::Close,
            Command::Unselect => CommandName::Unselect,
            Command::Expunge(_) => CommandName::Expunge,
            Command::Search { is_uid } => CommandName::Search,
            Command::Fetch(_) => CommandName::Fetch,
            Command::Store { is_uid } => CommandName::Store,
            Command::Copy { is_uid } => CommandName::Copy,
            Command::Move { is_uid } => CommandName::Move,
        }
    }
}

fn parse_command(s: &str, is_uid: bool) -> Result<Command, ParseError> {
    let (verb, i) = s.split_once(' ').unwrap_or((s, ""));
    Ok(match (verb.to_ascii_uppercase().as_str(), is_uid) {
        ("CAPABILITY", false) => Command::Capability,
        ("NOOP", false) => Command::Noop,
        ("LOGOUT", false) => Command::Logout,
        ("STARTTLS", false) => Command::Starttls,
        ("AUTHENTICATE", false) => parse_args!(Authenticate, i),
        ("LOGIN", false) => parse_args!(Login, i),
        ("ENABLE", false) => parse_args!(Enable, i),
        ("SELECT", false) => parse_args!(Select, i),
        ("EXAMINE", false) => parse_args!(Examine, i),
        ("CREATE", false) => parse_args!(Create, i),
        ("DELETE", false) => parse_args!(Delete, i),
        ("RENAME", false) => parse_args!(Rename, i),
        ("SUBSCRIBE", false) => parse_args!(Subscribe, i),
        ("UNSUBSCRIBE", false) => parse_args!(Unsubscribe, i),
        ("LIST", false) => parse_args!(List, i),
        ("NAMESPACE", false) => Command::Namespace,
        ("STATUS", false) => parse_args!(Status, i),
        ("APPEND", false) => Command::Append,
        ("IDLE", false) => Command::Idle,
        ("CLOSE", false) => Command::Close,
        ("UNSELECT", false) => Command::Unselect,
        ("EXPUNGE", is_uid) => Command::Expunge(Expunge { is_uid }),
        ("SEARCH", is_uid) => Command::Search { is_uid },
        ("FETCH", is_uid) => parse_args!(Fetch, i, is_uid),
        ("STORE", is_uid) => Command::Store { is_uid },
        ("COPY", is_uid) => Command::Copy { is_uid },
        ("MOVE", is_uid) => Command::Move { is_uid },
        _ => return Err(ParseError::UnrecognizedCommand),
    })
}

impl FromStr for Command {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.get(0..4).map(str::to_ascii_uppercase).as_deref() == Some("UID ") {
            parse_command(&s[4..], true)
        } else {
            parse_command(s, false)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Syntax(&'static str),
    UnrecognizedCommand,
}

impl From<ParseError> for StatusResponse {
    fn from(e: ParseError) -> Self {
        Self::bad(match e {
            ParseError::Syntax(s) => s,
            ParseError::UnrecognizedCommand => "Unrecognized command",
        })
    }
}

#[derive(Debug)]
pub enum Error {
    Bad(TaggedStatusResponse),
    InvalidUtf8,
}

impl From<Utf8Error> for Error {
    fn from(_e: Utf8Error) -> Self {
        Self::InvalidUtf8
    }
}

impl TryFrom<&[u8]> for TaggedCommand {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let s = std::str::from_utf8(value)?;
        debug!(?s, "parsing command");
        let (tag, rest) = s.split_once(' ').unwrap_or((s, ""));
        match rest.parse() {
            Ok(kind) => Ok(Self {
                tag: tag.into(),
                command: kind,
            }),
            Err(e) => Err(Error::Bad(StatusResponse::from(e).with_tag(tag))),
        }
    }
}

fn parse_dquote_str(i: &str) -> IResult<&str, String> {
    let (i, _) = nom::bytes::complete::tag("\"")(i)?;
    let (i, s) = nom::bytes::complete::escaped_transform(
        nom::character::complete::none_of("\\\""),
        '\\',
        nom::branch::alt((
            nom::bytes::complete::tag("\\"),
            nom::bytes::complete::tag("\""),
        )),
    )(i)?;
    let (i, _) = nom::bytes::complete::tag("\"")(i)?;

    Ok((i, s))
}

fn parse_str(i: &str) -> IResult<&str, Cow<'_, str>> {
    nom::branch::alt((
        map(parse_dquote_str, Cow::Owned),
        map(nom::bytes::complete::is_not(" \t\r\n"), Cow::Borrowed),
    ))(i)
}

trait ParseArg {
    fn parse_arg(i: &str) -> IResult<&str, Self>
    where
        Self: Sized;
}

impl<T: ParseArg> ParseArg for Option<T> {
    fn parse_arg(i: &str) -> IResult<&str, Self> {
        let (i, _) = space0(i)?;
        opt(T::parse_arg)(i)
    }
}

impl ParseArg for String {
    fn parse_arg(i: &str) -> IResult<&str, Self> {
        let (i, _) = space0(i)?;
        map(parse_str, Cow::into_owned)(i)
    }
}

impl ParseArg for SecretString {
    fn parse_arg(i: &str) -> IResult<&str, Self> {
        map(String::parse_arg, SecretString::new)(i)
    }
}

impl ParseArg for MechanismKind {
    fn parse_arg(i: &str) -> IResult<&str, Self> {
        let (i, _) = space0(i)?;
        map_res(parse_str, |s| s.parse())(i)
    }
}

impl ParseArg for Capabilities {
    fn parse_arg(i: &str) -> IResult<&str, Self> {
        let (i, _) = space0(i)?;
        Ok(("", i.split(' ').collect()))
    }
}

impl ParseArg for status::Items {
    fn parse_arg(i: &str) -> IResult<&str, Self> {
        let (i, _) = space0(i)?;
        let (i, items) = delimited(tag("("), take_while(|c| c != ')'), tag(")"))(i)?;

        Ok((i, items.split(' ').collect()))
    }
}

impl ParseArg for sequence::Set {
    fn parse_arg(i: &str) -> IResult<&str, Self>
    where
        Self: Sized,
    {
        let (i, _) = space0(i)?;
        sequence::Set::parse(i)
    }
}

impl ParseArg for fetch::Items {
    fn parse_arg(i: &str) -> IResult<&str, Self> {
        let (i, _) = space0(i)?;
        fetch::Items::parse(i)
    }
}

trait ParseResultExt<T, E> {
    fn or_syntax_err(self, msg: &'static str) -> Result<T, ParseError>;
}

impl<T, E> ParseResultExt<T, E> for Result<T, E> {
    fn or_syntax_err(self, msg: &'static str) -> Result<T, ParseError> {
        self.map_err(|_| ParseError::Syntax(msg))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use secrecy::ExposeSecret;

    use super::*;

    #[test]
    fn dquote() {
        let cases = [
            ("\"Hello\"", ("", "Hello")),
            ("\"Hello World!\" rest", (" rest", "Hello World!")),
            ("\"dquote \\\"\"", ("", "dquote \"")),
            ("\"backslash \\\\\"", ("", "backslash \\")),
        ];

        for (input, (rest, str)) in cases {
            assert_eq!(super::parse_dquote_str(input), Ok((rest, str.to_owned())));
        }
    }

    #[test]
    fn str() {
        let cases = [
            ("Hello World!", (" World!", "Hello")),
            ("\"Hello World!\"", ("", "Hello World!")),
        ];

        for (input, (rest, str)) in cases {
            assert_eq!(super::parse_str(input), Ok((rest, str.into())));
        }
    }

    #[test]
    fn login() {
        let Ok(Command::Login(Login { username, password })) = "login alice \"hunter 2\"".parse()
        else {
            panic!()
        };

        assert_eq!(username, "alice");
        assert_eq!(password.expose_secret(), "hunter 2");
    }

    #[test]
    fn impl_gen() {
        let errors = [
            (
                "login bob",
                ParseError::Syntax("Syntax: LOGIN <username> <password>"),
            ),
            ("unrecognizedcommand", ParseError::UnrecognizedCommand),
        ];

        for (input, expected) in errors {
            assert_eq!(Command::from_str(input).unwrap_err(), expected);
        }
    }

    #[test]
    fn status() {
        match "status \"INBOX\" (MESSAGES UNSEEN)".parse() {
            Ok(Command::Status(super::Status { mailbox, items })) => {
                assert_eq!(mailbox, "INBOX");
                assert_eq!(items, status::Items::MESSAGES | status::Items::UNSEEN);
            }
            other => panic!("{other:?}"),
        }

        assert!("status INBOX ()".parse::<Command>().is_ok());
    }
}

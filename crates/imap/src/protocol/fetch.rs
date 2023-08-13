use std::str::FromStr;

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::char,
    combinator::{map, map_res},
    multi::separated_list1,
    sequence::delimited,
    IResult,
};

#[derive(Debug, PartialEq, Eq)]
pub enum Attribute {
    Flags,
    Internaldate,
    Rfc822Size,
    Envelope,
    Body,
}

impl FromStr for Attribute {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "FLAGS" => Self::Flags,
            "INTERNALDATE" => Self::Internaldate,
            "RFC822.SIZE" => Self::Rfc822Size,
            "ENVELOPE" => Self::Envelope,
            "BODY" => Self::Body,
            _ => return Err(()),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Items {
    All,
    Fast,
    Full,
    Attributes(Vec<Attribute>),
}

impl Items {
    #[must_use]
    pub fn attributes(&self) -> &[Attribute] {
        match self {
            Self::All => &[
                Attribute::Flags,
                Attribute::Internaldate,
                Attribute::Rfc822Size,
                Attribute::Envelope,
            ],
            Self::Fast => &[
                Attribute::Flags,
                Attribute::Internaldate,
                Attribute::Rfc822Size,
            ],
            Self::Full => &[
                Attribute::Flags,
                Attribute::Internaldate,
                Attribute::Rfc822Size,
                Attribute::Envelope,
                Attribute::Body,
            ],
            Self::Attributes(attributes) => attributes,
        }
    }
}

fn parse_attribute(i: &str) -> IResult<&str, Attribute> {
    dbg!(i);
    map_res(
        take_while1(|c: char| c != ' ' && c != ')'),
        Attribute::from_str,
    )(i)
}

impl Items {
    pub fn parse(i: &str) -> IResult<&str, Self> {
        alt((
            map(tag("ALL"), |_| Self::All),
            map(tag("FAST"), |_| Self::Fast),
            map(tag("FULL"), |_| Self::Full),
            map(
                delimited(
                    char('('),
                    separated_list1(char(' '), parse_attribute),
                    char(')'),
                ),
                Self::Attributes,
            ),
            map(parse_attribute, |a| Self::Attributes(vec![a])),
        ))(i)
    }
}

#[cfg(test)]
mod tests {
    use super::Items;

    #[test]
    fn parse_arg() {
        assert_eq!(
            Items::parse("ALL").unwrap().1.attributes(),
            Items::parse("(FLAGS INTERNALDATE RFC822.SIZE ENVELOPE)")
                .unwrap()
                .1
                .attributes(),
        )
    }
}

/*
seq-range       = seq-number ":" seq-number
                    ; two seq-number values and all values between
                    ; these two regardless of order.
                    ; Example: 2:4 and 4:2 are equivalent and
                    ; indicate values 2, 3, and 4.
                    ; Example: a unique identifier sequence range of
                    ; 3291:* includes the UID of the last message in
                    ; the mailbox, even if that value is less than
                    ; 3291.

sequence-set    = (seq-number / seq-range) ["," sequence-set]
                    ; set of seq-number values, regardless of order.
                    ; Servers MAY coalesce overlaps and/or execute
                    ; the sequence in any order.
                    ; Example: a message sequence number set of
                    ; 2,4:7,9,12:* for a mailbox with 15 messages is
                    ; equivalent to 2,4,5,6,7,9,12,13,14,15
                    ; Example: a message sequence number set of
                    ; *:4,5:7 for a mailbox with 10 messages is
                    ; equivalent to 10,9,8,7,6,5,4,5,6,7 and MAY
                    ; be reordered and overlap coalesced to be
                    ; 4,5,6,7,8,9,10.
 */

use std::{fmt, num::NonZeroU32, str::FromStr};

use nom::{
    branch::alt,
    character::complete::{char, digit1},
    combinator::{map, map_res},
    multi::separated_list0,
    IResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    Inclusive(NonZeroU32),
    Unbounded,
}

impl fmt::Display for Bound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inclusive(n) => n.fmt(f),
            Self::Unbounded => write!(f, "*"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SequenceRange {
    lower: Bound,
    upper: Bound,
}

impl SequenceRange {
    pub fn new(lower: Bound, upper: Bound) -> Self {
        Self { lower, upper }
    }
}

impl fmt::Display for SequenceRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.lower == self.upper {
            return self.lower.fmt(f);
        }
        write!(f, "{}:{}", self.lower, self.upper)
    }
}

fn parse_nz_u32(i: &str) -> IResult<&str, NonZeroU32> {
    map_res(digit1, NonZeroU32::from_str)(i)
}

impl Bound {
    fn parse(i: &str) -> IResult<&str, Self> {
        alt((
            map(parse_nz_u32, Self::Inclusive),
            map(char('*'), |_| Self::Unbounded),
        ))(i)
    }
}

fn parse_range(i: &str) -> IResult<&str, SequenceRange> {
    let (i, lower_bound) = Bound::parse(i)?;
    let (i, _) = char(':')(i)?;
    let (i, upper_bound) = Bound::parse(i)?;
    Ok((
        i,
        SequenceRange {
            lower: lower_bound,
            upper: upper_bound,
        },
    ))
}

impl SequenceRange {
    fn parse(i: &str) -> IResult<&str, Self> {
        alt((
            map(parse_range, |r| r),
            map(parse_nz_u32, |n| SequenceRange {
                lower: Bound::Inclusive(n),
                upper: Bound::Inclusive(n),
            }),
        ))(i)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SequenceSet {
    ranges: Vec<SequenceRange>,
}

impl SequenceSet {
    pub fn parse(i: &str) -> IResult<&str, Self> {
        let (i, ranges) = separated_list0(char(','), SequenceRange::parse)(i)?;
        Ok((i, Self { ranges }))
    }
}

impl fmt::Display for SequenceSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.ranges.iter().peekable();
        while let Some(range) = iter.next() {
            write!(f, "{}", range)?;
            if iter.peek().is_some() {
                write!(f, ",")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::sequence::{SequenceRange, SequenceSet};

    #[test]
    fn parse() {
        assert_eq!(
            SequenceSet::parse("1:3,5,6:*").unwrap().1.to_string(),
            "1:3,5,6:*"
        )
    }
}

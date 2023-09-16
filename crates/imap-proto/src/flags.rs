use std::{convert::Infallible, fmt};

#[derive(Debug)]
pub enum Flag {
    Seen,
    Answered,
    Flagged,
    Deleted,
    Draft,
    Recent,
    Forwarded,
    MDNSent,
    Junk,
    NotJunk,
    Phishing,
    Keyword(String),
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Flag::Seen => write!(f, "\\Seen"),
            Flag::Answered => write!(f, "\\Answered"),
            Flag::Flagged => write!(f, "\\Flagged"),
            Flag::Deleted => write!(f, "\\Deleted"),
            Flag::Draft => write!(f, "\\Draft"),
            Flag::Recent => write!(f, "\\Recent"),
            Flag::Forwarded => write!(f, "$Forwarded"),
            Flag::MDNSent => write!(f, "$MDNSent"),
            Flag::Junk => write!(f, "$Junk"),
            Flag::NotJunk => write!(f, "$NotJunk"),
            Flag::Phishing => write!(f, "$Phishing"),
            Flag::Keyword(keyword) => write!(f, "{keyword}"),
        }
    }
}

impl std::str::FromStr for Flag {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "\\Seen" => Flag::Seen,
            "\\Answered" => Flag::Answered,
            "\\Flagged" => Flag::Flagged,
            "\\Deleted" => Flag::Deleted,
            "\\Draft" => Flag::Draft,
            "\\Recent" => Flag::Recent,
            "$Forwarded" => Flag::Forwarded,
            "$MDNSent" => Flag::MDNSent,
            "$Junk" => Flag::Junk,
            "$NotJunk" => Flag::NotJunk,
            "$Phishing" => Flag::Phishing,
            _ => Flag::Keyword(s.to_string()),
        })
    }
}

pub struct Response(pub Vec<Flag>);

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FLAGS ")?;
        crate::fmt_paren_list(f, self.0.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::{Flag, Response};

    #[test]
    fn fmt() {
        assert_eq!(
            Response(vec![
                Flag::Seen,
                Flag::Answered,
                Flag::Flagged,
                Flag::Deleted,
                Flag::Draft,
                Flag::Recent,
                Flag::Forwarded,
            ])
            .to_string(),
            "FLAGS (\\Seen \\Answered \\Flagged \\Deleted \\Draft \\Recent $Forwarded)"
        );
    }
}

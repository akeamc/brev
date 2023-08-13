use std::fmt;

use util::flags;

use super::fmt_paren_list;

flags! {
    pub Attributes: u16 {
        (1 << 0, "\\NonExistent", NON_EXISTENT);
        (1 << 1, "\\Noinferiors", NOINFERIORS);
        (1 << 2, "\\Noselect", NOSELECT);
        (1 << 3, "\\HasChildren", HAS_CHILDREN);
        (1 << 4, "\\HasNoChildren", HAS_NO_CHILDREN);
        (1 << 5, "\\Marked", MARKED);
        (1 << 6, "\\Unmarked", UNMARKED);
        (1 << 7, "\\Subscribed", SUBSCRIBED);
        (1 << 8, "\\Remote", REMOTE);
        // Special-use attributes
        (1 << 9, "\\All", ALL);
        (1 << 10, "\\Archive", ARCHIVE);
        (1 << 11, "\\Drafts", DRAFTS);
        (1 << 12, "\\Flagged", FLAGGED);
        (1 << 13, "\\Junk", JUNK);
        (1 << 14, "\\Sent", SENT);
        (1 << 15, "\\Trash", TRASH);
    }
}

impl fmt::Display for Attributes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_paren_list(f, self.names())
    }
}

pub struct ListItem {
    pub name: String,
    pub attributes: Attributes,
    pub hierarchy_delimiter: Option<char>,
}

impl ListItem {
    pub fn new(name: impl Into<String>, attributes: Attributes) -> Self {
        Self {
            name: name.into(),
            attributes,
            hierarchy_delimiter: None,
        }
    }
}

struct DelimiterDisplay<'a>(&'a Option<char>);

impl fmt::Display for DelimiterDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(delimiter) => write!(f, "\"{delimiter}\""),
            None => write!(f, "NIL"),
        }
    }
}

impl fmt::Display for ListItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "* LIST {} {} \"{}\"\r\n",
            self.attributes,
            DelimiterDisplay(&self.hierarchy_delimiter),
            self.name
        )
    }
}

pub struct Response {
    pub list_items: Vec<ListItem>,
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for item in &self.list_items {
            item.fmt(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_item_fmt() {
        assert_eq!(
            ListItem {
                attributes: Attributes::DRAFTS,
                name: "Drafts".to_string(),
                hierarchy_delimiter: Some('/'),
            }
            .to_string(),
            "* LIST (\\Drafts) \"/\" \"Drafts\"\r\n"
        );

        assert_eq!(
            ListItem {
                attributes: Attributes::NOSELECT | Attributes::NOINFERIORS,
                name: "INBOX".to_string(),
                hierarchy_delimiter: None,
            }
            .to_string(),
            "* LIST (\\Noinferiors \\Noselect) NIL \"INBOX\"\r\n"
        )
    }
}

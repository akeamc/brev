use std::fmt;

pub struct Response {
    pub n: u32,
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} RECENT", self.n)
    }
}

use std::{borrow::Cow, fmt};

use auth::{sasl::MechanismError, ValidationError};

use crate::Tag;

#[derive(Debug)]
pub enum Status {
    Ok,
    No,
    Bad,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Ok => write!(f, "OK"),
            Status::No => write!(f, "NO"),
            Status::Bad => write!(f, "BAD"),
        }
    }
}

#[derive(Debug)]
pub struct StatusResponse {
    pub status: Status,
    pub message: Cow<'static, str>,
}

impl StatusResponse {
    pub fn new(status: Status, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn ok(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(Status::Ok, message)
    }

    pub fn bad(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(Status::Bad, message)
    }

    pub fn no(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(Status::No, message)
    }

    pub fn with_tag(self, tag: impl Into<Tag>) -> TaggedStatusResponse {
        TaggedStatusResponse {
            tag: tag.into(),
            status: self.status,
            message: self.message,
        }
    }
}

#[derive(Debug)]
pub struct TaggedStatusResponse {
    pub tag: Tag,
    pub status: Status,
    pub message: Cow<'static, str>,
}

impl fmt::Display for TaggedStatusResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}\r\n", self.tag, self.status, self.message)
    }
}

impl From<ValidationError> for StatusResponse {
    fn from(value: ValidationError) -> Self {
        match value {
            ValidationError::InvalidCredentials => Self::no("invalid credentials"),
            ValidationError::Unknown => Self::bad("invalid identity"),
        }
    }
}

impl From<MechanismError> for StatusResponse {
    fn from(value: MechanismError) -> Self {
        match value {
            MechanismError::Decode => Self::bad("failed to decode response"),
            MechanismError::Validation(e) => e.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::response::{Status, TaggedStatusResponse};

    #[test]
    fn fmt() {
        assert_eq!(
            TaggedStatusResponse {
                tag: "A0001".into(),
                status: Status::Ok,
                message: "Nice".into(),
            }
            .to_string(),
            "A0001 OK Nice\r\n"
        );
    }
}

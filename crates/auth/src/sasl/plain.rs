use async_trait::async_trait;
use secrecy::SecretString;

use crate::Credentials;

use super::Mechanism;

#[derive(Debug)]
pub enum DecodeError {
    Utf8,
    MissingParts,
}

impl From<std::str::Utf8Error> for DecodeError {
    fn from(_: std::str::Utf8Error) -> Self {
        Self::Utf8
    }
}

impl From<DecodeError> for super::MechanismError {
    fn from(_: DecodeError) -> Self {
        Self::Decode
    }
}

/// Decode base64-encoded credentials.
///
/// ```text
/// C: AUTH PLAIN
/// S: +
/// C: AGJvYgBodW50ZXIy
/// ```
///
/// ```
/// # use auth::Credentials;
/// # use auth::sasl::plain::{decode, DecodeError};
/// # use secrecy::ExposeSecret;
/// let Credentials { username, password } = decode(b"\0bob\0hunter2")?;
/// assert_eq!(username, "bob");
/// assert_eq!(password.expose_secret(), "hunter2");
/// # Ok::<(), DecodeError>(())
/// ```
pub fn decode(data: &[u8]) -> Result<Credentials, DecodeError> {
    let mut parts = std::str::from_utf8(&data)?.splitn(3, '\0').skip(1);
    let username = parts.next().ok_or(DecodeError::MissingParts)?;
    let password = parts.next().ok_or(DecodeError::MissingParts)?;

    Ok(Credentials {
        username: username.to_owned(),
        password: SecretString::new(password.to_owned()),
    })
}

pub struct Plain {
    _private: (),
}

#[async_trait]
impl Mechanism for Plain {
    fn init() -> (Self, Vec<u8>) {
        (Self { _private: () }, Vec::new())
    }

    async fn eat<A: crate::Validator>(
        &mut self,
        validator: &A,
        challenge: &[u8],
    ) -> Result<super::Response, super::MechanismError> {
        let credentials = decode(challenge)?;
        let identity = validator.validate(&credentials).await?;
        Ok(super::Response::Success(identity))
    }
}

use crate::Identity;

pub mod plain;

pub use plain::Plain;

#[derive(Debug, PartialEq, Eq)]
pub enum MechanismKind {
    Plain,
}

impl std::str::FromStr for MechanismKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "PLAIN" => Ok(Self::Plain),
            _ => Err(()),
        }
    }
}

pub enum Response {
    Success(Identity),
    Proceed(Vec<u8>),
}

#[derive(Debug, thiserror::Error)]
pub enum MechanismError {
    #[error(transparent)]
    Validation(#[from] crate::ValidationError),
    #[error("decode error")]
    Decode,
}

#[async_trait::async_trait]
pub trait Mechanism: Sized {
    fn init() -> (Self, Vec<u8>);

    async fn eat<V: crate::Validator>(&mut self, validator: &V, bytes: &[u8]) -> EatResult;
}

pub type EatResult = Result<Response, MechanismError>;

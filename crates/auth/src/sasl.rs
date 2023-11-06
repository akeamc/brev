use std::ops::ControlFlow;

use crate::Identity;

pub mod plain;

pub use plain::Plain;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum WhichMechanism {
    Plain,
}

impl std::str::FromStr for WhichMechanism {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "PLAIN" => Ok(Self::Plain),
            _ => Err(()),
        }
    }
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

    async fn eat<V: crate::Validator>(&mut self, validator: &V, bytes: &[u8]) -> MechanismResult;
}

pub type MechanismResult = Result<ControlFlow<Identity, Vec<u8>>, MechanismError>;

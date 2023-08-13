use auth::{
    sasl::{self, EatResult, Mechanism, MechanismError, MechanismKind, Plain},
    Identity, ValidationError, Validator,
};
use base64::Engine;
use line::{read_line, write, write_flush, ReadLineError};
use tokio::io::{AsyncBufRead, AsyncWrite};

use crate::{
    command::{self, Authenticate},
    response::StatusResponse,
};

const BASE64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

enum Authenticator {
    Plain(Plain),
}

impl Authenticator {
    fn init(mechanism: &MechanismKind) -> (Self, Vec<u8>) {
        match mechanism {
            MechanismKind::Plain => {
                let (plain, challenge) = Plain::init();
                (Self::Plain(plain), challenge)
            }
        }
    }

    async fn eat<A: Validator>(&mut self, validator: &A, base64: &[u8]) -> EatResult {
        // trim trailing whitespace
        let i = base64
            .iter()
            .rposition(|&c| c != b'\r' && c != b'\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let base64 = &base64[..i];

        let bytes = if base64 == b"=" {
            vec![]
        } else {
            match BASE64.decode(base64) {
                Ok(bytes) => bytes,
                Err(_) => return Err(MechanismError::Decode),
            }
        };

        match self {
            Self::Plain(plain) => plain.eat(validator, &bytes).await,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Mechanism(#[from] MechanismError),
}

impl From<ReadLineError> for Error {
    fn from(value: ReadLineError) -> Self {
        match value {
            ReadLineError::Io(io) => Self::Io(io),
            ReadLineError::Eof => Self::Io(std::io::ErrorKind::UnexpectedEof.into()),
        }
    }
}

pub async fn authenticate<S: AsyncBufRead + AsyncWrite + Unpin, A: Validator>(
    stream: &mut S,
    data: command::Authenticate,
    validator: &A,
) -> Result<Identity, Error> {
    let Authenticate {
        mechanism,
        mut initial_response,
    } = data;

    let (mut authenticator, mut challenge) = Authenticator::init(&mechanism);

    loop {
        let line = if let Some(initial_response) = initial_response.take() {
            initial_response.into_bytes()
        } else {
            // send challenge
            write(stream, "+ ").await?;
            write(stream, BASE64.encode(&challenge)).await?;
            write_flush(stream, "\r\n").await?;

            // read response
            let mut buf = Vec::new();
            read_line(stream, &mut buf).await?;
            buf
        };

        match authenticator.eat(validator, &line).await? {
            sasl::Response::Success(identity) => {
                return Ok(identity);
            }
            sasl::Response::Proceed(bytes) => {
                challenge = bytes;
            }
        }
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

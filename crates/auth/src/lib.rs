use secrecy::SecretString;

pub mod sasl;

pub struct Credentials {
    pub username: String,
    pub password: SecretString,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity(pub String);

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("unknown error")]
    Unknown,
}

#[async_trait::async_trait]
pub trait Validator: Send + Sync {
    async fn validate(&self, credentials: &Credentials) -> Result<Identity, ValidationError>;
}

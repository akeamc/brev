use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::rustls;

use self::session::Session;

pub mod session;

#[derive(Debug)]
pub struct Context<A: auth::Validator> {
    pub hostname: String,
    pub tls: Option<Arc<rustls::ServerConfig>>,
    pub auth: Arc<A>,
}

impl<A: auth::Validator> Clone for Context<A> {
    fn clone(&self) -> Self {
        Self {
            hostname: self.hostname.clone(),
            tls: self.tls.clone(),
            auth: Arc::clone(&self.auth),
        }
    }
}

pub struct Server<A: auth::Validator> {
    context: Context<A>,
}

impl<A: auth::Validator> Server<A> {
    #[must_use]
    pub fn new(context: Context<A>) -> Self {
        Self { context }
    }

    #[must_use]
    pub fn accept<IO: AsyncRead + AsyncWrite + Unpin>(&self, stream: IO) -> Session<IO, A> {
        Session::new(stream, self.context.clone())
    }
}

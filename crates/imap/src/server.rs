pub mod ops;
mod session;

use std::sync::Arc;

use line::stream::{MaybeTlsStream, ServerTlsStream};
pub use session::Session;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct Context<A: auth::Validator> {
    pub tls: Option<Arc<rustls::ServerConfig>>,
    pub auth: Arc<A>,
}

impl<V: auth::Validator> Clone for Context<V> {
    fn clone(&self) -> Self {
        Self {
            tls: self.tls.clone(),
            auth: Arc::clone(&self.auth),
        }
    }
}

pub struct Server<V: auth::Validator> {
    context: Context<V>,
}

impl<A: auth::Validator> Server<A> {
    #[must_use]
    pub fn new(context: Context<A>) -> Self {
        Self { context }
    }

    pub fn accept<IO: AsyncRead + AsyncWrite + Unpin>(
        &self,
        stream: impl Into<MaybeTlsStream<ServerTlsStream<IO>, IO>>,
    ) -> Session<IO, A> {
        Session::new(stream, self.context.clone())
    }
}

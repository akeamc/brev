pub mod ops;
mod queue;
pub mod session;

use std::sync::Arc;

use imap_proto::command::TaggedCommand;
use line::{
    stream::{MaybeTls, ServerTlsStream},
    ReadLineError,
};
pub use session::Session;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite};
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct Context<A: auth::Validator> {
    pub tls: Option<Arc<rustls::ServerConfig>>,
    pub auth: Arc<A>,
}

impl<A: auth::Validator> Clone for Context<A> {
    fn clone(&self) -> Self {
        Self {
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

    pub fn accept<IO: AsyncRead + AsyncWrite + Unpin>(
        &self,
        stream: impl Into<MaybeTls<ServerTlsStream<IO>, IO>>,
    ) -> Session<IO, A> {
        Session::new(stream, self.context.clone())
    }
}

#[instrument(skip_all)]
pub async fn read_cmd<S: AsyncRead + AsyncBufRead + AsyncWrite + Unpin>(
    stream: &mut S,
) -> std::io::Result<Option<TaggedCommand>> {
    use imap_proto::command::Error;

    let mut buf = Vec::new();
    loop {
        match line::read_line(stream, &mut buf).await {
            Ok(()) => match TaggedCommand::try_from(&buf[..]) {
                Ok(cmd) => return Ok(Some(cmd)),
                Err(Error::Bad(res)) => {
                    line::write_flush(stream, res.to_string()).await?;
                }
                Err(Error::InvalidUtf8) => debug!("invalid utf8"),
            },
            Err(ReadLineError::Eof) => return Ok(None),
            Err(ReadLineError::Io(e)) => return Err(e),
        }

        buf.clear();
    }
}

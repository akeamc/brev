use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::rustls;

use self::session::Session;

pub mod session;

#[derive(Debug, Clone)]
pub struct Config {
    pub hostname: String,
    pub tls: Option<Arc<rustls::ServerConfig>>,
}

pub struct Server {
    config: Config,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn accept<IO: AsyncRead + AsyncWrite + Unpin>(&self, stream: IO) -> Session<IO> {
        Session::new(stream, self.config.clone())
    }
}

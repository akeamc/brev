use std::{net::SocketAddr, sync::Arc};

use line::stream::{MaybeTls, ServerTlsStream};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio_rustls::{rustls, TlsAcceptor};
use tracing::info;

pub struct MultiListener {
    plain: TcpListener,
    tls: Option<(TcpListener, Arc<rustls::ServerConfig>)>,
}

impl MultiListener {
    pub async fn new(plain: impl ToSocketAddrs) -> std::io::Result<Self> {
        let plain: TcpListener = TcpListener::bind(plain).await?;
        info!("Binding {}", plain.local_addr()?);
        Ok(Self { plain, tls: None })
    }

    pub async fn with_tls(
        mut self,
        addr: impl ToSocketAddrs,
        config: Arc<rustls::ServerConfig>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        info!("Binding {} (TLS)", listener.local_addr()?);
        self.tls = Some((listener, config));
        Ok(self)
    }

    /// Accept a TLS connection if TLS is enabled. If not, a forever
    /// pending future is returned.
    async fn accept_tls(&self) -> std::io::Result<(ServerTlsStream<TcpStream>, SocketAddr)> {
        match &self.tls {
            Some((tls, config)) => {
                let (stream, addr) = tls.accept().await?;
                TlsAcceptor::from(config.clone())
                    .accept(stream)
                    .await
                    .map(|stream| (stream, addr))
            }
            None => std::future::pending().await,
        }
    }

    pub async fn accept(
        &self,
    ) -> std::io::Result<(MaybeTls<ServerTlsStream<TcpStream>, TcpStream>, SocketAddr)> {
        tokio::select! {
            plain = self.plain.accept() => {
                let (stream, addr) = plain?;
                Ok((MaybeTls::from_plain(stream), addr))
            }
            tls = self.accept_tls() => {
                let (stream, addr) = tls?;
                Ok((MaybeTls::from_tls(stream), addr))
            }
        }
    }
}

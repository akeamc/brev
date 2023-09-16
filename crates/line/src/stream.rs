use std::{future::Future, pin::Pin};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_rustls::rustls::ServerName;

pub use tokio_rustls::{
    client::TlsStream as ClientTlsStream, server::TlsStream as ServerTlsStream,
};

pub trait Tls<IO>: Sized + AsyncRead + AsyncWrite + Unpin {
    /// The type of the future returned by [`Tls::upgrade`].
    type Upgrade: Future<Output = Result<Self, (std::io::Error, IO)>>;

    /// The type of the configuration used by [`Tls::upgrade`].
    type Config<'a>;

    /// Upgrade a plaintext stream to TLS.
    fn upgrade(plain: IO, config: Self::Config<'_>) -> Self::Upgrade;
}

impl<IO: AsyncRead + AsyncWrite + Unpin> Tls<IO> for ServerTlsStream<IO> {
    type Upgrade = tokio_rustls::FallibleAccept<IO>;

    type Config<'a> = &'a tokio_rustls::TlsAcceptor;

    fn upgrade(plain: IO, config: Self::Config<'_>) -> Self::Upgrade {
        config.accept(plain).into_fallible()
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin> Tls<IO> for ClientTlsStream<IO> {
    type Upgrade = tokio_rustls::FallibleConnect<IO>;

    type Config<'a> = (ServerName, &'a tokio_rustls::TlsConnector);

    fn upgrade(plain: IO, (domain, config): Self::Config<'_>) -> Self::Upgrade {
        config.connect(domain, plain).into_fallible()
    }
}

enum Inner<T: Tls<IO>, IO> {
    Plain(IO),
    Tls(T),
    Empty,
}

/// A stream that may or may not be encrypted.
///
/// This is used in STARTTLS implementations.
pub struct MaybeTls<T: Tls<IO>, IO> {
    inner: Inner<T, IO>,
}

impl<T: Tls<IO>, IO> From<IO> for MaybeTls<T, IO> {
    fn from(plain: IO) -> Self {
        Self::from_plain(plain)
    }
}

impl<T: Tls<IO>, IO> MaybeTls<T, IO> {
    pub const fn from_plain(plain: IO) -> Self {
        Self {
            inner: Inner::Plain(plain),
        }
    }

    pub const fn from_tls(tls: T) -> Self {
        Self {
            inner: Inner::Tls(tls),
        }
    }

    pub const fn is_plain(&self) -> bool {
        matches!(self.inner, Inner::Plain(_))
    }

    pub const fn is_tls(&self) -> bool {
        matches!(self.inner, Inner::Tls(_))
    }
}

async fn upgrade<T: Tls<IO>, IO>(
    inner: Inner<T, IO>,
    config: T::Config<'_>,
) -> (MaybeTls<T, IO>, std::io::Result<()>) {
    match inner {
        Inner::Plain(plain) => match T::upgrade(plain, config).await {
            Ok(tls) => (MaybeTls::from_tls(tls), Ok(())),
            Err((err, plain)) => (MaybeTls::from_plain(plain), Err(err)),
        },
        Inner::Tls(plain) => (MaybeTls::from_tls(plain), Ok(())),
        Inner::Empty => unreachable!(),
    }
}

impl<T: Tls<IO>, IO: AsyncRead + AsyncWrite + Unpin> MaybeTls<T, IO> {
    /// Upgrade the stream to TLS.
    ///
    /// If the stream is already encrypted, this is a no-op and `Ok(())` is
    /// returned.
    ///
    /// # Errors
    ///
    /// If the TLS handshake fails, an error is returned and the stream is
    /// reverted to plaintext.
    pub async fn upgrade(&mut self, config: T::Config<'_>) -> std::io::Result<()> {
        let (stream, result) =
            upgrade(std::mem::replace(&mut self.inner, Inner::Empty), config).await;
        *self = stream;
        result
    }
}

impl<T: Tls<IO>, IO: AsyncRead + AsyncWrite + Unpin> AsyncRead for MaybeTls<T, IO> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut self.inner {
            Inner::Plain(stream) => Pin::new(stream).poll_read(cx, buf),
            Inner::Tls(stream) => Pin::new(stream).poll_read(cx, buf),
            Inner::Empty => unreachable!(),
        }
    }
}

impl<T: Tls<IO>, IO: AsyncRead + AsyncWrite + Unpin> AsyncWrite for MaybeTls<T, IO> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match &mut self.inner {
            Inner::Plain(stream) => Pin::new(stream).poll_write(cx, buf),
            Inner::Tls(stream) => Pin::new(stream).poll_write(cx, buf),
            Inner::Empty => unreachable!(),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut self.inner {
            Inner::Plain(stream) => Pin::new(stream).poll_flush(cx),
            Inner::Tls(stream) => Pin::new(stream).poll_flush(cx),
            Inner::Empty => unreachable!(),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut self.inner {
            Inner::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            Inner::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
            Inner::Empty => unreachable!(),
        }
    }
}

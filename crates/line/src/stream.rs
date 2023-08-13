use std::{future::Future, pin::Pin};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_rustls::rustls::ServerName;

pub use tokio_rustls::{
    client::TlsStream as ClientTlsStream, server::TlsStream as ServerTlsStream,
};

pub trait Tls<IO>: Sized + AsyncRead + AsyncWrite + Unpin {
    type Future: Future<Output = Result<Self, (std::io::Error, IO)>>;

    type Config<'a>;

    fn upgrade(plain: IO, config: Self::Config<'_>) -> Self::Future;
}

impl<IO: AsyncRead + AsyncWrite + Unpin> Tls<IO> for ServerTlsStream<IO> {
    type Future = tokio_rustls::FallibleAccept<IO>;

    type Config<'a> = &'a tokio_rustls::TlsAcceptor;

    fn upgrade(plain: IO, config: Self::Config<'_>) -> Self::Future {
        config.accept(plain).into_fallible()
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin> Tls<IO> for ClientTlsStream<IO> {
    type Future = tokio_rustls::FallibleConnect<IO>;

    type Config<'a> = (ServerName, &'a tokio_rustls::TlsConnector);

    fn upgrade(plain: IO, (domain, config): Self::Config<'_>) -> Self::Future {
        config.connect(domain, plain).into_fallible()
    }
}

enum Inner<T: Tls<IO>, IO> {
    Plain(IO),
    Tls(T),
    Empty,
}

pub struct MaybeTlsStream<T: Tls<IO>, IO> {
    inner: Inner<T, IO>,
}

impl<T: Tls<IO>, IO> From<IO> for MaybeTlsStream<T, IO> {
    fn from(plain: IO) -> Self {
        Self::plain(plain)
    }
}

impl<T: Tls<IO>, IO> MaybeTlsStream<T, IO> {
    pub const fn plain(plain: IO) -> Self {
        Self {
            inner: Inner::Plain(plain),
        }
    }

    pub const fn tls(tls: T) -> Self {
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
) -> (MaybeTlsStream<T, IO>, std::io::Result<()>) {
    match inner {
        Inner::Plain(plain) => match T::upgrade(plain, config).await {
            Ok(tls) => (MaybeTlsStream::tls(tls), Ok(())),
            Err((err, plain)) => (MaybeTlsStream::plain(plain), Err(err)),
        },
        Inner::Tls(plain) => (MaybeTlsStream::tls(plain), Ok(())),
        Inner::Empty => unreachable!(),
    }
}

impl<T: Tls<IO>, IO: AsyncRead + AsyncWrite + Unpin> MaybeTlsStream<T, IO> {
    pub async fn upgrade(&mut self, config: T::Config<'_>) -> std::io::Result<()> {
        let (stream, result) =
            upgrade(std::mem::replace(&mut self.inner, Inner::Empty), config).await;
        *self = stream;
        result
    }
}

impl<T: Tls<IO>, IO: AsyncRead + AsyncWrite + Unpin> AsyncRead for MaybeTlsStream<T, IO> {
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

impl<T: Tls<IO>, IO: AsyncRead + AsyncWrite + Unpin> AsyncWrite for MaybeTlsStream<T, IO> {
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

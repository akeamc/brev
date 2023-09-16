use std::{collections::HashSet, pin::Pin};

use email_address::EmailAddress;
use line::write_flush;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite};
use tracing::instrument;

use self::{bdat::Bdat, data::Data};

mod bdat;
mod data;

#[cfg(fuzzing)]
pub use data::fuzz as data_fuzz;

#[derive(Debug)]
pub struct Envelope {
    pub from: EmailAddress,
    pub recipients: HashSet<EmailAddress>,
}

impl Envelope {
    #[must_use]
    pub fn new(from: EmailAddress) -> Self {
        Self {
            from,
            recipients: HashSet::new(),
        }
    }
}

enum Inner<'a, S: AsyncRead + AsyncWrite + Unpin> {
    Data(Data<'a, S>),
    Bdat(Bdat<'a, S>),
}

pub struct Incoming<'a, S: AsyncRead + AsyncWrite + Unpin> {
    envelope: Envelope,
    inner: Inner<'a, S>,
}

impl<'a, S: AsyncRead + AsyncWrite + Unpin> Incoming<'a, S> {
    pub(crate) fn data(envelope: Envelope, stream: &'a mut S) -> Self {
        Self {
            envelope,
            inner: Inner::Data(Data::new(stream)),
        }
    }

    pub(crate) fn bdat(envelope: Envelope, remaining: u64, last: bool, stream: &'a mut S) -> Self {
        Self {
            envelope,
            inner: Inner::Bdat(Bdat::new(stream, remaining, last)),
        }
    }

    #[must_use]
    pub fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    fn take_stream(self) -> Option<&'a mut S> {
        match self.inner {
            Inner::Data(data) => Some(data.into_stream()),
            Inner::Bdat(mut bdat) => bdat.take_stream(),
        }
    }

    #[instrument(skip_all)]
    pub async fn accept(self) -> std::io::Result<()> {
        write_flush(self.take_stream().unwrap(), "250 ok\r\n").await
    }

    #[instrument(skip_all)]
    pub async fn reject(self) -> std::io::Result<()> {
        write_flush(self.take_stream().unwrap(), "554 nope\r\n").await
    }
}

impl<S: AsyncRead + AsyncBufRead + AsyncWrite + Unpin + Send + Sync> AsyncRead for Incoming<'_, S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut self.inner {
            Inner::Data(data) => Pin::new(data).poll_read(cx, buf),
            Inner::Bdat(bdat) => Pin::new(bdat).poll_read(cx, buf),
        }
    }
}

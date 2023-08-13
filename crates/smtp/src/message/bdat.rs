use std::{pin::Pin, task::Poll};

use futures_util::{future::BoxFuture, ready, FutureExt};
use line::write_flush;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncWrite};
use tracing::{debug, instrument};

use crate::{
    command::{read_cmd, Command},
    io::bye,
};

// future that returns the stream politely
type WriteFuture<'a, S> = BoxFuture<'a, (&'a mut S, std::io::Result<(u64, bool)>)>;

pub enum Inner<'a, S: AsyncRead + AsyncWrite + Unpin> {
    Read {
        stream: tokio::io::Take<&'a mut S>,
        last: bool,
    },
    Write(WriteFuture<'a, S>),
    None,
}

pub struct Bdat<'a, S: AsyncRead + AsyncWrite + Unpin> {
    inner: Inner<'a, S>,
}

impl<'a, S: AsyncRead + AsyncWrite + Unpin> Bdat<'a, S> {
    pub fn new(stream: &'a mut S, size: u64, last: bool) -> Self {
        Self {
            inner: Inner::Read {
                stream: stream.take(size),
                last,
            },
        }
    }

    pub fn take_stream(&mut self) -> Option<&'a mut S> {
        match std::mem::replace(&mut self.inner, Inner::None) {
            Inner::Read { stream, .. } => Some(stream.into_inner()),
            _ => None,
        }
    }
}

#[instrument(skip_all)]
async fn next_bdat<S: AsyncRead + AsyncBufRead + AsyncWrite + Unpin>(
    stream: &mut S,
) -> std::io::Result<(u64, bool)> {
    write_flush(stream, "250 ok\r\n").await?; // request more data

    loop {
        match read_cmd(stream).await? {
            Some(Command::Bdat { size, last }) => {
                debug!(?size, ?last, "got bdat command");
                return Ok((size, last));
            }
            Some(Command::Quit) => {
                bye(stream).await?;
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
            }
            Some(Command::Rset) | None => {
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))
            }
            Some(Command::Noop) => write_flush(stream, "250 ok\r\n").await?,
            Some(cmd) => {
                debug!(?cmd, "unexpected command");
                write_flush(stream, "503 expected BDAT\r\n").await?;
            }
        }
    }
}

impl<'a, T: AsyncRead + AsyncBufRead + AsyncWrite + Unpin + Send + Sync> AsyncRead for Bdat<'a, T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        loop {
            match &mut self.inner {
                Inner::Read { stream, last } => {
                    let before = buf.filled().len();
                    let mut stream = Pin::new(stream);
                    ready!(stream.as_mut().poll_read(cx, buf))?;

                    if before != buf.filled().len() {
                        return Poll::Ready(Ok(()));
                    }

                    if stream.limit() == 0 {
                        if *last {
                            return std::task::Poll::Ready(Ok(()));
                        }

                        let stream = self.take_stream().unwrap();
                        self.inner = Inner::Write(
                            async move {
                                let res = next_bdat(stream).await;
                                (stream, res)
                            }
                            .boxed(),
                        );
                    } else {
                        return Poll::Ready(Err(std::io::Error::from(
                            std::io::ErrorKind::UnexpectedEof,
                        )));
                    }
                }
                Inner::Write(future) => {
                    let (stream, result) = futures_util::ready!(future.poll_unpin(cx));
                    let (size, last) = result?;

                    if size == 0 && last {
                        return Poll::Ready(Ok(()));
                    }

                    self.inner = Inner::Read {
                        stream: stream.take(size),
                        last,
                    };
                }
                Inner::None => unreachable!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

    use super::Bdat;

    #[tokio::test]
    async fn bdat() -> anyhow::Result<()> {
        let (mut client, server) = tokio::io::duplex(1024);

        let task = tokio::spawn(async move {
            let mut server = BufReader::new(server);
            let mut bdat = Bdat::new(&mut server, 4, false); // C: BDAT 4

            let mut buf = Vec::new();
            bdat.read_to_end(&mut buf).await?;
            anyhow::Ok(buf)
        });

        client.write_all(b"Edel").await?;
        client.write_all(b"BDAT 2\r\n").await?;
        client.write_all(b"we").await?;
        client.write_all(b"BDAT 2 LAST\r\n").await?;
        client.write_all(b"i\xDF").await?;

        assert_eq!(task.await??, b"Edelwei\xDF");

        let mut from_server = String::new();
        client.read_to_string(&mut from_server).await?;
        assert_eq!(from_server, "250 ok\r\n250 ok\r\n");

        Ok(())
    }
}

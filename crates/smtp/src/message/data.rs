use std::{
    pin::{pin, Pin},
    task::Poll,
};

use futures_util::{ready, FutureExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum State {
    #[default]
    Start,
    Cr,
    CrLf,
    CrLfDot,
    CrLfDotCr,
    CrLfDotDot,
    CrLfDotDotCr,
    Eof,
}

impl State {
    const fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Cr => b"\r",
            Self::CrLf => b"\r\n",
            Self::CrLfDot => b"\r\n.",
            Self::CrLfDotCr => b"\r\n.\r",
            Self::CrLfDotDot => b"\r\n..",
            Self::CrLfDotDotCr => b"\r\n..\r",
            Self::Start | Self::Eof => unreachable!(),
        }
    }

    fn advance(&mut self, buf: &mut ReadBuf<'_>, b: u8) {
        *self = match (*self, b) {
            (State::Start, b'\r') => State::Cr,
            (State::Start, _) => {
                buf.put_slice(&[b]);
                return;
            }
            (State::Cr, b'\n') => State::CrLf,
            (State::CrLf, b'.') => State::CrLfDot,
            (State::CrLfDot, b'\r') => State::CrLfDotCr,
            (State::CrLfDot, b'.') => State::CrLfDotDot,
            (State::CrLfDotDot, b'\r') => State::CrLfDotDotCr,
            (State::CrLfDotDotCr, b'\n') => {
                buf.put_slice(b"\r\n."); // unescape dot
                State::CrLf // and continue
            }
            (State::CrLfDotCr, b'\n') => {
                buf.put_slice(b"\r\n");
                State::Eof
            }
            (State::Eof, _) => panic!("unexpected data after end of message"),
            (state, _) => {
                buf.put_slice(state.as_bytes());
                State::Start
            }
        };

        if *self == State::Start {
            self.advance(buf, b);
        }
    }
}

/// Unbuffered data stream (`S` should be buffered already).
pub struct Data<'a, S: AsyncRead + AsyncWrite + Unpin> {
    pub stream: &'a mut S,
    state: State,
}

impl<'a, S: AsyncRead + AsyncWrite + Unpin> Data<'a, S> {
    pub fn new(stream: &'a mut S) -> Self {
        Self {
            stream,
            state: State::default(),
        }
    }

    pub fn into_stream(self) -> &'a mut S {
        self.stream
    }
}

impl<'a, S: AsyncRead + AsyncWrite + Unpin> AsyncRead for Data<'a, S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let filled_before = buf.filled().len();

        while this.state != State::Eof {
            if buf.remaining() < 5 {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }

            let b = match ready!(pin!(this.stream.read_u8()).poll_unpin(cx)) {
                Ok(b) => b,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Poll::Ready(Ok(()));
                }
                Err(e) => return Poll::Ready(Err(e)),
            };

            this.state.advance(buf, b);

            if buf.filled().len() != filled_before {
                return Poll::Ready(Ok(()));
            }
        }

        Poll::Ready(Ok(()))
    }
}

#[cfg(fuzzing)]
pub async fn fuzz(data: Box<[u8]>) {
    use tokio::io::AsyncWriteExt;

    let (mut client, server) = tokio::io::duplex(1024);
    let mut server = BufReader::new(server);
    let mut reader = Data::new(&mut server);

    client.write_all(&data).await.unwrap();
    client.shutdown().await.unwrap();

    let mut data = Vec::new();
    reader.read_to_end(&mut data).await.unwrap();
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

    use super::Data;

    #[tokio::test]
    async fn data() -> anyhow::Result<()> {
        let (mut client, server) = tokio::io::duplex(1024);
        let mut server = BufReader::new(server);
        let mut reader = Data::new(&mut server);

        client
            .write_all(b"We've been trying to reach you\r\n")
            .await?;
        client
            .write_all(b"about your car's extended warranty.\r\n")
            .await?;
        client.write_all(b"..\r\n").await?; // Lone dots are escaped using another dot per RFC 5321
        client.write_all(b".\r\n").await?;
        client.shutdown().await?;

        let mut message = String::new();
        reader.read_to_string(&mut message).await?;
        assert_eq!(
            message,
            "We've been trying to reach you\r\nabout your car's extended warranty.\r\n.\r\n"
        );

        Ok(())
    }
}

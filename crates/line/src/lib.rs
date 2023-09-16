pub mod stream;

use stream::{MaybeTls, Tls};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tracing::debug;

pub async fn write<S: AsyncWrite + Unpin>(
    stream: &mut S,
    src: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    let src = src.as_ref();
    debug!("write: {:?}", String::from_utf8_lossy(src));
    stream.write_all(src).await
}

pub async fn write_flush<S: AsyncWrite + Unpin>(
    stream: &mut S,
    src: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    write(stream, src).await?;
    stream.flush().await
}

pub enum ReadLineError {
    Io(std::io::Error),
    Eof,
}

impl From<std::io::Error> for ReadLineError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub async fn read_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut Vec<u8>,
) -> Result<(), ReadLineError> {
    assert!(buf.is_empty(), "buffer must be empty");

    if reader.read_until(b'\n', buf).await? == 0 {
        return Err(ReadLineError::Eof);
    }

    debug!("read: {:?}", String::from_utf8_lossy(buf));

    let rpos = buf
        .iter()
        .rposition(|&c| c != b'\r' && c != b'\n')
        .map(|pos| pos + 1)
        .unwrap_or(0);
    buf.truncate(rpos);

    Ok(())
}

pub struct Connection<T: Tls<IO>, IO: AsyncRead + AsyncWrite + Unpin> {
    stream: BufReader<MaybeTls<T, IO>>,
}

impl<T: Tls<IO>, IO: AsyncRead + AsyncWrite + Unpin> Connection<T, IO> {
    pub fn new(stream: impl Into<MaybeTls<T, IO>>) -> Self {
        Self {
            stream: BufReader::new(stream.into()),
        }
    }

    pub fn stream_mut(&mut self) -> &mut BufReader<MaybeTls<T, IO>> {
        &mut self.stream
    }

    pub async fn write(&mut self, src: impl AsRef<[u8]>) -> std::io::Result<()> {
        write(&mut self.stream, src).await
    }

    pub async fn write_flush(&mut self, src: impl AsRef<[u8]>) -> std::io::Result<()> {
        write_flush(&mut self.stream, src).await
    }

    pub async fn upgrade(&mut self, tls_config: T::Config<'_>) -> std::io::Result<()> {
        assert!(self.stream.buffer().is_empty(), "buffer must be empty");
        self.stream.get_mut().upgrade(tls_config).await
    }

    pub fn is_plain(&self) -> bool {
        self.stream.get_ref().is_plain()
    }

    pub fn is_tls(&self) -> bool {
        self.stream.get_ref().is_tls()
    }
}

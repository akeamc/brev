use tokio::io::{AsyncWrite, AsyncWriteExt};

/// Send a 221 response and shutdown the connection.
///
/// This is the only acceptable way to respond to a `QUIT` command.
pub async fn bye<S: AsyncWrite + Unpin>(stream: &mut S) -> std::io::Result<()> {
    stream.write_all(b"221 Bye\r\n").await?;
    stream.shutdown().await?;
    Ok(())
}

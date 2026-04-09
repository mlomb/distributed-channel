use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::AsyncWriteExt;

const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024; // 16 MiB

/// Write a length-prefixed postcard-serialized message to a QUIC send stream.
pub async fn write_message<T: Serialize>(
    send: &mut iroh::endpoint::SendStream,
    msg: &T,
) -> Result<()> {
    use iroh::endpoint::WriteError;
    let bytes = postcard::to_allocvec(msg).context("serialize message")?;
    let len = (bytes.len() as u32).to_le_bytes();
    send.write_all(&len)
        .await
        .map_err(|e: WriteError| anyhow::anyhow!(e))
        .context("write length")?;
    send.write_all(&bytes)
        .await
        .map_err(|e: WriteError| anyhow::anyhow!(e))
        .context("write payload")?;
    AsyncWriteExt::flush(send)
        .await
        .context("flush stream")?;
    Ok(())
}

/// Read a length-prefixed postcard-serialized message from a QUIC recv stream.
pub async fn read_message<T: DeserializeOwned>(
    recv: &mut iroh::endpoint::RecvStream,
) -> Result<T> {
    use iroh::endpoint::ReadExactError;
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e: ReadExactError| anyhow::anyhow!(e))
        .context("read length")?;
    let len = u32::from_le_bytes(len_buf) as usize;
    anyhow::ensure!(len <= MAX_MESSAGE_SIZE, "message too large: {len} bytes");
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf)
        .await
        .map_err(|e: ReadExactError| anyhow::anyhow!(e))
        .context("read payload")?;
    postcard::from_bytes(&buf).context("deserialize message")
}

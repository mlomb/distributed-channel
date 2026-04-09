use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024; // 16 MiB

/// Write a length-prefixed postcard-serialized message.
pub async fn write_message<W: AsyncWriteExt + Unpin>(
    send: &mut W,
    msg: &impl Serialize,
) -> Result<()> {
    let bytes = postcard::to_allocvec(msg).context("serialize message")?;
    let len = (bytes.len() as u32).to_le_bytes();
    send.write_all(&len).await.context("write length")?;
    send.write_all(&bytes).await.context("write payload")?;
    send.flush().await.context("flush stream")?;
    Ok(())
}

/// Read a length-prefixed postcard-serialized message.
pub async fn read_message<T: DeserializeOwned, R: AsyncReadExt + Unpin>(
    recv: &mut R,
) -> Result<T> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await.context("read length")?;
    let len = u32::from_le_bytes(len_buf) as usize;
    anyhow::ensure!(len <= MAX_MESSAGE_SIZE, "message too large: {len} bytes");
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await.context("read payload")?;
    postcard::from_bytes(&buf).context("deserialize message")
}

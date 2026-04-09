use crate::wire::{read_message, write_message};
use crate::Networked;
use log::{info, trace, warn};
use tokio::net::{TcpListener, TcpStream};

/// Accept consumer connections and distribute work across them.
pub async fn run_producer_loop<W, R>(
    listener: TcpListener,
    work_rx: async_channel::Receiver<W>,
    work_tx: async_channel::Sender<W>,
    result_tx: async_channel::Sender<R>,
) where
    W: Networked + Sync,
    R: Networked + Sync,
{
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                info!("Consumer connected: {}", addr);
                let work_rx = work_rx.clone();
                let work_tx = work_tx.clone();
                let result_tx = result_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection::<W, R>(stream, addr, work_rx, work_tx, result_tx).await {
                        info!("Consumer {} disconnected: {}", addr, e);
                    }
                });
            }
            Err(e) => {
                warn!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_connection<W, R>(
    stream: TcpStream,
    addr: std::net::SocketAddr,
    work_rx: async_channel::Receiver<W>,
    work_tx: async_channel::Sender<W>,
    result_tx: async_channel::Sender<R>,
) -> anyhow::Result<()>
where
    W: Networked + Sync,
    R: Networked + Sync,
{
    let (mut read_half, mut write_half) = stream.into_split();

    loop {
        let work = work_rx.recv().await
            .map_err(|_| anyhow::anyhow!("work channel closed"))?;

        if let Err(e) = write_message(&mut write_half, &work).await {
            let _ = work_tx.send(work).await;
            return Err(e);
        }
        trace!("Sent work to {}", addr);

        match read_message::<R, _>(&mut read_half).await {
            Ok(result) => {
                result_tx.send(result).await
                    .map_err(|_| anyhow::anyhow!("result channel closed"))?;
                trace!("Received result from {}", addr);
            }
            Err(e) => {
                warn!("Lost result from {}: {} (work item consumed)", addr, e);
                return Err(e);
            }
        }
    }
}

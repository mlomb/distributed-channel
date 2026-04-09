use crate::discovery;
use crate::wire::{read_message, write_message};
use crate::Networked;
use log::{info, trace, warn};
use std::net::SocketAddr;
use tokio::net::TcpStream;

pub type WorkTx<W, R> = async_channel::Sender<WorkEntry<W, R>>;
pub type WorkRx<W, R> = async_channel::Receiver<WorkEntry<W, R>>;

pub struct WorkEntry<W, R> {
    pub work: W,
    pub sender: tokio::sync::oneshot::Sender<R>,
}

/// Discover producers via mDNS, connect to each, and funnel work into `work_tx`.
pub async fn run_consumer_loop<W, R>(
    service_name: String,
    peer_id: String,
    work_tx: WorkTx<W, R>,
) -> anyhow::Result<()>
where
    W: Networked + Sync,
    R: Networked + Sync,
{
    let handle = tokio::runtime::Handle::current();
    let (mut discovered_rx, _guard) = discovery::start_discovery(
        service_name,
        peer_id,
        None,
        &handle,
    )?;

    while let Some(addr) = discovered_rx.recv().await {
        info!("Discovered producer at {}, connecting...", addr);

        let work_tx = work_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = connection_loop::<W, R>(addr, work_tx).await {
                warn!("Connection to producer {} ended: {}", addr, e);
            }
        });
    }
    Ok(())
}

/// Single producer connection: read pushed work, process, send result.
async fn connection_loop<W, R>(
    addr: SocketAddr,
    work_tx: WorkTx<W, R>,
) -> anyhow::Result<()>
where
    W: Networked + Sync,
    R: Networked + Sync,
{
    let stream = TcpStream::connect(addr).await?;
    stream.set_nodelay(true)?;
    info!("Connected to producer {}", addr);

    let (mut read_half, mut write_half) = stream.into_split();

    loop {
        let work: W = read_message(&mut read_half).await?;
        trace!("Received work from {}", addr);

        let (result_tx, result_rx) = tokio::sync::oneshot::channel::<R>();
        work_tx
            .send(WorkEntry { work, sender: result_tx })
            .await
            .map_err(|_| anyhow::anyhow!("work channel closed"))?;

        let result = result_rx.await.map_err(|_| anyhow::anyhow!("result sender dropped"))?;
        write_message(&mut write_half, &result).await?;
        trace!("Sent result to {}", addr);
    }
}

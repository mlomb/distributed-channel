use crate::discovery::{self, DiscoveredPeer};
use crate::wire::{read_message, write_message};
use crate::Networked;
use iroh::{EndpointAddr, EndpointId, Endpoint, TransportAddr};
use log::{info, trace, warn};
use std::collections::HashSet;

pub type WorkTx<W, R> = async_channel::Sender<WorkEntry<W, R>>;
pub type WorkRx<W, R> = async_channel::Receiver<WorkEntry<W, R>>;

pub struct WorkEntry<W, R> {
    pub work: W,
    pub sender: tokio::sync::oneshot::Sender<R>,
}

/// Discover producers via mDNS, connect to each, and funnel work into `work_tx`.
pub async fn run_consumer_loop<W, R>(
    endpoint: Endpoint,
    alpn: Vec<u8>,
    service_name: String,
    work_tx: WorkTx<W, R>,
) -> anyhow::Result<()>
where
    W: Networked + Sync,
    R: Networked + Sync,
{
    let handle = tokio::runtime::Handle::current();
    let (mut discovered_rx, _guard) = discovery::start_discovery(
        service_name,
        endpoint.id(),
        None,
        &handle,
    )?;

    let mut connected = HashSet::<EndpointId>::new();

    while let Some(DiscoveredPeer { endpoint_id, addrs }) = discovered_rx.recv().await {
        if !connected.insert(endpoint_id) {
            continue;
        }
        info!("Discovered producer {}, connecting...", endpoint_id);

        let endpoint = endpoint.clone();
        let alpn = alpn.clone();
        let work_tx = work_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = connection_loop::<W, R>(endpoint, endpoint_id, addrs, alpn, work_tx).await {
                warn!("Connection to producer {} ended: {}", endpoint_id, e);
            }
        });
    }
    Ok(())
}

/// Single producer connection: read pushed work, process, send result.
async fn connection_loop<W, R>(
    endpoint: Endpoint,
    endpoint_id: EndpointId,
    addrs: Vec<std::net::SocketAddr>,
    alpn: Vec<u8>,
    work_tx: WorkTx<W, R>,
) -> anyhow::Result<()>
where
    W: Networked + Sync,
    R: Networked + Sync,
{
    let addr = EndpointAddr::from_parts(endpoint_id, addrs.into_iter().map(TransportAddr::Ip));
    let conn = endpoint.connect(addr, &alpn).await?;
    info!("Connected to producer {}", endpoint_id);

    let (mut send, mut recv) = conn.accept_bi().await?;

    loop {
        let work: W = read_message(&mut recv).await?;
        trace!("Received work from {}", endpoint_id);

        // Send to worker threads via bounded channel
        let (result_tx, result_rx) = tokio::sync::oneshot::channel::<R>();
        work_tx
            .send(WorkEntry { work, sender: result_tx })
            .await
            .map_err(|_| anyhow::anyhow!("work channel closed"))?;

        // Wait for result and send back
        let result = result_rx.await.map_err(|_| anyhow::anyhow!("result sender dropped"))?;
        write_message(&mut send, &result).await?;
        trace!("Sent result to {}", endpoint_id);
    }
}

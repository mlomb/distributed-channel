use crate::wire::{read_message, write_message};
use crate::Networked;
use iroh::protocol::{AcceptError, ProtocolHandler};
use log::{info, trace, warn};

#[derive(Debug, Clone)]
pub struct ProducerProtocol<W, R> {
    work_rx: async_channel::Receiver<W>,
    work_tx: async_channel::Sender<W>,
    result_tx: async_channel::Sender<R>,
}

impl<W, R> ProducerProtocol<W, R>
where
    W: Networked + Sync,
    R: Networked + Sync,
{
    pub fn new(
        work_rx: async_channel::Receiver<W>,
        work_tx: async_channel::Sender<W>,
        result_tx: async_channel::Sender<R>,
    ) -> Self {
        Self { work_rx, work_tx, result_tx }
    }

    async fn handle_connection(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> anyhow::Result<()> {
        let remote = connection.remote_id();
        info!("Consumer connected: {}", remote);

        let (mut send, mut recv) = connection.open_bi().await?;

        loop {
            let work = self.work_rx.recv().await
                .map_err(|_| anyhow::anyhow!("work channel closed"))?;

            if let Err(e) = write_message(&mut send, &work).await {
                // Re-queue work so it isn't lost to a failed connection
                let _ = self.work_tx.send(work).await;
                return Err(e);
            }
            trace!("Sent work to {}", remote);

            match read_message::<R>(&mut recv).await {
                Ok(result) => {
                    self.result_tx.send(result).await
                        .map_err(|_| anyhow::anyhow!("result channel closed"))?;
                    trace!("Received result from {}", remote);
                }
                Err(e) => {
                    warn!("Lost result from {}: {} (work item consumed)", remote, e);
                    return Err(e);
                }
            }
        }
    }
}

impl<W, R> ProtocolHandler for ProducerProtocol<W, R>
where
    W: Networked + Sync,
    R: Networked + Sync,
{
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Result<(), AcceptError> {
        if let Err(e) = self.handle_connection(connection).await {
            info!("Consumer connection ended: {}", e);
        }
        Ok(())
    }
}

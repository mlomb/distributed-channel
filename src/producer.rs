use crate::wire::{read_message, write_message};
use crate::Networked;
use iroh::protocol::{AcceptError, ProtocolHandler};
use log::{info, trace};

#[derive(Debug, Clone)]
pub struct ProducerProtocol<W, R> {
    work_rx: async_channel::Receiver<W>,
    result_tx: async_channel::Sender<R>,
}

impl<W, R> ProducerProtocol<W, R>
where
    W: Networked + Sync,
    R: Networked + Sync,
{
    pub fn new(work_rx: async_channel::Receiver<W>, result_tx: async_channel::Sender<R>) -> Self {
        Self { work_rx, result_tx }
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
            write_message(&mut send, &work).await?;
            trace!("Sent work to {}", remote);

            // Read result
            let result: R = read_message(&mut recv).await?;
            self.result_tx.send(result).await
                .map_err(|_| anyhow::anyhow!("result channel closed"))?;
            trace!("Received result from {}", remote);
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

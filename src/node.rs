use crate::consumer::{self, WorkRx};
use crate::discovery;
use crate::producer::ProducerProtocol;
use crate::Networked;
use tokio::runtime::Runtime;

pub struct NodeSetup {
    /// Protocol name. Used as the ALPN identifier and mDNS service name.
    pub protocol: String,
}

/// Handle to a running node. Dropping it shuts down the runtime.
pub struct Node {
    #[allow(dead_code)]
    runtime: Runtime,
}

impl Default for NodeSetup {
    fn default() -> Self {
        NodeSetup {
            protocol: "distributed-channel-1".to_string(),
        }
    }
}

impl NodeSetup {
    fn alpn(&self) -> Vec<u8> {
        self.protocol.as_bytes().to_vec()
    }

    fn service_name(&self) -> String {
        self.protocol
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
            .collect()
    }

    pub fn into_consumer<W, R>(self) -> (Node, WorkRx<W, R>)
    where
        W: Networked + Sync,
        R: Networked + Sync,
    {
        let runtime = Runtime::new().expect("tokio to initialize");
        let (work_tx, work_rx) = async_channel::bounded(1);
        let alpn = self.alpn();
        let service_name = self.service_name();

        runtime.spawn(async move {
            let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::N0)
                .bind().await.expect("bind endpoint");

            if let Err(e) = consumer::run_consumer_loop::<W, R>(endpoint, alpn, service_name, work_tx).await {
                log::error!("Consumer loop failed: {}", e);
            }
        });

        (Node { runtime }, work_rx)
    }

    pub fn into_producer<W, R>(self) -> (Node, async_channel::Sender<W>, async_channel::Receiver<R>)
    where
        W: Networked + Sync,
        R: Networked + Sync,
    {
        let runtime = Runtime::new().expect("tokio to initialize");
        let (work_tx, work_rx) = async_channel::bounded::<W>(1);
        let (result_tx, result_rx) = async_channel::unbounded::<R>();
        let alpn = self.alpn();
        let service_name = self.service_name();
        let handler = ProducerProtocol::new(work_rx, result_tx);

        runtime.spawn(async move {
            let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::N0)
                .alpns(vec![alpn.clone()])
                .bind().await.expect("bind endpoint");

            let port = endpoint.bound_sockets().first()
                .map(|s| s.port()).expect("at least one bound socket");
            log::info!("Producer endpoint: {} on port {}", endpoint.id(), port);

            let router = iroh::protocol::Router::builder(endpoint.clone())
                .accept(alpn.clone(), handler)
                .spawn();

            // Broadcast presence via mDNS
            let handle = tokio::runtime::Handle::current();
            let (_rx, _mdns_guard) = discovery::start_discovery(
                service_name, endpoint.id(), Some(port), &handle,
            ).expect("start mDNS discovery");

            // Keep the router and mDNS guard alive indefinitely.
            // When Node is dropped, the runtime shuts down, cleaning up everything.
            let _router = router;
            std::future::pending::<()>().await;
        });

        (Node { runtime }, work_tx, result_rx)
    }
}

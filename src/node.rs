use crate::consumer::{self, WorkRx};
use crate::discovery;
use crate::producer;
use crate::Networked;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;

pub struct NodeSetup {
    /// Protocol name. Used as the mDNS service name.
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

fn random_peer_id() -> String {
    use rand::Rng;
    let bytes: [u8; 8] = rand::thread_rng().gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

impl NodeSetup {
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
        let service_name = self.service_name();
        let peer_id = random_peer_id();

        runtime.spawn(async move {
            if let Err(e) = consumer::run_consumer_loop::<W, R>(service_name, peer_id, work_tx).await {
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
        let service_name = self.service_name();
        let peer_id = random_peer_id();

        let internal_work_tx = work_tx.clone();
        runtime.spawn(async move {
            let listener = TcpListener::bind("0.0.0.0:0").await.expect("bind TCP listener");
            let port = listener.local_addr().expect("local addr").port();
            log::info!("Producer listening on port {}", port);

            let handle = tokio::runtime::Handle::current();
            let (_rx, _mdns_guard) = discovery::start_discovery(
                service_name, peer_id, Some(port), &handle,
            ).expect("start mDNS discovery");

            producer::run_producer_loop::<W, R>(listener, work_rx, internal_work_tx, result_tx).await;
        });

        (Node { runtime }, work_tx, result_rx)
    }
}

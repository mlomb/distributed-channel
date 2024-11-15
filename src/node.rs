use crate::{
    consumer::{ConsumerHandler, WorkRx},
    producer::ProducerHandler,
    swarm::SwarmLoop,
    Networked,
};
use tokio::runtime::Runtime;

pub struct NodeSetup {
    /// Address to listen on
    pub listen_address: String,

    /// Name of the protocol
    pub protocol: String,
}

pub struct Node {
    /// The tokio runtime where the network loop is running.
    /// When the `Node` is dropped, the runtime will be shut down.
    #[allow(dead_code)]
    runtime: tokio::runtime::Runtime,
}

impl Default for NodeSetup {
    fn default() -> Self {
        NodeSetup {
            listen_address: "/ip4/0.0.0.0/tcp/0".to_string(),
            protocol: "/mlomb/distributed_channel/1".to_string(),
        }
    }
}

impl NodeSetup {
    pub fn into_consumer<I, W, R>(self) -> (Node, WorkRx<I, W, R>)
    where
        I: Networked,
        W: Networked,
        R: Networked,
    {
        let runtime = Runtime::new().expect("tokio to initialize");

        // this channel is bounded to 1 so that we don't reserve too many work items and starve other consumers
        let (work_tx, work_rx) = async_channel::bounded(1);

        // start network loop
        runtime.spawn(SwarmLoop::<I, W, R, ConsumerHandler<I, W, R>>::start(
            self,
            ConsumerHandler::new(work_tx.clone()),
        ));

        (Node { runtime }, work_rx)
    }

    pub fn into_producer<I, W, R>(
        self,
        init: I,
    ) -> (
        Node,
        crossbeam_channel::Sender<W>,
        crossbeam_channel::Receiver<R>,
    )
    where
        I: Networked,
        W: Networked,
        R: Networked,
    {
        let runtime = Runtime::new().expect("tokio to initialize");

        // this channel is bounded to 1 so that when we consume a work item, we know we are sending it
        let (s, r) = crossbeam_channel::bounded::<W>(1);
        // this channel is unbounded so that the producer loop can send results without blocking
        let (u, v) = crossbeam_channel::unbounded::<R>();

        // start network loop
        runtime.spawn(SwarmLoop::<I, W, R, ProducerHandler<I, W, R>>::start(
            self,
            ProducerHandler::new(init.clone(), r.clone(), u.clone()),
        ));

        (Node { runtime }, s, v)
    }
}

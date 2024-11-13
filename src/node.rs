use crate::{
    consumer::{ConsumerNode, WorkRx},
    producer::ProducerNode,
    swarm::SwarmLoop,
    Networked,
};

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
        let runtime = tokio::runtime::Runtime::new().expect("tokio to initialize");

        let (command_sender, command_receiver) = tokio::sync::mpsc::channel(1);
        let (event_sender, event_receiver) = tokio::sync::mpsc::channel(1);
        let (work_tx, work_rx) = async_channel::bounded(1);

        // start network loop
        runtime.spawn(SwarmLoop::<I, W, R>::start_loop(
            self,
            command_receiver,
            event_sender,
        ));

        // start consumer loop
        runtime.spawn(ConsumerNode::<I, W, R>::start_loop(
            command_sender,
            event_receiver,
            work_tx,
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
        let runtime = tokio::runtime::Runtime::new().expect("tokio to initialize");

        let (command_sender, command_receiver) = tokio::sync::mpsc::channel(1);
        let (event_sender, event_receiver) = tokio::sync::mpsc::channel(1);

        let (s, r) = crossbeam_channel::bounded::<W>(1);
        let (u, v) = crossbeam_channel::bounded::<R>(1);

        // start network loop
        runtime.spawn(SwarmLoop::<I, W, R>::start_loop(
            self,
            command_receiver,
            event_sender,
        ));

        // start producer loop
        runtime.spawn(ProducerNode::<I, W, R>::start_loop(
            command_sender,
            event_receiver,
            init,
            r,
            u,
        ));

        (Node { runtime }, s, v)
    }
}

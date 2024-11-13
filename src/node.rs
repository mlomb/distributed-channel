use crate::{
    consumer::{ConsumerNode, WorkEntry},
    producer::ProducerNode,
    swarm::SwarmLoop,
};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;

pub struct NodeSetup {
    /// Address to listen on
    pub(crate) listen_address: String,

    /// Name of the protocol
    pub(crate) protocol: String,
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
    pub fn into_consumer<I, W, R>(self, tx: tokio::sync::mpsc::Sender<WorkEntry<W, R>>) -> Node
    where
        I: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
        W: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
        R: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
    {
        let runtime = tokio::runtime::Runtime::new().expect("tokio to initialize");

        let (command_sender, command_receiver) = tokio::sync::mpsc::channel(1);
        let (event_sender, event_receiver) = tokio::sync::mpsc::channel(1);

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
            tx,
        ));

        Node { runtime }
    }

    pub fn into_producer<I, W, R>(
        self,
        init: I,
        tx: crossbeam_channel::Receiver<W>,
        rx: crossbeam_channel::Sender<R>,
    ) -> Node
    where
        I: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
        W: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
        R: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
    {
        let runtime = tokio::runtime::Runtime::new().expect("tokio to initialize");

        let (command_sender, command_receiver) = tokio::sync::mpsc::channel(1);
        let (event_sender, event_receiver) = tokio::sync::mpsc::channel(1);

        // start network loop
        runtime.spawn(SwarmLoop::<I, W, R>::start_loop(
            self,
            command_receiver,
            event_sender,
        ));

        // start consumer loop
        runtime.spawn(ProducerNode::<I, W, R>::start_loop(
            command_sender,
            event_receiver,
            init,
            tx,
            rx,
        ));

        Node { runtime }
    }
}

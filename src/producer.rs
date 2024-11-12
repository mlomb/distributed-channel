use core::fmt;

use super::{
    message::MessageRequest,
    swarm::{Event, SwarmNode},
};
use crate::message::MessageResponse;
use crossbeam_channel::{select, Receiver, Sender};
use serde::{de::DeserializeOwned, Serialize};
pub struct ProducerPeer<I, W, R> {
    swarm_node: SwarmNode<I, W, R>,

    init: I,

    rx: Receiver<W>,
    tx: Sender<R>,
}

impl<I, W, R> ProducerPeer<I, W, R>
where
    I: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
    W: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
    R: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
{
    pub fn new(init: I, rx: Receiver<W>, tx: Sender<R>) -> Self {
        ProducerPeer {
            swarm_node: SwarmNode::new(),
            init,
            rx,
            tx,
        }
    }

    pub fn run(mut self) {
        loop {
            select! {
                //recv(self.rx) -> work_definition => {
                //    let work_definition = work_definition.unwrap();
                //    println!("Received work: {:?}", work_definition);
                //},
                recv(self.swarm_node.event_receiver()) -> event => self.handle_event(event.unwrap()),
            };
        }
    }

    fn handle_event(&mut self, event: Event<I, W, R>) {
        match event {
            Event::PeerConnected { peer_id } => {
                println!("Connected to peer: {:?}", peer_id);
            }
            Event::MessageRequestReceived {
                peer_id: _,
                message,
                sender,
            } => match message {
                MessageRequest::WhoAreYou => {
                    println!("Received request for whoami");

                    // https://github.com/libp2p/rust-libp2p/issues/5383
                    sender
                        .send(MessageResponse::MeProducer(self.init.clone()))
                        .unwrap();
                }
                MessageRequest::RequestWork => {
                    if let Ok(work) = self.rx.try_recv() {
                        sender.send(MessageResponse::SomeWork(work)).unwrap();
                    } else {
                        sender.send(MessageResponse::NoWorkAvailable).unwrap();
                    }
                }
                MessageRequest::RespondWork(work_result) => {
                    sender.send(MessageResponse::Acknowledge).unwrap();
                    self.tx.send(work_result).unwrap();
                }
            },
            _ => {}
        }
    }
}

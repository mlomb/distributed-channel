use super::{message::MessageRequest, swarm::Event};
use crate::{
    message::MessageResponse,
    swarm::{CommandTx, EventRx},
};
use core::fmt;
use crossbeam_channel::{select, Receiver, Sender};
use log::{info, trace, warn};
use serde::{de::DeserializeOwned, Serialize};

// https://github.com/libp2p/rust-libp2p/issues/5383
pub struct ProducerNode<I, W, R> {
    init: I,

    command_sender: CommandTx<R>,
    event_receiver: EventRx<I, W, R>,

    tx: Sender<R>,
    rx: Receiver<W>,
}

impl<I, W, R> ProducerNode<I, W, R>
where
    I: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
    W: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
    R: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
{
    pub async fn start_loop(
        command_sender: CommandTx<R>,
        event_receiver: EventRx<I, W, R>,
        init: I,
        rx: Receiver<W>,
        tx: Sender<R>,
    ) {
        Self {
            init,
            command_sender,
            event_receiver,
            tx,
            rx,
        }
        .run_loop()
        .await
    }

    pub async fn run_loop(mut self) {
        loop {
            tokio::select! {
                event = self.event_receiver.recv() => {
                    self.handle_event(event.unwrap());
                },
                //recv(self.rx) -> work_definition => {
                //    let work_definition = work_definition.unwrap();
                //    println!("Received work: {:?}", work_definition);
                //},
                //recv(self.swarm_node.event_receiver()) -> event => self.handle_event(event.unwrap()),
            };
        }
    }

    fn handle_event(&mut self, event: Event<I, W, R>) {
        match event {
            Event::ListeningOn { address: multiaddr } => {
                info!("Producer listening on {}", multiaddr);
            }
            Event::PeerConnected { peer_id } => {
                info!("Connected to peer {}", peer_id);
            }
            Event::MessageRequestReceived {
                peer_id,
                message,
                sender,
            } => match message {
                MessageRequest::WhoAreYou => {
                    info!("Received a request for identity from peer {}", peer_id);

                    sender
                        .send(MessageResponse::MeProducer(self.init.clone()))
                        .unwrap();
                }
                MessageRequest::RequestWork => {
                    trace!("Received a request for work from peer {}", peer_id);

                    if let Ok(work) = self.rx.try_recv() {
                        sender.send(MessageResponse::SomeWork(work)).unwrap();
                    } else {
                        sender.send(MessageResponse::NoWorkAvailable).unwrap();
                    }
                }
                MessageRequest::RespondWork(work_result) => {
                    trace!("Received work result from peer {}", peer_id);

                    // acknowledge receipt
                    sender.send(MessageResponse::Acknowledge).unwrap();
                    // send the result to the channel
                    self.tx.send(work_result).unwrap();
                }
            },
            Event::MessageResponseReceived {
                peer_id,
                message: _,
            } => {
                // producers are not meant to receive responses
                warn!(
                    "Producer received unexpected response from peer {}",
                    peer_id
                );
            }
        }
    }
}

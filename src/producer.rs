use super::{message::MessageRequest, swarm::Event};
use crate::{
    message::MessageResponse,
    swarm::{CommandTx, EventRx},
    Networked,
};
use crossbeam_channel::{Receiver, Sender};
use log::{info, trace, warn};

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
    I: Networked,
    W: Networked,
    R: Networked,
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
            let event = self.event_receiver.recv().await.unwrap();
            self.handle_event(event);
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

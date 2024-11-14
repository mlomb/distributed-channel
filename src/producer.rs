use super::{message::MessageRequest, swarm::Event};
use crate::{
    message::MessageResponse,
    swarm::{CommandTx, EventRx, PeerHandler},
    Networked,
};
use crossbeam_channel::{Receiver, Sender};
use libp2p::PeerId;
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

pub struct ProducerPeerHandler<I, W, R> {
    init: I,

    rx: Receiver<W>,
    tx: Sender<R>,
}

impl<I, W, R> ProducerPeerHandler<I, W, R> {
    pub fn new(init: I, rx: Receiver<W>, tx: Sender<R>) -> Self {
        Self { init, rx, tx }
    }
}

impl<I, W, R> PeerHandler<I, W, R> for ProducerPeerHandler<I, W, R>
where
    I: Clone,
{
    async fn next_request(&mut self) -> Option<(PeerId, MessageRequest<R>)> {
        //timeout
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        None
    }

    fn handle_connection(&self, peer_id: PeerId) -> Option<MessageRequest<R>> {
        None
    }

    fn handle_request(&self, peer_id: PeerId, request: MessageRequest<R>) -> MessageResponse<I, W> {
        match request {
            MessageRequest::WhoAreYou => MessageResponse::MeProducer(self.init.clone()),
            MessageRequest::RequestWork => {
                if let Ok(work) = self.rx.try_recv() {
                    MessageResponse::SomeWork(work)
                } else {
                    MessageResponse::NoWorkAvailable
                }
            }
            MessageRequest::RespondWork(_) => todo!(),
        }
    }

    fn handle_response(
        &mut self,
        peer_id: PeerId,
        response: MessageResponse<I, W>,
    ) -> Option<MessageRequest<R>> {
        None
    }
}

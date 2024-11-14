use super::message::MessageRequest;
use crate::{message::MessageResponse, swarm::PeerHandler};
use crossbeam_channel::{Receiver, Sender};
use libp2p::PeerId;
use log::{info, trace, warn};

// https://github.com/libp2p/rust-libp2p/issues/5383

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
            MessageRequest::RespondWork(result) => {
                // NOTE: this operation is blocking, find a workaround
                // the channel is unbounded so there is little risk of blocking (I think)
                self.tx.send(result).unwrap();
                MessageResponse::Acknowledge
            }
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

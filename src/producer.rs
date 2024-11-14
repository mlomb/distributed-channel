use super::message::MessageRequest;
use crate::{message::MessageResponse, swarm::PeerHandler};
use crossbeam_channel::{Receiver, Sender};
use libp2p::PeerId;

pub struct ProducerHandler<I, W, R> {
    // https://github.com/libp2p/rust-libp2p/issues/5383
    init: I,

    rx: Receiver<W>,
    tx: Sender<R>,
}

impl<I, W, R> ProducerHandler<I, W, R> {
    pub fn new(init: I, rx: Receiver<W>, tx: Sender<R>) -> Self {
        Self { init, rx, tx }
    }
}

impl<I, W, R> PeerHandler<I, W, R> for ProducerHandler<I, W, R>
where
    I: Clone,
{
    async fn next_request(&mut self) -> Option<(PeerId, MessageRequest<R>)> {
        // sleep since we don't have any work to do
        tokio::time::sleep(tokio::time::Duration::from_secs(69_420_666)).await;
        None
    }

    fn handle_connection(&self, _peer_id: PeerId) -> Option<MessageRequest<R>> {
        None
    }

    fn handle_request(
        &self,
        _peer_id: PeerId,
        request: MessageRequest<R>,
    ) -> MessageResponse<I, W> {
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
        _peer_id: PeerId,
        _response: MessageResponse<I, W>,
    ) -> Option<MessageRequest<R>> {
        None
    }
}

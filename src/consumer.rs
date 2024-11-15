use crate::{
    message::{MessageRequest, MessageResponse},
    swarm::PeerHandler,
    Networked,
};
use futures::channel::oneshot;
use libp2p::PeerId;
use log::{info, trace};
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, Mutex},
};

pub type WorkTx<I, W, R> = async_channel::Sender<WorkEntry<I, W, R>>;
pub type WorkRx<I, W, R> = async_channel::Receiver<WorkEntry<I, W, R>>;

pub struct WorkEntry<I, W, R> {
    pub peer_data: Arc<Mutex<I>>,
    pub work_definition: W,
    pub sender: oneshot::Sender<R>,
}

pub struct ConsumerHandler<I, W, R> {
    work_tx: WorkTx<I, W, R>,

    response_rx: tokio::sync::mpsc::Receiver<(PeerId, R)>,
    response_tx: tokio::sync::mpsc::Sender<(PeerId, R)>,

    peers: HashMap<PeerId, Peer<I, W>>,
}

/// A peer in the network, from the perspective of a consumer.
struct Peer<I, W> {
    /// The initialization data provided by the peer.
    init: Option<Arc<Mutex<I>>>,

    /// The next work item provided by this peer.
    next_work: Option<W>,
}

impl<I, W, R> ConsumerHandler<I, W, R>
where
    R: Networked,
{
    pub fn new(work_tx: WorkTx<I, W, R>) -> Self {
        let (response_tx, response_rx) = tokio::sync::mpsc::channel(1);
        Self {
            work_tx,
            response_rx,
            response_tx,
            peers: HashMap::new(),
        }
    }

    fn build_entry(&self, peer_id: PeerId, work_definition: W) -> WorkEntry<I, W, R> {
        let (sender, receiver) = oneshot::channel::<R>();
        let peer_id = peer_id.clone();
        let response_tx = self.response_tx.clone();

        tokio::spawn(async move {
            match receiver.await {
                Ok(result) => {
                    trace!("Sending work result to peer {}", peer_id);

                    response_tx.send((peer_id.clone(), result)).await.unwrap();
                }
                Err(_) => {
                    // receiver dropped
                    // this happens when the `WorkEntry` was not used in the select!
                }
            }
        });

        WorkEntry {
            peer_data: self
                .peers
                .get(&peer_id)
                .unwrap()
                .init
                .as_ref()
                .unwrap()
                .clone(),
            work_definition,
            sender,
        }
    }
}

impl<I, W, R> PeerHandler<I, W, R> for ConsumerHandler<I, W, R>
where
    I: Networked,
    W: Networked,
    R: Networked,
{
    async fn next_request(&mut self) -> Option<(PeerId, MessageRequest<R>)> {
        // TODO: balance
        let next_peer_with_work = self
            .peers
            .iter()
            .filter(|(_, peer)| peer.next_work.is_some())
            .map(|(peer_id, peer)| (peer_id.clone(), peer.next_work.clone().unwrap()))
            .next();

        if let Some((peer_id, next_work)) = next_peer_with_work {
            let entry = self.build_entry(peer_id, next_work);

            tokio::select! {
                _ = self.work_tx.send(entry) => {
                    trace!("Sent work entry from peer {} to channel, asking for more work...", peer_id);

                    self.peers.get_mut(&peer_id).unwrap().next_work = None;
                    // ask for more work
                    Some((peer_id, MessageRequest::RequestWork))
                },
                Some((peer_id, result)) = self.response_rx.recv() => {
                    Some((peer_id,MessageRequest::RespondWork(result)))
                },
            }
        } else {
            // timeout
            tokio::select! {
                Some((peer_id, result)) = self.response_rx.recv() => {
                    Some((peer_id, MessageRequest::RespondWork(result)))
                },
            }
        }
    }

    fn handle_connection(&mut self, peer_id: PeerId) -> Option<MessageRequest<R>> {
        match self.peers.entry(peer_id) {
            Entry::Occupied(_) => {
                // already connected
                None
            }
            Entry::Vacant(entry) => {
                info!("Connected to peer {}, asking for identity", peer_id);

                entry.insert(Peer {
                    init: None,
                    next_work: None,
                });

                Some(MessageRequest::WhoAreYou)
            }
        }
    }

    fn handle_request(
        &mut self,
        peer_id: PeerId,
        request: MessageRequest<R>,
    ) -> MessageResponse<I, W> {
        match request {
            MessageRequest::WhoAreYou => {
                info!("Received a request for identity from peer {}.", peer_id);
                MessageResponse::MeConsumer
            }
            _ => MessageResponse::Acknowledge,
        }
    }

    fn handle_response(
        &mut self,
        peer_id: PeerId,
        response: MessageResponse<I, W>,
    ) -> Option<MessageRequest<R>> {
        match response {
            MessageResponse::MeConsumer => {
                info!("Peer {} is a consumer", peer_id);
                None
            }
            MessageResponse::MeProducer(init) => {
                info!("Peer {} is a producer! Requesting work...", peer_id);

                // save the initialization data
                self.peers.insert(
                    peer_id.clone(),
                    Peer {
                        init: Some(Arc::new(Mutex::new(init))),
                        next_work: None,
                    },
                );

                // request work
                Some(MessageRequest::RequestWork)
            }
            MessageResponse::NoWorkAvailable => {
                // request work again
                // TODO: ADD A TIMEOUT
                Some(MessageRequest::RequestWork)
            }
            MessageResponse::SomeWork(work_definition) => {
                trace!("Received work from peer {}", peer_id);

                self.peers.get_mut(&peer_id).unwrap().next_work = Some(work_definition);
                None
            }
            MessageResponse::Acknowledge => {
                trace!("An acknowledgment was received from peer {}", peer_id);
                None
            }
        }
    }
}

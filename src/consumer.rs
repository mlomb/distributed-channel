use super::swarm::Event;
use crate::{
    message::{MessageRequest, MessageResponse},
    swarm::{Command, CommandTx, EventRx},
    Networked,
};
use futures::channel::oneshot;
use libp2p::PeerId;
use log::{info, trace, warn};
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

pub struct ConsumerNode<I, W, R> {
    peers: HashMap<PeerId, Peer<I, W>>,

    work_tx: WorkTx<I, W, R>,
    command_sender: CommandTx<R>,
    event_receiver: EventRx<I, W, R>,
}

impl<I, W, R> ConsumerNode<I, W, R>
where
    I: Networked,
    W: Networked,
    R: Networked,
{
    pub async fn start_loop(
        command_sender: CommandTx<R>,
        event_receiver: EventRx<I, W, R>,
        work_tx: WorkTx<I, W, R>,
    ) {
        Self {
            peers: HashMap::new(),
            work_tx,
            command_sender,
            event_receiver,
        }
        .run_loop()
        .await
    }

    pub async fn run_loop(mut self) {
        loop {
            let next_peer_with_work = self
                .peers
                .iter()
                .filter(|(_, peer)| peer.next_work.is_some())
                .map(|(peer_id, peer)| (peer_id.clone(), peer.next_work.clone().unwrap()))
                .next();

            if let Some((peer_id, next_work)) = next_peer_with_work {
                let entry = self.build_entry(peer_id, next_work);

                tokio::select! {
                    event = self.event_receiver.recv() => {
                        self.handle_event(event.unwrap()).await;
                    },
                    _ = self.work_tx.send(entry) => {
                        trace!("Sent work entry from peer {} to channel, asking for more work...", peer_id);

                        self.peers.get_mut(&peer_id).unwrap().next_work = None;
                        // ask for more work
                        self.command_sender.send(Command::SendRequest {
                            peer_id,
                            request: MessageRequest::RequestWork,
                        }).await.unwrap();
                    },
                };
            } else {
                let evt = self.event_receiver.recv().await.unwrap();
                self.handle_event(evt).await;
            }
        }
    }

    async fn handle_event(&mut self, event: Event<I, W, R>) {
        match event {
            Event::ListeningOn { address: multiaddr } => {
                info!("Consumer listening on {}", multiaddr);
            }
            Event::PeerConnected { peer_id } => match self.peers.entry(peer_id) {
                Entry::Occupied(_) => {} // we already know this peer
                Entry::Vacant(v) => {
                    info!("Connected to peer {}, asking for identity", peer_id);

                    // add to the map of known peers
                    v.insert(Peer::new(peer_id));
                    // ask for the peer's identity
                    self.command_sender
                        .send(Command::SendRequest {
                            peer_id,
                            request: MessageRequest::WhoAreYou,
                        })
                        .await
                        .unwrap();
                }
            },
            Event::MessageRequestReceived {
                peer_id,
                message,
                sender,
            } => match message {
                MessageRequest::WhoAreYou => {
                    info!("Received a request for identity from peer {}.", peer_id);
                    sender.send(MessageResponse::MeConsumer).unwrap();
                }
                request => {
                    warn!("Unexpected request from peer {}: {:?}", peer_id, request);
                    sender.send(MessageResponse::Acknowledge).unwrap();
                }
            },
            Event::MessageResponseReceived { peer_id, message } => match message {
                MessageResponse::MeConsumer => {
                    info!("Peer {} is a consumer", peer_id);
                }
                MessageResponse::MeProducer(init) => {
                    info!("Peer {} is a producer! Requesting work...", peer_id);

                    // save the initialization data
                    self.peers.get_mut(&peer_id).expect("peer to exist").init =
                        Some(Arc::new(Mutex::new(init)));
                    // request work
                    self.command_sender
                        .send(Command::SendRequest {
                            peer_id,
                            request: MessageRequest::RequestWork,
                        })
                        .await
                        .unwrap();
                }
                MessageResponse::NoWorkAvailable => {
                    todo!()
                }
                MessageResponse::SomeWork(work_definition) => {
                    trace!("Received work from peer {}", peer_id);

                    self.peers
                        .get_mut(&peer_id)
                        .expect("peer to exist")
                        .next_work = Some(work_definition);
                }
                MessageResponse::Acknowledge => {
                    trace!("An acknowledgment was received from peer {}", peer_id);
                }
            },
        }
    }

    fn build_entry(&self, peer_id: PeerId, work_definition: W) -> WorkEntry<I, W, R> {
        let (sender, receiver) = oneshot::channel();

        let peer_id = peer_id.clone();
        let command_sender = self.command_sender.clone();

        tokio::spawn(async move {
            match receiver.await {
                Ok(result) => {
                    trace!("Sending work result to peer {}", peer_id);

                    command_sender
                        .send(Command::SendRequest {
                            peer_id,
                            request: MessageRequest::RespondWork(result),
                        })
                        .await
                        .unwrap();
                }
                Err(_) => {
                    // receiver dropped
                    // this happens when the `WorkEntry` was not used in the select!
                }
            }
        });

        WorkEntry {
            peer_data: self.peers.get(&peer_id).unwrap().init.clone().unwrap(),
            work_definition,
            sender,
        }
    }
}

struct Peer<I, W> {
    peer_id: PeerId,

    init: Option<Arc<Mutex<I>>>,
    next_work: Option<W>,
}

impl<I, W> Peer<I, W> {
    fn new(peer_id: PeerId) -> Self {
        Peer {
            peer_id,
            init: None,
            next_work: None,
        }
    }
}

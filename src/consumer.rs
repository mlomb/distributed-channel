use super::swarm::{Event, SwarmNode};
use crate::{
    message::{MessageRequest, MessageResponse},
    swarm::Command,
};
use crossbeam_channel::{select, Sender};
use futures::{channel::oneshot, SinkExt};
use libp2p::PeerId;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::{hash_map::Entry, HashMap},
    fmt,
};

pub struct WorkEntry<W, R> {
    pub work_definition: W,
    pub sender: oneshot::Sender<R>,
}

pub struct ConsumerPeer<I, W, R> {
    swarm_node: SwarmNode<I, W, R>,

    peers: HashMap<PeerId, Peer<I, W>>,

    tx: Sender<WorkEntry<W, R>>,
}

impl<I, W, R> ConsumerPeer<I, W, R>
where
    I: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
    W: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
    R: fmt::Debug + Send + Clone + Serialize + DeserializeOwned + 'static,
{
    pub fn new(tx: Sender<WorkEntry<W, R>>) -> Self {
        ConsumerPeer {
            swarm_node: SwarmNode::new(),
            peers: HashMap::new(),
            tx,
        }
    }

    pub async fn run(mut self) {
        loop {
            let next_peer_with_work = self
                .peers
                .iter()
                .filter(|(_, peer)| peer.next_work.is_some())
                .map(|(peer_id, peer)| (peer_id.clone(), peer.next_work.clone().unwrap()))
                .next();

            if let Some((peer_id, next_work)) = next_peer_with_work {
                let entry = self.build_entry(peer_id, next_work);

                select! {
                    recv(self.swarm_node.event_receiver()) -> event => self.handle_event(event.unwrap()),
                    send(self.tx, entry) -> _ => {
                        println!("Sent work to peer: {:?}", peer_id);

                        self.peers.get_mut(&peer_id).unwrap().next_work = None;
                        // ask for more work
                        self.swarm_node.send(peer_id, MessageRequest::RequestWork);
                    },
                };
            } else {
                self.handle_event(self.swarm_node.event_receiver().recv().unwrap());
            }
        }
    }

    fn handle_event(&mut self, event: Event<I, W, R>) {
        match event {
            Event::ListeningOn { address: multiaddr } => {
                println!("Listening on: {:?}", multiaddr);
            }
            Event::PeerConnected { peer_id } => match self.peers.entry(peer_id) {
                Entry::Occupied(_) => {}
                Entry::Vacant(v) => {
                    println!("Connected to peer: {:?}.", peer_id);

                    // add to the list of known peers
                    v.insert(Peer::new(peer_id));

                    // ask for the peer's identity
                    self.swarm_node.send(peer_id, MessageRequest::WhoAreYou);
                }
            },
            Event::MessageRequestReceived {
                peer_id: _,
                message,
                sender,
            } => match message {
                MessageRequest::WhoAreYou => {
                    sender.send(MessageResponse::MeConsumer).ok();
                }
                MessageRequest::RequestWork => {
                    println!("Received invalid request for work");
                }
                MessageRequest::RespondWork(work_result) => {
                    println!("Received invalid work ack");
                }
            },
            Event::MessageResponseReceived { peer_id, message } => match message {
                MessageResponse::MeProducer(init) => {
                    println!("Received init from peer: {:?}", peer_id);

                    // save the initialization data
                    self.peers.get_mut(&peer_id).expect("peer to exist").init = Some(init);

                    // request work
                    self.swarm_node.send(peer_id, MessageRequest::RequestWork);
                }
                MessageResponse::MeConsumer => {
                    println!("Peer {:?} is a consumer", peer_id);
                }
                MessageResponse::SomeWork(work_definition) => {
                    self.peers
                        .get_mut(&peer_id)
                        .expect("peer to exist")
                        .next_work = Some(work_definition);
                }
                MessageResponse::Acknowledge => {
                    println!("Received work ACK!");
                }
                _ => {
                    println!("Received unexpected response");
                }
            },
        }
    }

    fn build_entry(&self, peer_id: PeerId, work_definition: W) -> WorkEntry<W, R> {
        let (sender, receiver) = oneshot::channel();

        let peer_id = peer_id.clone();
        let mut cmd_sender = self.swarm_node.command_sender().clone();

        tokio::spawn(async move {
            match receiver.await {
                Ok(result) => {
                    cmd_sender
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
            work_definition,
            sender,
        }
    }
}

struct Peer<I, W> {
    peer_id: PeerId,

    init: Option<I>,
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

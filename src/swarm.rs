use super::message::MessageRequest;
use super::message::MessageResponse;
use crate::node::NodeSetup;
use crate::Networked;
use futures::channel::oneshot;
use futures::StreamExt;
use futures::TryFutureExt;
use libp2p::identity::Keypair;
use libp2p::request_response;
use libp2p::swarm::SwarmEvent;
use libp2p::Multiaddr;
use libp2p::PeerId;
use libp2p::StreamProtocol;
use libp2p::Swarm;
use libp2p::{mdns, swarm::NetworkBehaviour};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt;
use std::future::IntoFuture;
use std::time::Duration;
use tokio::select;

pub type CommandRx<R> = tokio::sync::mpsc::Receiver<Command<R>>;
pub type CommandTx<R> = tokio::sync::mpsc::Sender<Command<R>>;

pub type EventTx<I, W, R> = tokio::sync::mpsc::Sender<Event<I, W, R>>;
pub type EventRx<I, W, R> = tokio::sync::mpsc::Receiver<Event<I, W, R>>;

/// Events sent from the Swarm loop to the outside world.
#[derive(Debug)]
pub enum Event<I, W, R> {
    /// The local node is now listening on the given multiaddr.
    ListeningOn { address: Multiaddr },
    /// A connection to a peer has been established.
    PeerConnected { peer_id: PeerId },
    /// A message request has been received from a peer.
    MessageRequestReceived {
        peer_id: PeerId,
        message: MessageRequest<R>,
        sender: oneshot::Sender<MessageResponse<I, W>>,
    },
    /// A message response has been received from a peer.
    MessageResponseReceived {
        peer_id: PeerId,
        message: MessageResponse<I, W>,
    },
}

/// Commands sent from the outside world to the Swarm loop.
#[derive(Debug)]
pub enum Command<R> {
    SendRequest {
        peer_id: PeerId,
        request: MessageRequest<R>,
    },
}

// A custom network behaviour that combines mDNS with RequestResponse
#[derive(NetworkBehaviour)]
struct Behaviour<I, W, R>
where
    I: Send + Clone + Serialize + DeserializeOwned + 'static,
    W: Send + Clone + Serialize + DeserializeOwned + 'static,
    R: Send + Clone + Serialize + DeserializeOwned + 'static,
{
    mdns: mdns::tokio::Behaviour,
    request_response: request_response::cbor::Behaviour<MessageRequest<R>, MessageResponse<I, W>>,
}

pub struct SwarmLoop<I, W, R>
where
    I: Networked,
    W: Networked,
    R: Networked,
{
    /// The libp2p Swarm
    swarm: Swarm<Behaviour<I, W, R>>,

    /// Receiver for commands from the outside world
    command_receiver: CommandRx<R>,

    /// Sender for events to the outside world
    event_sender: EventTx<I, W, R>,
}

impl<I, W, R> SwarmLoop<I, W, R>
where
    I: Networked,
    W: Networked,
    R: Networked,
{
    pub async fn start_loop(
        node_setup: NodeSetup,
        command_receiver: CommandRx<R>,
        event_sender: EventTx<I, W, R>,
    ) {
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )
            .unwrap()
            .with_behaviour(move |key: &Keypair| {
                Ok(Behaviour {
                    mdns: mdns::tokio::Behaviour::new(
                        mdns::Config::default(),
                        key.public().to_peer_id(),
                    )?,
                    request_response: request_response::cbor::Behaviour::new(
                        [(
                            StreamProtocol::try_from_owned(node_setup.protocol)
                                .expect("a valid protocol"),
                            request_response::ProtocolSupport::Full,
                        )],
                        request_response::Config::default(),
                    ),
                })
            })
            .unwrap()
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX))
            })
            .build();

        // Listen on all interfaces and whatever port the OS assigns
        swarm
            .listen_on(node_setup.listen_address.parse().expect("a valid address"))
            .expect("listen to succeed");

        Self {
            swarm,
            command_receiver,
            event_sender,
        }
        .run_loop()
        .await
    }

    pub async fn run_loop(mut self) {
        loop {
            println!("Swarm Loop");
            tokio::select! {
                Some(event) = self.swarm.next() => self.handle_behaviour_event(event).await,
                Some(command) = self.command_receiver.recv() => self.handle_command(command),
            }
        }
    }

    async fn handle_behaviour_event(&mut self, event: SwarmEvent<BehaviourEvent<I, W, R>>) {
        println!("Handling event: {:?}", event);
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                self.event_sender
                    .send(Event::ListeningOn { address })
                    .await
                    .unwrap();
            }
            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, _multiaddr) in list {
                    self.swarm.dial(peer_id).ok();
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                self.event_sender
                    .send(Event::PeerConnected { peer_id })
                    .await
                    .unwrap();
            }
            SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(rr)) => match rr {
                request_response::Event::Message { peer, message } => match message {
                    request_response::Message::Request {
                        request_id,
                        request,
                        channel,
                    } => {
                        let (sender, receiver) = oneshot::channel();
                        self.event_sender
                            .send(Event::MessageRequestReceived {
                                peer_id: peer,
                                message: request,
                                sender,
                            })
                            .await
                            .unwrap();
                        let response = tokio::time::timeout(Duration::from_secs(3), receiver).await;

                        match response {
                            Ok(Ok(response)) => {
                                self.swarm
                                    .behaviour_mut()
                                    .request_response
                                    .send_response(channel, response)
                                    .unwrap();
                            }
                            Ok(Err(_)) => {
                                println!("Failed to get response from handler");
                            }
                            Err(elapsed) => {
                                println!("Request timed out: {:?}", elapsed);
                            }
                        }
                    }
                    request_response::Message::Response {
                        request_id,
                        response,
                    } => {
                        self.event_sender
                            .send(Event::MessageResponseReceived {
                                peer_id: peer,
                                message: response,
                            })
                            .await
                            .unwrap();
                    }
                },
                request_response::Event::OutboundFailure {
                    peer,
                    request_id,
                    error,
                } => {
                    println!("Outbound failure: {}", error);
                }
                request_response::Event::InboundFailure {
                    peer,
                    request_id,
                    error,
                } => {
                    println!("Inbound failure: {}", error);
                }
                request_response::Event::ResponseSent { .. } => {}
            },
            a => {
                println!("Unhandled event: {:?}", a);
            }
        }
        println!("Finished handling event");
    }

    fn handle_command(&mut self, command: Command<R>) {
        match command {
            Command::SendRequest { peer_id, request } => {
                println!("Sending request to peer!");
                self.swarm
                    .behaviour_mut()
                    .request_response
                    .send_request(&peer_id, request);
            }
        }
    }
}

pub mod consumer;
pub mod consumer_node;
pub mod discovery;
pub mod node;
pub mod producer;
pub mod wire;

pub use crate::consumer::{WorkEntry, WorkRx, WorkTx};
pub use crate::consumer_node::start_consumer_node;
pub use crate::node::{Node, NodeSetup};

/// Marker trait for types that can be sent over the network.
pub trait Networked:
    Send + std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned + 'static
{
}
impl<T> Networked for T where
    T: Send + std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned + 'static
{
}

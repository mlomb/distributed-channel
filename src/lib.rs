pub mod consumer;
pub mod error;
pub mod message;
pub mod node;
pub mod producer;
pub mod swarm;
pub mod work_pool;
pub mod worker;

pub use crate::consumer::WorkEntry;
pub use crate::consumer::WorkTx;
pub use crate::node::Node;
pub use crate::node::NodeSetup;

/// A trait for types that can be sent over the network.
pub trait Networked:
    Send + std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned + 'static
{
}

impl<T> Networked for T where
    T: Send + std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned + 'static
{
}

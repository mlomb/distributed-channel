use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum MessageRequest<R> {
    /// A peer requests the identity of another peer.
    WhoAreYou,

    /// A consumer requests work from a producer.
    RequestWork,

    /// A consumer sends the result of a work item to the producer.
    /// The producer should respond with an acknowledgment.
    RespondWork(R),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MessageResponse<I, W> {
    /// The peer is a consumer.
    MeConsumer,
    /// The peer is a producer with the given initialization data.
    MeProducer(I),

    /// The producer has no work to provide.
    NoWorkAvailable,
    /// The producer provides the given work.
    SomeWork(W),

    /// Generic acknowledgment of a message.
    /// Used when no response is needed.
    Acknowledge,
}

use bytes::Bytes;

use super::protocol::Command;

/// An event sent by the client to the server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BeanstalkClientEvent {
    /// A command sent by the client
    Command(Command),
    /// A chunk of data received from a client for a Put request
    PutChunk(Bytes),
    /// Flag indicating the end of a Put request
    PutEnd,
    /// Flag indicating part of the input was discarded due to a client error
    Discarded,
}

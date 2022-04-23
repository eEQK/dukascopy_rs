/// Represents different possible error types that could happen when
/// interacting with the Dukascopy API.
#[derive(Debug)]
pub enum Kind {
    /// Emitted when data was fetched but is malformed and therefore cannot be decoded
    Decode,

    /// Emitted when a network error occurred, e.g. when the server is not reachable or
    /// when the server is rate-limiting the client
    Network,
}

pub(crate) type BoxError = Box<dyn std::error::Error>;

/// Error that can be emitted when interacting with [DukascopyService](crate::DukascopyService)
#[derive(Debug)]
pub struct Error {
    pub inner: BoxError,
    pub kind: Kind,
}

//! Errors found throughout this crate

use thiserror::Error;
use x11rb::errors::ConnectError;

/// Errors that occur from interacting with the X-Server
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// Invalid property (`Atom`) queried for
    #[error("the property {0} was not found on this server")]
    InvalidProperty(String),

    /// Failure to connect to the server
    #[error("failed to connect to the X11 server: {0}")]
    Connection(#[from] ConnectError),
}

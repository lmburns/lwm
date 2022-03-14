//! Various utilities specifically dealing with X

use crate::error::Error;
use anyhow::Result;

use x11rb::{rust_connection::RustConnection, wrapper::ConnectionExt as _};

// ================== XUtility ====================

/// Wrapper to do basic X11 commands
pub(crate) struct XUtility;

impl XUtility {
    /// Setup the X11 [`Connection`](RustConnection)
    pub(crate) fn setup_connection() -> Result<(RustConnection, usize), Error> {
        RustConnection::connect(None).map_err(Error::Connection)
    }
}
